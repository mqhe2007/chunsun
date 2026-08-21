//! 安全策略服务层：把 DB 动态策略（system_setting）与纯函数判定（core::security_policy）组合。
//!
//! 对齐 `packages/backend/src/lib/securityPolicy.ts` 中带 DB 的四个函数：
//! validatePassword / checkLoginLockout / recordFailedLogin / recordSuccessfulLogin。

use chrono::{Duration, Utc};
use sqlx::PgPool;

use crate::api::AppError;
use crate::core::security_policy::{
    evaluate_lockout, validate_password, LockoutStatus, LoginLockoutPolicy, PasswordPolicy,
    PasswordValidationResult,
};
use crate::repos::login_attempt;
use crate::services::settings;

/// 读取 DB 密码策略并校验明文密码。
pub async fn validate_password_with_policy(
    pool: &PgPool,
    password: &str,
) -> Result<PasswordValidationResult, AppError> {
    let p = settings::get_password_policy(pool).await?;
    let policy = PasswordPolicy {
        min_length: p.min_length as usize,
        require_number: p.require_number,
        require_uppercase: p.require_uppercase,
        require_special_char: p.require_special_char,
    };
    Ok(validate_password(password, &policy))
}

/// 检查该标识（邮箱小写）当前是否处于锁定期。
pub async fn check_login_lockout(
    pool: &PgPool,
    identifier: &str,
) -> Result<LockoutStatus, AppError> {
    let record = login_attempt::get_login_attempt_by_identifier(pool, identifier).await?;
    let Some(record) = record else {
        return Ok(LockoutStatus { locked: false, remaining_seconds: 0 });
    };
    let Some(locked_until) = record.locked_until else {
        return Ok(LockoutStatus { locked: false, remaining_seconds: 0 });
    };

    let now = Utc::now();
    if locked_until > now {
        // 与 TS 的 Math.ceil(ms / 1000) 对齐
        let ms = (locked_until - now).num_milliseconds().max(0);
        let remaining = ((ms as f64) / 1000.0).ceil() as u64;
        return Ok(LockoutStatus { locked: true, remaining_seconds: remaining });
    }

    Ok(LockoutStatus { locked: false, remaining_seconds: 0 })
}

/// 记录一次失败登录；达到阈值则写入锁定截止时间。
pub async fn record_failed_login(
    pool: &PgPool,
    identifier: &str,
    user_id: Option<&str>,
) -> Result<LockoutStatus, AppError> {
    let record = login_attempt::record_login_failure(pool, identifier, user_id).await?;
    let p = settings::get_login_lockout_policy(pool).await?;
    let policy = LoginLockoutPolicy {
        max_attempts: p.max_attempts,
        lockout_minutes: p.lockout_minutes,
    };

    let status = evaluate_lockout(record.attempts.max(0) as u32, &policy);
    if status.locked {
        let locked_until = Utc::now() + Duration::minutes(policy.lockout_minutes as i64);
        login_attempt::set_login_attempt_locked(pool, identifier, locked_until).await?;
    }
    Ok(status)
}

/// 登录成功：清除失败计数。
pub async fn record_successful_login(pool: &PgPool, identifier: &str) -> Result<(), AppError> {
    login_attempt::clear_login_attempt(pool, identifier).await
}
