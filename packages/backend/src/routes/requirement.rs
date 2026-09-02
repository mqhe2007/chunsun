//! 需求域路由（1:1 移植自 `packages/backend/src/routes/requirement.ts`）。
//!
//! 五条端点全部落在 `auth_middleware` 之下（旧后端 `.use(authGuard)`），
//! 权限只有一档——项目可见性，不可见 404 `PROJECT_NOT_FOUND`。
//!
//! 字段形态与 TypeBox 的对应关系（都走 `double_option`，见 [`crate::routes::validate`]）：
//!
//! | 字段 | POST | PATCH |
//! | --- | --- | --- |
//! | `description` | `t.String({minLength:1})` 必填 | `t.Optional(t.String())`，**空串合法** |
//! | `sourceText` / `clientNotes` | `t.Optional(t.String())` | 同左 |
//! | `coverage` | `t.Optional(union)` | 同左 |
//! | `status` | 不接受（固定 pending） | `t.Optional(union)` |
//! | `releasedAt` | 不接受 | `t.Optional(t.Union([String, Null]))` 三态 |
//! | `ownerId` | `t.Optional(t.Union([String, Null]))` | 同左 |
//!
//! 注意 PATCH 的 `description` 没有 `minLength`——旧实现允许把描述改成空串，
//! 而 POST 不允许建空描述。这个不对称照搬。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ok_with_meta, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::query_filters::parse_comma_separated_enum;
use crate::core::serde_ext::double_option;
use crate::routes::body_helpers::BlockedByRefBody;
use crate::routes::dto::requirement_dto;
use crate::routes::validate::{nullable_optional_string, optional_enum, optional_string, required_string};
use crate::services::dependency::BlockedByRef;
use crate::services::requirement::{
    self as requirement_service, CreateRequirementArgs, ListRequirementsQuery,
    UpdateRequirementArgs,
};
use crate::state::AppState;

/// `REQUIREMENT_STATUSES`（列表筛选与 PATCH 共用同一套白名单）。
const STATUSES: &[&str] = &["pending", "running", "completed", "abandoned"];
const COVERAGES: &[&str] = &["none", "partial", "full"];

/// TypeBox 里这几个字段只写了类型没写长度，用 0..MAX 表达「无长度限制」。
const NO_MIN: usize = 0;
const NO_MAX: usize = usize::MAX;

// ── 请求体 / 查询参数 ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListQuery {
    status: Option<String>,
    id: Option<String>,
    #[serde(rename = "ownerId")]
    owner_id: Option<String>,
    page: Option<String>,
    #[serde(rename = "pageSize")]
    page_size: Option<String>,
}

/// 仅当 `page` 或 `pageSize` 有非空值时启用分页。
/// `page` 默认 1（≥1），`pageSize` 默认 20（1..=100）。
fn parse_list_pagination(page: Option<&str>, page_size: Option<&str>) -> Option<(i64, i64)> {
    let has_page = page.map(str::trim).filter(|s| !s.is_empty()).is_some();
    let has_size = page_size.map(str::trim).filter(|s| !s.is_empty()).is_some();
    if !has_page && !has_size {
        return None;
    }
    let page = page
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
    let page_size = page_size
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    Some((page, page_size))
}

