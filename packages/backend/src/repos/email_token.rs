//! 邮箱验证 / 密码重置 token 表访问。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

const EMAIL_VERIFICATION_TOKEN_COLS: &str = "id, user_id, expires_at, used_at";
const PASSWORD_RESET_TOKEN_COLS: &str = "id, user_id, expires_at, used_at";

#[derive(Debug, Clone, FromRow)]
pub struct EmailVerificationToken {
    pub id: String,
    pub user_id: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PasswordResetToken {
    pub id: String,
    pub user_id: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

pub async fn create_email_verification_token(
    pool: &PgPool,
    user_id: &str,
    token: &str,
    expires_at: DateTime<Utc>,
) -> Result<EmailVerificationToken, AppError> {
    let sql = format!(
        "INSERT INTO email_verification_token (id, user_id, token, expires_at)
         VALUES ($4, $1, $2, $3) RETURNING {EMAIL_VERIFICATION_TOKEN_COLS}"
    );
    let row = sqlx::query_as::<_, EmailVerificationToken>(&sql)
        .bind(user_id)
        .bind(token)
        .bind(expires_at)
        .bind(nanoid(16))
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn get_email_verification_token_by_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<EmailVerificationToken>, AppError> {
    let sql = format!(
        "SELECT {EMAIL_VERIFICATION_TOKEN_COLS} FROM email_verification_token WHERE token = $1"
    );
    let row = sqlx::query_as::<_, EmailVerificationToken>(&sql)
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn mark_email_verification_token_used(pool: &PgPool, id: &str) -> Result<(), AppError> {
    sqlx::query(r#"UPDATE email_verification_token SET used_at = now() WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_password_reset_token(
    pool: &PgPool,
    user_id: &str,
    token: &str,
    expires_at: DateTime<Utc>,
) -> Result<PasswordResetToken, AppError> {
    let sql = format!(
        "INSERT INTO password_reset_token (id, user_id, token, expires_at)
         VALUES ($4, $1, $2, $3) RETURNING {PASSWORD_RESET_TOKEN_COLS}"
    );
    let row = sqlx::query_as::<_, PasswordResetToken>(&sql)
        .bind(user_id)
        .bind(token)
        .bind(expires_at)
        .bind(nanoid(16))
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn get_password_reset_token_by_token(
    pool: &PgPool,
    token: &str,
) -> Result<Option<PasswordResetToken>, AppError> {
    let sql = format!(
        "SELECT {PASSWORD_RESET_TOKEN_COLS} FROM password_reset_token WHERE token = $1"
    );
    let row = sqlx::query_as::<_, PasswordResetToken>(&sql)
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn mark_password_reset_token_used(pool: &PgPool, id: &str) -> Result<(), AppError> {
    sqlx::query(r#"UPDATE password_reset_token SET used_at = now() WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
