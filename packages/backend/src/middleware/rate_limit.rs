//! 内存令牌桶限流，对齐 `packages/backend/src/middleware/rateLimit.ts`。
//! 默认策略取自 DEFAULT_SETTINGS（generalMax=1000/60s，authMax=20/60s）；
//! DB 动态策略（system_setting 表 + 30s 缓存）在系统设置域移植时接入。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::header::RETRY_AFTER;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::api::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    pub max: u32,
    pub window_ms: u64,
    pub key_prefix: &'static str,
}

impl RateLimitConfig {
    pub const GENERAL: Self = Self { max: 1000, window_ms: 60_000, key_prefix: "general" };
    pub const AUTH: Self = Self { max: 20, window_ms: 60_000, key_prefix: "auth" };
}

struct Entry {
    count: u32,
    reset_at: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, Entry>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())), config }
    }

    /// 返回 Ok(()) 放行；Err(retry_after_secs) 限流。
    pub fn check(&self, ip: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut store = self.inner.lock().unwrap();

        // 惰性清理过期条目
        store.retain(|_, e| e.reset_at > now);

        let key = format!("{}:{ip}", self.config.key_prefix);
        let entry = store
            .entry(key)
            .or_insert_with(|| Entry { count: 0, reset_at: now + Duration::from_millis(self.config.window_ms) });

        entry.count += 1;
        if entry.count > self.config.max {
            let retry_after = entry.reset_at.saturating_duration_since(now).as_secs();
            return Err(retry_after.max(1));
        }
        Ok(())
    }
}

/// 取客户端 IP：x-forwarded-for 首段 → x-real-ip → 无法识别返回 None（不限流）。
fn client_ip(headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(fwd) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        let first = fwd.split(',').next().map(str::trim).filter(|s| !s.is_empty());
        if let Some(ip) = first {
            return Some(ip.to_string());
        }
    }
    if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()).map(str::to_string) {
        return Some(ip);
    }
    None
}

fn rate_limited_response(retry_after: u64) -> Response {
    let body = ApiResponse::<()> {
        success: false,
        data: None,
        error: Some("RATE_LIMITED".to_string()),
        message: None,
        hint: Some(format!("请求过于频繁，请 {retry_after} 秒后再试")),
        meta: None,
    };
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(RETRY_AFTER, retry_after.to_string())],
        axum::Json(body),
    )
        .into_response()
}

async fn apply(limiter: &RateLimiter, req: Request, next: Next) -> Response {
    let Some(ip) = client_ip(req.headers()) else {
        return next.run(req).await;
    };
    match limiter.check(&ip) {
        Ok(()) => next.run(req).await,
        Err(retry_after) => rate_limited_response(retry_after),
    }
}

pub async fn general_rate_limit(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let limiter = state.rate_limiter().clone();
    apply(&limiter, req, next).await
}

/// 认证类端点严格限流（register / login / verify-email / ... ）。
pub async fn auth_rate_limit(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let limiter = state.auth_rate_limiter().clone();
    apply(&limiter, req, next).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_under_max_and_blocks_over() {
        let limiter = RateLimiter::new(RateLimitConfig { max: 2, window_ms: 60_000, key_prefix: "test" });
        assert!(limiter.check("1.2.3.4").is_ok());
        assert!(limiter.check("1.2.3.4").is_ok());
        assert!(limiter.check("1.2.3.4").is_err());
        // 不同 IP 独立计数
        assert!(limiter.check("5.6.7.8").is_ok());
    }

    #[test]
    fn key_prefix_isolates_buckets() {
        let a = RateLimiter::new(RateLimitConfig { max: 1, window_ms: 60_000, key_prefix: "a" });
        let b = RateLimiter::new(RateLimitConfig { max: 1, window_ms: 60_000, key_prefix: "b" });
        assert!(a.check("9.9.9.9").is_ok());
        assert!(b.check("9.9.9.9").is_ok()); // 不同前缀不互扰
        assert!(a.check("9.9.9.9").is_err());
    }
}
