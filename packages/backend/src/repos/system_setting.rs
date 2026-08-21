//! 系统设置仓储（1:1 移植自 `systemSettingRepository.ts` / `systemSettingService.ts`）。
//!
//! 关键对齐点：
//! - `GET /settings` 返回**仅 DEFAULT_SETTINGS 中的键**，顺序严格一致，缺失键回落默认值；
//!   任意不在默认表里的 key 不会被返回（旧实现 `getAllSettings` 只遍历 DEFAULT_SETTINGS）。
//! - `normalizeSettingsPatch` 只对 `rateLimit.*` 四个键做数值校验与 clamp，校验失败抛
//!   `INVALID_SETTING`（→ 400）；命中 rate 键时把四个键整体重算后回写。

use crate::api::AppError;
use crate::core::js_number::js_number;
use serde_json::{Map, Value};
use sqlx::PgPool;
use std::collections::HashMap;

/// 与旧后端 `DEFAULT_SETTINGS` 完全一致（键顺序 + 默认值），决定 GET /settings 的返回顺序。
pub const DEFAULT_SETTINGS: &[(&str, &str)] = &[
    ("registration.inviteOnly", "true"),
    ("security.passwordMinLength", "8"),
    ("security.passwordRequireNumber", "true"),
    ("security.passwordRequireUppercase", "false"),
    ("security.passwordRequireSpecialChar", "false"),
    ("security.loginMaxAttempts", "5"),
    ("security.loginLockoutMinutes", "30"),
    ("security.jwtExpiresIn", "2h"),
    ("email.fromAddress", ""),
    ("email.fromName", "春笋"),
    ("email.smtpHost", ""),
    ("email.smtpPort", "587"),
    ("email.smtpSecure", "false"),
    ("email.smtpUser", ""),
    ("email.smtpPassword", ""),
    ("rateLimit.generalMax", "1000"),
    ("rateLimit.generalWindowMs", "60000"),
    ("rateLimit.authMax", "20"),
    ("rateLimit.authWindowMs", "60000"),
];

pub async fn get_setting(pool: &PgPool, key: &str) -> Result<Option<String>, AppError> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"SELECT value FROM system_setting WHERE key = $1"#,
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn get_all_settings(pool: &PgPool) -> Result<HashMap<String, String>, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT key, value FROM system_setting"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// 复刻 `getAllSettings`：按 DEFAULT_SETTINGS 顺序返回，缺失键回落默认值。
pub async fn all_settings_map(pool: &PgPool) -> Result<Value, AppError> {
    let stored = get_all_settings(pool).await?;
    let mut map = Map::new();
    for (k, default) in DEFAULT_SETTINGS {
        let v = stored
            .get(*k)
            .cloned()
            .unwrap_or_else(|| default.to_string());
        map.insert(k.to_string(), Value::String(v));
    }
    Ok(Value::Object(map))
}

pub async fn set_setting(pool: &PgPool, key: &str, value: &str) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO system_setting (key, value, updated_at) VALUES ($1, $2, NOW())
           ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"#,
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_settings(
    pool: &PgPool,
    settings: &HashMap<String, String>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    for (k, v) in settings {
        sqlx::query(
            r#"INSERT INTO system_setting (key, value, updated_at) VALUES ($1, $2, NOW())
               ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()"#,
        )
        .bind(k)
        .bind(v)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 复刻 `normalizeSettingsPatch`：仅校验 rateLimit 四键的数值合法性，命中则整体重算回写。
///
/// 返回规范化后的扁平 map（键保持调用方传入形状，仅 rate 四键被覆盖为 clamp 后的字符串）。
pub fn normalize_settings_patch(input: &Map<String, Value>) -> Result<HashMap<String, String>, AppError> {
    let mut next: HashMap<String, String> = input
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();

    let rate_keys = [
        "rateLimit.generalMax",
        "rateLimit.generalWindowMs",
        "rateLimit.authMax",
        "rateLimit.authWindowMs",
    ];

    // 校验：rate 键存在时其值必须是有限数字（对齐 `Number.isFinite && !isNaN`）。
    // 注意 js_number("") = 0（有限），js_number("abc") = NaN → 抛错。
    for key in rate_keys.iter() {
        if !next.contains_key(*key) {
            continue;
        }
        let raw = next.get(*key).cloned().unwrap_or_default();
        let n = js_number(&raw);
        if !n.is_finite() || n.is_nan() {
            return Err(AppError::bad_request("INVALID_SETTING"));
        }
    }

    if rate_keys.iter().any(|k| next.contains_key(*k)) {
        // 合并默认 + 补丁，再整体 clamp 四个 rate 键（对齐 parseRateLimitSettings）。
        let mut merged: HashMap<String, String> =
            DEFAULT_SETTINGS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        for (k, v) in &next {
            merged.insert(k.clone(), v.clone());
        }
        let general_max = clamp_int(js_number(merged.get("rateLimit.generalMax").unwrap()), 0.0, 100_000.0, 1000.0);
        let general_window = clamp_int(js_number(merged.get("rateLimit.generalWindowMs").unwrap()), 1000.0, 3_600_000.0, 60_000.0);
        let auth_max = clamp_int(js_number(merged.get("rateLimit.authMax").unwrap()), 0.0, 10_000.0, 20.0);
        let auth_window = clamp_int(js_number(merged.get("rateLimit.authWindowMs").unwrap()), 1000.0, 3_600_000.0, 60_000.0);
        next.insert("rateLimit.generalMax".to_string(), general_max.to_string());
        next.insert("rateLimit.generalWindowMs".to_string(), general_window.to_string());
        next.insert("rateLimit.authMax".to_string(), auth_max.to_string());
        next.insert("rateLimit.authWindowMs".to_string(), auth_window.to_string());
    }

    Ok(next)
}

/// 复刻 `clampInt`：`Math.min(max, Math.max(min, Math.trunc(value)))`；调用方已保证有限非 NaN。
fn clamp_int(value: f64, min: f64, max: f64, _fallback: f64) -> i64 {
    let t = value.trunc();
    let c = t.max(min).min(max);
    c as i64
}
