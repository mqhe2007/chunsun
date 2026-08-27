//! 项目 Secret Key 路由（1:1 移植自 `packages/backend/src/routes/projectSecretKey.ts`）。
//!
//! 三条端点的权限差异：
//! - `GET`   → `secretKey.read`（任意成员），SK 通道需与路由 projectId 一致
//! - `POST /generate` / `DELETE` → `secretKey.write`（创建者 / OWNER / ADMIN），
//!   且 **SK 通道一律 403**，防止用旧密钥自举出新密钥。

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use crate::api::{ok, ApiResponse, AppError};
use crate::auth::CurrentUser;
use crate::services::project as project_service;
use crate::state::AppState;

async fn get_key(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let view = project_service::get_secret_key(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        session.project_id.as_deref(),
    )
    .await?;

    Ok(ok(json!({
        "secretKey": view.secret_key,
        "hasSecretKey": view.has_secret_key,
    })))
}

async fn generate(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let key = project_service::generate_secret_key(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        session.project_id.as_deref(),
        &state.config().public_origin,
    )
    .await?;

    Ok(ok(json!({ "secretKey": key })))
}

/// 撤销成功时旧后端返回的是 `{ success: true }`（**没有 data 字段**）。
async fn revoke(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(project_id): Path<String>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    project_service::revoke_secret_key(
        &state.pool(),
        &project_id,
        &session.user.user_id,
        session.user.role == "ADMIN",
        session.project_id.as_deref(),
        &state.config().public_origin,
    )
    .await?;

    Ok(Json(ApiResponse::ok_no_data()))
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/projects/{projectId}/secret-key",
            get(get_key).delete(revoke),
        )
        .route("/projects/{projectId}/secret-key/generate", post(generate))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
