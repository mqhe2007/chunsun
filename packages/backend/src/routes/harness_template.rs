//! `GET /harness/template`：下载当前实例的 harness 技能模板。
//!
//! 挂在 `auth_middleware` 下（与 harness 域一致）：合法 JWT / secretKey → 200；
//! 缺失或无效 Authorization → 401。

use axum::http::StatusCode;
use axum::{Json, Router};
use serde_json::Value;

use crate::api::ApiResponse;
use crate::auth::CurrentUser;
use crate::harness_template::template_payload;
use crate::state::AppState;

async fn get_template(
    CurrentUser(_session): CurrentUser,
) -> (StatusCode, Json<ApiResponse<Value>>) {
    (
        StatusCode::OK,
        Json(ApiResponse::ok(template_payload())),
    )
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/harness/template", axum::routing::get(get_template))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness_template::TEMPLATE_VERSION;

    #[test]
    fn payload_version_is_stable_for_api_contract() {
        let data = template_payload();
        assert_eq!(data["templateVersion"].as_str(), Some(TEMPLATE_VERSION));
        assert!(data["files"]["SKILL.md"].as_str().unwrap().contains("春笋"));
        assert!(data["files"]["slash/chunsun.md"].as_str().is_some());
        assert!(data["files"]["slash/chunsun-fix.md"].as_str().is_some());
    }
}
