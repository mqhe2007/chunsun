//! 登录尝试 / 锁定表访问。失败计数用 ON CONFLICT upsert，避免先查后写的竞态。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// 业务只读 attempts / locked_until；其余列由 upsert 维护。
const LOGIN_ATTEMPT_COLS: &str = "attempts, locked_until";

#[derive(Debug, Clone, FromRow)]
pub struct LoginAttempt {
    pub attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
}

pub async fn get_login_attempt_by_identifier(
    pool: &PgPool,
    identifier: &str,
) -> Result<Option<LoginAttempt>, AppError> {
    let sql = format!("SELECT {LOGIN_ATTEMPT_COLS} FROM login_attempt WHERE identifier = $1");
    let row = sqlx::query_as::<_, LoginAttempt>(&sql)
        .bind(identifier)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn record_login_failure(
    pool: &PgPool,
    identifier: &str,
    user_id: Option<&str>,
) -> Result<LoginAttempt, AppError> {
    let sql = format!(
        "INSERT INTO login_attempt (id, identifier, attempts, last_attempt_at, user_id, updated_at)
         VALUES ($3, $1, 1, now(), $2, now())
         ON CONFLICT (identifier) DO UPDATE
           SET attempts = login_attempt.attempts + 1,
               last_attempt_at = now(),
               updated_at = now(),
               user_id = COALESCE(EXCLUDED.user_id, login_attempt.user_id)
         RETURNING {LOGIN_ATTEMPT_COLS}"
    );
    let row = sqlx::query_as::<_, LoginAttempt>(&sql)
        .bind(identifier)
        .bind(user_id)
        .bind(nanoid(16))
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn set_login_attempt_locked(
    pool: &PgPool,
    identifier: &str,
    locked_until: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"UPDATE login_attempt SET locked_until = $2, updated_at = now() WHERE identifier = $1"#,
    )
        .bind(identifier)
        .bind(locked_until)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn clear_login_attempt(pool: &PgPool, identifier: &str) -> Result<(), AppError> {
    sqlx::query(r#"DELETE FROM login_attempt WHERE identifier = $1"#)
        .bind(identifier)
        .execute(pool)
        .await?;
    Ok(())
}
