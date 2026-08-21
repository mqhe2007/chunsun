//! 实例级配置（chunsun.json）：安装后可在管理后台修改 publicOrigin。

use axum::extract::State;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::{ok, ok_no_data, AppError};
use crate::auth::{AuthSession, CurrentUser};
use crate::services::instance_config;
use crate::state::AppState;

fn require_admin(session: &AuthSession) -> Result<(), AppError> {
    if session.user.role != "ADMIN" {
        return Err(AppError::forbidden("FORBIDDEN"));
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceConfigResponse {
    public_origin: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PatchInstanceConfigBody {
    public_origin: String,
}

async fn get_config(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<crate::api::ApiResponse<InstanceConfigResponse>>, AppError> {
    require_admin(&session)?;
    let cfg = state.config();
    Ok(ok(InstanceConfigResponse {
        public_origin: cfg.public_origin,
    }))
}

async fn patch_config(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<PatchInstanceConfigBody>,
) -> Result<Json<crate::api::ApiResponse<()>>, AppError> {
    require_admin(&session)?;
    instance_config::update_public_origin(&state, &body.public_origin)?;
    Ok(ok_no_data())
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/admin/instance",
            axum::routing::get(get_config).patch(patch_config),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