fn list_page_meta(page: i64, page_size: i64, total: i64) -> serde_json::Value {
    json!({
        "page": page,
        "pageSize": page_size,
        "total": total,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequirementBody {
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub repository_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub source_text: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub client_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub coverage: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub owner_id: Option<Option<String>>,
    /// 创建时携带的"被谁阻塞"上游依赖。可选；缺省与 `null` 均合法（视为空列表）。
    /// 每项 `{ kind: "requirement" | "defect", id: string }`；kind 非法或节点不存在返回 422 / 404。
    #[serde(default, deserialize_with = "double_option")]
    pub blocked_by: Option<Option<Vec<BlockedByRefBody>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequirementBody {
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub source_text: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub client_notes: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub status: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub coverage: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub released_at: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub owner_id: Option<Option<String>>,
}

// ── handlers ────────────────────────────────────────────────────────────

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    // 非法状态片段被静默丢弃；全非法 → 不按状态过滤（不是「查不到」）
    let statuses = parse_comma_separated_enum(q.status.as_deref(), STATUSES, |s| *s);
    let pagination = parse_list_pagination(q.page.as_deref(), q.page_size.as_deref());

    let result = requirement_service::list_requirements(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        ListRequirementsQuery {
            status: statuses,
            id: q.id.as_deref(),
            owner_id: q.owner_id.as_deref(),
            page: pagination.map(|(p, _)| p),
            page_size: pagination.map(|(_, s)| s),
        },
    )
    .await?;

    let data: Vec<Value> = result.items.iter().map(requirement_dto).collect();
    if let Some((page, page_size)) = pagination {
        Ok(ok_with_meta(
            data,
            list_page_meta(page, page_size, result.total),
        ))
    } else {
        Ok(ok(data))
    }
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<CreateRequirementBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    let description = required_string("description", &body.description, 1, NO_MAX)?;
    let repository_id = optional_string("repositoryId", &body.repository_id, NO_MIN, NO_MAX)?;
    let source_text = optional_string("sourceText", &body.source_text, NO_MIN, NO_MAX)?;
    let client_notes = optional_string("clientNotes", &body.client_notes, NO_MIN, NO_MAX)?;
    let coverage = optional_enum("coverage", &body.coverage, COVERAGES)?;
    let owner_id = nullable_optional_string("ownerId", &body.owner_id, NO_MIN, NO_MAX)?;

    let blocked_by_raw = body.blocked_by.flatten();
    let blocked_by: Vec<BlockedByRef<'_>> = blocked_by_raw
        .as_deref()
        .map(|arr| {
            arr.iter()
                .map(|r| -> Result<BlockedByRef<'_>, AppError> {
                    let kind = required_string("blockedBy[].kind", &r.kind, 1, NO_MAX)?;
                    let id = required_string("blockedBy[].id", &r.id, 1, NO_MAX)?;
                    Ok(BlockedByRef { kind, id })
                })
                .collect::<Result<Vec<_>, AppError>>()
        })
        .transpose()?
        .unwrap_or_default();
    let blocked_by_slice: &[BlockedByRef<'_>] = &blocked_by;

    let row = requirement_service::create_requirement(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        CreateRequirementArgs {
            repository_id,
            description,
            source_text,
            client_notes,
            coverage,
            owner_id,
            blocked_by: blocked_by_slice,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::ok(requirement_dto(&row))),
    ))
}

async fn detail(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, requirement_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let row = requirement_service::get_requirement(
        &state.pool(),
        &project_id,
        &requirement_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;
    Ok(ok(requirement_dto(&row)))
}

async fn update(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, requirement_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<UpdateRequirementBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    // PATCH 的 description 没有 minLength，空串合法
    let description = optional_string("description", &body.description, NO_MIN, NO_MAX)?;
    let source_text = optional_string("sourceText", &body.source_text, NO_MIN, NO_MAX)?;
    let client_notes = optional_string("clientNotes", &body.client_notes, NO_MIN, NO_MAX)?;
    let status = optional_enum("status", &body.status, STATUSES)?;
    let coverage = optional_enum("coverage", &body.coverage, COVERAGES)?;
    let owner_id = nullable_optional_string("ownerId", &body.owner_id, NO_MIN, NO_MAX)?;

    // 三态 → 原始串透传：releasedAt 解析必须**晚于存在性校验**
    // （对齐 `new Date()` 不抛错、且 record-not-found 先于落库报错），故不在路由层解析。
    let released_at = nullable_optional_string("releasedAt", &body.released_at, NO_MIN, NO_MAX)?;

    let row = requirement_service::update_requirement(
        &state.pool(),
        &project_id,
        &requirement_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        UpdateRequirementArgs {
            description,
            source_text,
            client_notes,
            status,
            coverage,
            released_at,
            owner_id,
        },
        &state.config().public_origin,
    )
    .await?;

    Ok(ok(requirement_dto(&row)))
}

async fn remove(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, requirement_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let id = requirement_service::delete_requirement(
        &state.pool(),
        &project_id,
        &requirement_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;
    Ok(ok(json!({ "id": id })))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/projects/{projectId}/requirements", get(list).post(create))
        .route(
            "/projects/{projectId}/requirements/{requirementId}",
            get(detail).patch(update).delete(remove),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_and_coverage_whitelists_match_prisma_enums() {
        assert_eq!(STATUSES, &["pending", "running", "completed", "abandoned"]);
        assert_eq!(COVERAGES, &["none", "partial", "full"]);
    }

    #[test]
    fn list_status_filter_drops_invalid_tokens() {
        let parsed = parse_comma_separated_enum(Some("pending,bogus,completed"), STATUSES, |s| *s);
        assert_eq!(parsed, Some(vec!["pending", "completed"]));
        // 全非法 → None，语义是「不过滤」，而不是「查不到」
        assert_eq!(
            parse_comma_separated_enum(Some("bogus"), STATUSES, |s| *s),
            None
        );
    }

    #[test]
    fn parse_list_pagination_absent_means_unpaginated() {
        assert_eq!(parse_list_pagination(None, None), None);
        assert_eq!(parse_list_pagination(Some(""), Some("")), None);
        assert_eq!(parse_list_pagination(Some("  "), None), None);
    }

    #[test]
    fn parse_list_pagination_defaults_and_clamps() {
        assert_eq!(parse_list_pagination(Some("1"), Some("20")), Some((1, 20)));
        assert_eq!(parse_list_pagination(Some("2"), None), Some((2, 20)));
        assert_eq!(parse_list_pagination(None, Some("10")), Some((1, 10)));
        assert_eq!(parse_list_pagination(Some("0"), Some("5")), Some((1, 5)));
        assert_eq!(parse_list_pagination(Some("-3"), Some("200")), Some((1, 100)));
        assert_eq!(parse_list_pagination(Some("abc"), Some("xyz")), Some((1, 20)));
    }

    #[test]
    fn list_page_meta_includes_status_page_contract() {
        let meta = list_page_meta(2, 20, 45);
        assert_eq!(meta["page"], 2);
        assert_eq!(meta["pageSize"], 20);
        assert_eq!(meta["total"], 45);
    }
}
