//! 通知偏好覆盖仓储：只存相对默认的差异行。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct PreferenceOverrideRow {
    pub user_id: String,
    pub category: String,
    pub channel: String,
    pub enabled: bool,
    pub updated_at: DateTime<Utc>,
}

pub async fn list_overrides(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<PreferenceOverrideRow>, AppError> {
    let rows = sqlx::query_as::<_, PreferenceOverrideRow>(
        r#"SELECT user_id, category, channel, enabled, updated_at
           FROM notification_preference_override
           WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn upsert_override(
    pool: &PgPool,
    user_id: &str,
    category: &str,
    channel: &str,
    enabled: bool,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO notification_preference_override (user_id, category, channel, enabled, updated_at)
           VALUES ($1, $2, $3, $4, NOW())
           ON CONFLICT (user_id, category, channel)
           DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = NOW()"#,
    )
    .bind(user_id)
    .bind(category)
    .bind(channel)
    .bind(enabled)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_override(
    pool: &PgPool,
    user_id: &str,
    category: &str,
    channel: &str,
) -> Result<(), AppError> {
    sqlx::query(
        r#"DELETE FROM notification_preference_override
           WHERE user_id = $1 AND category = $2 AND channel = $3"#,
    )
    .bind(user_id)
    .bind(category)
    .bind(channel)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn clear_overrides(pool: &PgPool, user_id: &str) -> Result<(), AppError> {
    sqlx::query(r#"DELETE FROM notification_preference_override WHERE user_id = $1"#)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}
