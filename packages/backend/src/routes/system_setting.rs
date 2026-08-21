//! 系统设置路由（1:1 移植自 `systemSetting.ts`，另增测试发信）。
//!
//! 全部端点都在 `auth_middleware` 之下，且**仅 ADMIN**：非 ADMIN → 403 FORBIDDEN。
//!
//! | 端点 | 说明 |
//! | --- | --- |
//! | `GET /admin/settings` | 返回默认键表（缺省回落默认值） |
//! | `PATCH /admin/settings` | body 为 `Record<string,string>`，`normalizeSettingsPatch` 校验后批量写入 |
//! | `POST /admin/settings/{key}` | 单键写入，同样过 `normalizeSettingsPatch` |
//! | `POST /admin/email/test` | 用已保存 SMTP 向指定收件人发测试邮件 |
//!
//! 注意：PATCH/POST 的非法 rateLimit 值 → 400 INVALID_SETTING（旧端由 `normalizeSettingsPatch`
//! 抛错后返回 400）。

use axum::extract::{Path, State};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::api::{ok, ok_no_data, AppError};
use crate::auth::{AuthSession, CurrentUser};
use crate::repos::system_setting::{
    all_settings_map, normalize_settings_patch, set_setting, set_settings,
};
use crate::services::email::send_test_email;
use crate::state::AppState;

fn require_admin(session: &AuthSession) -> Result<(), AppError> {
    if session.user.role != "ADMIN" {
        return Err(AppError::forbidden("FORBIDDEN"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SettingsKeyBody {
    value: String,
}

#[derive(Debug, Deserialize)]
struct TestEmailBody {
    to: String,
}

async fn list(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
) -> Result<Json<crate::api::ApiResponse<Value>>, AppError> {
    require_admin(&session)?;
    let m = all_settings_map(&state.pool()).await?;
    Ok(ok(m))
}

async fn patch(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<Value>,
) -> Result<Json<crate::api::ApiResponse<()>>, AppError> {
    require_admin(&session)?;
    // 旧端 `t.Record(t.String(), t.String())` 对非对象 body（数组/字符串/数字/null）不会在 schema 层 422：
    // 实际落入 `normalizeSettingsPatch`，其内部 `(patch && typeof patch==="object" && !Array.isArray(patch)) ? patch : {}`
    // 把所有非对象退化成空补丁 → 200 成功（无写入）。这里对齐：非对象即当作空 Map。
    let map = match body.as_object() {
        Some(m) => m.clone(),
        None => Map::new(),
    };
    let normalized = normalize_settings_patch(&map)?;
    set_settings(&state.pool(), &normalized).await?;
    Ok(ok_no_data())
}

async fn set_key(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    Path(key): Path<String>,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<SettingsKeyBody>,
) -> Result<Json<crate::api::ApiResponse<()>>, AppError> {
    require_admin(&session)?;
    let mut m = Map::new();
    m.insert(key.clone(), Value::String(body.value.clone()));
    let normalized = normalize_settings_patch(&m)?;
    let value = normalized.get(&key).cloned().unwrap_or(body.value);
    set_setting(&state.pool(), &key, &value).await?;
    Ok(ok_no_data())
}

async fn test_email(
    State(state): State<AppState>,
    CurrentUser(session): CurrentUser,
    crate::api::ValidatedJson(body): crate::api::ValidatedJson<TestEmailBody>,
) -> Result<Json<crate::api::ApiResponse<()>>, AppError> {
    require_admin(&session)?;
    send_test_email(&state.pool(), &body.to).await?;
    Ok(ok_no_data())
}

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/admin/settings", axum::routing::get(list).patch(patch))
        .route(
            "/admin/settings/{key}",
            axum::routing::post(set_key),
        )
        .route("/admin/email/test", axum::routing::post(test_email))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth::auth_middleware,
        ))
}
