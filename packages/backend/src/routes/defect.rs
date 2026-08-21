//! 缺陷域路由（1:1 移植自 `packages/backend/src/routes/defect.ts`）。
//!
//! 六条端点全部落在 `auth_middleware` 之下，权限只有一档——项目可见性，
//! 不可见 404 `PROJECT_NOT_FOUND`。没有 owner 成员校验、没有 SK-only 门禁，
//! 比 requirement 更简单。
//!
//! 字段形态与 TypeBox 的对应关系（都走 `double_option`，见 [`crate::routes::validate`]）：
//!
//! | 字段 | POST | PATCH |
//! | --- | --- | --- |
//! | `description` | `t.Optional(t.String())`，不 trim | `t.Optional(t.String())`，**不 trim** |
//! | `status` | `t.Optional(statusSchema)` | `t.Optional(statusSchema)` |
//! | `severity` | `t.Optional(severitySchema)` | `t.Optional(severitySchema)` |
//! | `requirementId` | `t.Optional(t.String())`（POST）/ `t.Union([String,Null])`（PATCH） | 见下 |
//!
//! 列表 query：`status` 逗号分隔枚举（非法 token 静默丢弃）；`severity` 精确枚举（非法→422）；
//! `requirementId` 精确匹配、**未 trim**，空串按 JS falsy 视为不过滤；
//! `q` 先 trim，再 `OR [id LIKE '%q%'（区分大小写）, description ILIKE '%q%'（不区分）]`。
//!
//! `convert-to-requirement` 是事务端点：已派生需求则幂等返回、状态非 open 则 409，
//! 否则原子建 requirement（origin=defect）+ 回链缺陷（status=processing）。

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::datetime::to_value as dt_value;
use crate::core::query_filters::parse_comma_separated_enum;
use crate::core::serde_ext::double_option;
use crate::routes::dto::defect_dto;
use crate::routes::validate::{
    nullable_optional_string, optional_enum, optional_string,
};
use crate::services::activity_log::{log_activity, ActivityAction, LogActivityOptions};
use crate::services::defect::{
    self as defect_service, ConvertDefectResult, CreateDefectArgs, ListDefectsQuery,
    UpdateDefectArgs,
};
use crate::state::AppState;

/// 列表筛选与 PATCH 共用的状态白名单。
const STATUSES: &[&str] = &["open", "processing", "resolved", "closed"];
/// PATCH 与 create 共用的严重级别白名单。
const SEVERITIES: &[&str] = &["critical", "major", "minor", "trivial"];

/// TypeBox 里这几个字段只写了类型没写长度，用 0..MAX 表达「无长度限制」。
const NO_MIN: usize = 0;
const NO_MAX: usize = usize::MAX;

// ── 请求体 / 查询参数 ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListQuery {
    status: Option<String>,
    severity: Option<String>,
    #[serde(rename = "requirementId")]
    requirement_id: Option<String>,
    q: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDefectBody {
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub status: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub severity: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub requirement_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDefectBody {
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub status: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub severity: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub requirement_id: Option<Option<String>>,
}

// ── handlers ────────────────────────────────────────────────────────────

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let statuses = parse_comma_separated_enum(q.status.as_deref(), STATUSES, |s| *s);
    // 列表 severity 是 query 参数（无「显式 null」语义），有值才校验枚举合法性
    let severity_q = q.severity.map(Some);
    let severity = optional_enum("severity", &severity_q, SEVERITIES)?;

    let rows = defect_service::list_defects(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        ListDefectsQuery {
            status: statuses,
            severity,
            requirement_id: q.requirement_id.as_deref(),
            q: q.q.as_deref(),
        },
    )
    .await?;

    Ok(ok(rows.iter().map(defect_dto).collect()))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<CreateDefectBody>,
) -> Result<(StatusCode, Json<ApiResponse<Value>>), AppError> {
    let description = optional_string("description", &body.description, NO_MIN, NO_MAX)?;
    let status = optional_enum("status", &body.status, STATUSES)?;
    let severity = optional_enum("severity", &body.severity, SEVERITIES)?;
    let requirement_id = optional_string("requirementId", &body.requirement_id, NO_MIN, NO_MAX)?;

    let row = defect_service::create_defect(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        CreateDefectArgs {
            description,
            status,
            severity,
            requirement_id,
        },
    )
    .await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(defect_dto(&row)))))
}

