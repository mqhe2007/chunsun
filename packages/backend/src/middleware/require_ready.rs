//! 安装未完成时拦截业务 API。`/setup/*` 与 `/health` 不走此中间件。

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::AppError;
use crate::state::AppState;

pub async fn require_ready(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if state.is_ready() {
        return next.run(req).await;
    }
    AppError::new(StatusCode::SERVICE_UNAVAILABLE, "SETUP_REQUIRED")
        .with_message("请先完成安装向导")
        .into_response()
}
