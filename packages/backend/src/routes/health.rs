//! 健康检查端点（对齐旧后端 index.ts：返回裸对象，无 success 包络）。

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::api::AppError;
use crate::state::AppState;

/// GET {prefix}/health — 对齐旧后端：`{status, version, uptime}`（uptime 为浮点秒）
pub async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": uptime_secs_f64(),
    }))
}

/// GET {prefix}/health/db — 对齐旧后端：`{status: "ok"}`（真实 DB 往返）
pub async fn health_db(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    sqlx::query("SELECT 1")
        .execute(&state.pool())
        .await?;
    Ok(Json(json!({ "status": "ok" })))
}

fn uptime_secs_f64() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_is_monotonic() {
        let a = uptime_secs_f64();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let b = uptime_secs_f64();
        assert!(b >= a);
    }
}