async fn detail(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, defect_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let row = defect_service::get_defect(
        &state.pool(),
        &project_id,
        &defect_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;
    Ok(ok(defect_dto(&row)))
}

async fn update(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, defect_id)): Path<(String, String)>,
    ValidatedJson(body): ValidatedJson<UpdateDefectBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    // PATCH description 没有 minLength，空串合法（不 trim，原样存）
    let description = optional_string("description", &body.description, NO_MIN, NO_MAX)?;
    let status = optional_enum("status", &body.status, STATUSES)?;
    let severity = optional_enum("severity", &body.severity, SEVERITIES)?;
    // requirementId：PATCH 是 `t.Union([String, Null])`，空串走 `|| null` 清空
    let requirement_id = nullable_optional_string("requirementId", &body.requirement_id, NO_MIN, NO_MAX)?
        .map(|v| v.filter(|s| !s.is_empty()));

    let row = defect_service::update_defect(
        &state.pool(),
        &project_id,
        &defect_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        UpdateDefectArgs {
            description,
            status,
            severity,
            requirement_id,
        },
    )
    .await?;

    Ok(ok(defect_dto(&row)))
}

async fn remove(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, defect_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let id = defect_service::delete_defect(
        &state.pool(),
        &project_id,
        &defect_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;
    Ok(ok(json!({ "id": id })))
}

async fn convert(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, defect_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let result = defect_service::convert_defect_to_requirement(
        &state.pool(),
        &defect_id,
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;

    let ConvertDefectResult::Ok { requirement, defect } = result else {
        return match result {
            ConvertDefectResult::NotFound => {
                Err(AppError::not_found("DEFECT_NOT_FOUND"))
            }
            ConvertDefectResult::NotConvertible => {
                Err(AppError::conflict("DEFECT_NOT_CONVERTIBLE"))
            }
            ConvertDefectResult::Ok { .. } => unreachable!(),
        };
    };

    // 事务已提交，仅在此记 activity（对齐旧实现 convert 在 $transaction 外 logActivity）
    log_activity(
        &state.pool(),
        &defect.project_id,
        &session.user.user_id,
        ActivityAction::DefectConverted,
        LogActivityOptions {
            entity_type: Some("requirement"),
            entity_id: Some(&requirement.id),
            description: Some(&format!(
                "缺陷派生修复需求：{} → {}",
                defect.id, requirement.id
            )),
            metadata: Some(json!({
                "defectId": defect.id,
                "requirementId": requirement.id,
            })),
        },
    )
    .await?;

    // 旧实现硬编码返回 requirement 字段（无 ownerId / owner），不套 requirement_dto
    let body = json!({
        "id": requirement.id,
        "projectId": requirement.project_id,
        "repositoryId": requirement.repository_id,
        "description": requirement.description,
        "sourceText": requirement.source_text,
        "clientNotes": requirement.client_notes,
        "status": requirement.status,
        "coverage": requirement.coverage,
        "origin": requirement.origin,
        "releasedAt": requirement.released_at.as_ref().map_or(Value::Null, dt_value),
        "createdAt": dt_value(&requirement.created_at),
        "updatedAt": dt_value(&requirement.updated_at),
        "requirement": { "id": requirement.id, "status": requirement.status },
    });

    Ok(ok(body))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/defects",
            get(list).post(create),
        )
        .route(
            "/projects/{projectId}/defects/{defectId}",
            get(detail).patch(update).delete(remove),
        )
        .route(
            "/projects/{projectId}/defects/{defectId}/convert-to-requirement",
            post(convert),
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
    fn status_and_severity_whitelists_match_prisma_enums() {
        assert_eq!(
            STATUSES,
            &["open", "processing", "resolved", "closed"]
        );
        assert_eq!(
            SEVERITIES,
            &["critical", "major", "minor", "trivial"]
        );
    }

    #[test]
    fn list_status_filter_drops_invalid_tokens() {
        let parsed = parse_comma_separated_enum(Some("open,bogus,resolved"), STATUSES, |s| *s);
        assert_eq!(parsed, Some(vec!["open", "resolved"]));
        assert_eq!(
            parse_comma_separated_enum(Some("bogus"), STATUSES, |s| *s),
            None
        );
    }
}
