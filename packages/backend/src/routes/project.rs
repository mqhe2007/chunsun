//! 项目域路由（1:1 移植自 `packages/backend/src/routes/project.ts`）。
//!
//! 全部端点落在 `auth_middleware` 之下（旧后端 `.use(authGuard)`）。
//!
//! 请求体用 `Option<Option<String>>` 建模，用于区分 TypeBox 的两种失败语义：
//! - 字段**缺省**（`None`）→ 不更新 / 走默认值
//! - 字段**显式 null**（`Some(None)`）→ 422，因为 `t.String()` 不接受 null
//!
//! 注意必须配 [`crate::core::serde_ext::double_option`]：serde 默认会把 JSON `null`
//! 折叠成外层 `None`，与「字段缺省」撞车，显式 null 会被静默放行。

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::api::{ok, ok_with_meta, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::js_number::{js_number, to_json_number, total_pages};
use crate::core::serde_ext::double_option;
use crate::routes::dto::{project_dto, prompt_dto, repository_dto};
use crate::routes::validate::{optional_string, required_string};
use crate::services::project as project_service;
use crate::state::AppState;

// ── 请求体 / 查询参数 ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ListQuery {
    page: Option<String>,
    #[serde(rename = "pageSize")]
    page_size: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectBody {
    #[serde(default, deserialize_with = "double_option")]
    pub name: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectBody {
    #[serde(default, deserialize_with = "double_option")]
    pub name: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub description: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBody {
    #[serde(default, deserialize_with = "double_option")]
    pub system_prompt: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub user_prompt_template: Option<Option<String>>,
}

// ── Handlers ───────────────────────────────────────────────────────────

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    // 对齐 `query.page ? Number(query.page) : 1`：空串是 falsy，走默认值
    let page = q
        .page
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(js_number)
        .unwrap_or(1.0);
    let page_size = q
        .page_size
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(js_number)
        .unwrap_or(20.0);

    let is_admin = session.user.role == "ADMIN";
    let result =
        project_service::list_projects(&state.pool(), &session.user.user_id, is_admin, page, page_size)
            .await?;

    let data: Vec<Value> = result.items.iter().map(project_dto).collect();
    let meta = json!({
        "total": result.total,
        "page": to_json_number(page),
        "pageSize": to_json_number(page_size),
        "totalPages": to_json_number(total_pages(result.total, page_size)),
    });
    Ok(ok_with_meta(data, meta))
}

async fn detail(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let is_admin = session.user.role == "ADMIN";
    let detail = project_service::get_project_detail(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        is_admin,
    )
    .await?;

    let mut data = project_dto(&detail.project);
    let obj = data.as_object_mut().expect("project_dto 恒为对象");
    obj.insert("statistics".to_string(), detail.statistics);
    obj.insert(
        "repositories".to_string(),
        Value::Array(detail.repositories.iter().map(repository_dto).collect()),
    );
    Ok(ok(data))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    ValidatedJson(body): ValidatedJson<CreateProjectBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let name = required_string("name", &body.name, 1, 100)?;
    let description = optional_string("description", &body.description, 0, usize::MAX)?;

    let project =
        project_service::create_project(&state.pool(), &session.user.user_id, name, description)
            .await?;
    Ok(ok(project_dto(&project)))
}

async fn update(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<UpdateProjectBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let name = optional_string("name", &body.name, 1, 100)?;
    let description = optional_string("description", &body.description, 0, usize::MAX)?;

    let is_admin = session.user.role == "ADMIN";
    let updated = project_service::update_project(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        is_admin,
        name,
        description,
    )
    .await?;
    Ok(ok(project_dto(&updated)))
}

async fn remove(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let is_admin = session.user.role == "ADMIN";
    let deleted = project_service::delete_project(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        is_admin,
    )
    .await?;
    Ok(ok(project_dto(&deleted)))
}

async fn get_prompt(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let is_admin = session.user.role == "ADMIN";
    let prompt =
        project_service::get_prompt(&state.pool(), &project_id, &session.user.user_id, is_admin)
            .await?;
    Ok(ok(prompt_dto(&prompt)))
}

async fn update_prompt(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<PromptBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let system_prompt = optional_string("systemPrompt", &body.system_prompt, 1, usize::MAX)?;
    let user_prompt_template =
        optional_string("userPromptTemplate", &body.user_prompt_template, 1, usize::MAX)?;

    let is_admin = session.user.role == "ADMIN";
    let prompt = project_service::update_prompt(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        is_admin,
        system_prompt,
        user_prompt_template,
    )
    .await?;
    Ok(ok(prompt_dto(&prompt)))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/projects", get(list).post(create))
        .route(
            "/projects/{projectId}",
            get(detail).patch(update).delete(remove),
        )
        .route(
            "/projects/{projectId}/prompt",
            get(get_prompt).patch(update_prompt),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::project::ProjectRow;
    use chrono::Utc;
    use serde_json::json;

    fn sample_project(secret: Option<&str>) -> ProjectRow {
        ProjectRow {
            id: "p1".into(),
            user_id: "u1".into(),
            name: "示例项目".into(),
            description: Some("desc".into()),
            secret_key: secret.map(str::to_string),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn project_dto_never_leaks_secret_key() {
        let v = project_dto(&sample_project(Some("sk_abc")));
        assert!(v.get("secretKey").is_none());
        assert_eq!(v["hasSecretKey"], json!(true));
        assert_eq!(v["userId"], "u1");
    }

    #[test]
    fn has_secret_key_is_false_when_absent() {
        let v = project_dto(&sample_project(None));
        assert_eq!(v["hasSecretKey"], json!(false));
        assert_eq!(v["description"], json!("desc"));
    }

    #[test]
    fn explicit_null_is_rejected_but_absent_is_allowed() {
        let absent: Option<Option<String>> = None;
        let null: Option<Option<String>> = Some(None);
        assert!(optional_string("description", &absent, 0, usize::MAX).unwrap().is_none());
        assert!(optional_string("description", &null, 0, usize::MAX).is_err());
        // 必填字段缺省同样 422
        assert!(required_string("name", &absent, 1, 100).is_err());
    }

    #[test]
    fn name_length_bounds_match_typebox() {
        let ok_name = Some(Some("a".repeat(100)));
        let too_long = Some(Some("a".repeat(101)));
        let empty = Some(Some(String::new()));
        assert!(required_string("name", &ok_name, 1, 100).is_ok());
        assert!(required_string("name", &too_long, 1, 100).is_err());
        assert!(required_string("name", &empty, 1, 100).is_err());
    }

    #[test]
    fn prompt_fields_require_non_empty_when_present() {
        let empty = Some(Some(String::new()));
        assert!(optional_string("systemPrompt", &empty, 1, usize::MAX).is_err());
    }

    /// 回归：serde 默认会把 JSON `null` 折叠成外层 `None`，导致
    /// `{"description": null}` 被当成「字段缺省」放行（旧后端是 422）。
    /// 三个请求体都必须挂 `double_option`。
    #[test]
    fn explicit_null_survives_deserialization_for_every_body() {
        let c: CreateProjectBody =
            serde_json::from_str(r#"{"name":"x","description":null}"#).unwrap();
        assert_eq!(c.description, Some(None));
        assert!(optional_string("description", &c.description, 0, usize::MAX).is_err());

        let u: UpdateProjectBody = serde_json::from_str(r#"{"name":null}"#).unwrap();
        assert_eq!(u.name, Some(None));
        assert!(optional_string("name", &u.name, 1, 100).is_err());

        let p: PromptBody = serde_json::from_str(r#"{"systemPrompt":null}"#).unwrap();
        assert_eq!(p.system_prompt, Some(None));
        assert!(optional_string("systemPrompt", &p.system_prompt, 1, usize::MAX).is_err());

        // 而字段缺省仍然是 None（不更新），不能被误伤
        let empty: UpdateProjectBody = serde_json::from_str("{}").unwrap();
        assert_eq!(empty.name, None);
        assert!(optional_string("name", &empty.name, 1, 100).unwrap().is_none());
    }
}
