//! 仓库域路由（1:1 移植自 `packages/backend/src/routes/repository.ts`）。
//!
//! 三条端点全部落在 `auth_middleware` 之下（旧后端 `.use(authGuard)`）。
//!
//! 请求体字段建模成 `Option<Option<String>>` + [`double_option`]，配合
//! [`crate::routes::validate`] 区分「缺省 / 显式 null / 有值」——`rootHint` 是本仓
//! 第一个 `t.Optional(t.Nullable(...))` 字段，显式 null 合法，不能照抄 project 域
//! 的 `optional_string`（那个见 null 就 422）。

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

use crate::api::{ok, ApiResponse, AppError, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::serde_ext::double_option;
use crate::routes::dto::repository_dto;
use crate::routes::validate::{nullable_optional_string, optional_string, required_string};
use crate::services::repository as repository_service;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepositoryBody {
    #[serde(default, deserialize_with = "double_option")]
    pub name: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub slug: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub root_hint: Option<Option<String>>,
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let rows = repository_service::list_repositories(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;

    Ok(ok(Value::Array(rows.iter().map(repository_dto).collect())))
}

async fn create(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<CreateRepositoryBody>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let name = required_string("name", &body.name, 1, 100)?;
    let slug = optional_string("slug", &body.slug, 1, 100)?;
    // POST 语义下「不传」与「传 null」同义（旧实现 `input.rootHint ?? null`），摊平即可
    let root_hint = nullable_optional_string("rootHint", &body.root_hint, 1, 500)?.flatten();

    let row = repository_service::create_repository(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        name,
        slug,
        root_hint,
    )
    .await?;

    Ok(ok(repository_dto(&row)))
}

async fn detail(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path((project_id, repository_id)): Path<(String, String)>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let row = repository_service::get_repository(
        &state.pool(),
        &project_id,
        &repository_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
    )
    .await?;

    Ok(ok(repository_dto(&row)))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/repositories",
            get(list).post(create),
        )
        .route(
            "/projects/{projectId}/repositories/{repositoryId}",
            get(detail),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
