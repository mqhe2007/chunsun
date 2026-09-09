//! 项目级记忆路由：`GET/PUT /projects/{projectId}/memory`。
//!
//! - 「仅可编辑不可删除」：本模块只挂 GET/PUT，**不提供 DELETE**——与宪法一致，
//!   删除天然不可达（静态路由无 DELETE 方法时 axum 回 405）。
//! - 容量上限与需求工作记忆统一（`core::memory::MEMORY_SNAPSHOT_MAX_CHARS`，10_000）。
//! - 权限对齐知识库路由：项目成员可见即可读写（`visible` 校验）。

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::api::{ok, AppError, ApiResponse, ValidatedJson};
use crate::auth::CurrentUser;
use crate::core::memory::validate_memory_snapshot;
use crate::core::serde_ext::double_option;
use crate::repos::project_memory::{
    get_project_memory, project_memory_dto, upsert_project_memory,
};
use crate::routes::project_knowledge::visible;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct MemoryBody {
    #[serde(default, deserialize_with = "double_option")]
    snapshot: Option<Option<String>>,
}

async fn get_memory_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let row = get_project_memory(&state.pool(), &pid).await?;
    let Some(row) = row else {
        return Err(AppError::not_found("MEMORY_NOT_FOUND"));
    };
    Ok(ok(project_memory_dto(&row)))
}

async fn put_memory_handler(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
    ValidatedJson(body): ValidatedJson<MemoryBody>,
) -> Result<Json<ApiResponse<serde_json::Value>>, AppError> {
    let pid = visible(&state, &session, &project_id).await?;
    let snapshot = validate_memory_snapshot(&body.snapshot)?;
    let row = upsert_project_memory(&state.pool(), &pid, Some(snapshot)).await?;
    Ok(ok(project_memory_dto(&row)))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/memory",
            get(get_memory_handler).put(put_memory_handler),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
