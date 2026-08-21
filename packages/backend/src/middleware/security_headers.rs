//! 安全响应头，对齐 `packages/backend/src/middleware/securityHeaders.ts`。

use axum::extract::State;
use axum::http::header::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

use crate::state::AppState;

pub async fn security_headers(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    let mut res = next.run(req).await;

    let headers = res.headers_mut();
    headers.insert(HeaderName::from_static("x-content-type-options"), HeaderValue::from_static("nosniff"));
    headers.insert(HeaderName::from_static("x-frame-options"), HeaderValue::from_static("DENY"));
    headers.insert(HeaderName::from_static("x-xss-protection"), HeaderValue::from_static("1; mode=block"));
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    let (is_production, api_prefix) = match state.try_config() {
        Some(cfg) => (cfg.is_production(), cfg.api_prefix),
        None => (false, "/api/v1".to_string()),
    };

    if is_production {
        headers.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }

    // CSP：允许同域脚本样式、inline style、QQ 头像图片、同域 API
    let csp = format!(
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https://q.qlogo.cn; connect-src 'self' {}; font-src 'self'; object-src 'none'; base-uri 'self'; form-action 'self';",
        api_prefix
    );
    if let Ok(v) = HeaderValue::from_str(&csp) {
        headers.insert(HeaderName::from_static("content-security-policy"), v);
    }

    res
}
