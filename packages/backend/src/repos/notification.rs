//! 通知仓储。
//!
//! - `unreadOnly` 为真时加 `is_read = false`；分页仅在 page 与 pageSize 均 truthy 时生效。
//! - `mark_as_read`：id + userId 双条件，无匹配行返回 None。

use crate::api::AppError;
use crate::core::datetime::to_value as dt_value;
use crate::core::ids::nanoid16;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};

const NOTIFICATION_COLS: &str =
    "id, type, title, body, link, is_read, read_at, created_at";

#[derive(Debug, Clone, FromRow)]
pub struct NotificationRow {
    pub id: String,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub title: String,
    pub body: Option<String>,
    pub link: Option<String>,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub fn notification_dto(n: &NotificationRow) -> Value {
    serde_json::json!({
        "id": n.id,
        "type": n.type_,
        "title": n.title,
        "body": n.body,
        "link": n.link,
        "isRead": n.is_read,
        "readAt": n.read_at.map_or(Value::Null, |dt| dt_value(&dt)),
        "createdAt": dt_value(&n.created_at),
    })
}

pub async fn list_notifications(
    pool: &PgPool,
    user_id: &str,
    unread_only: bool,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<(Vec<NotificationRow>, i64), AppError> {
    let mut conds = String::from("WHERE user_id = $1");
    if unread_only {
        conds.push_str(" AND is_read = false");
    }
    let total: (i64,) = sqlx::query_as(&format!(
        r#"SELECT COUNT(*)::bigint FROM notification {conds}"#
    ))
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    let mut sql = format!(
        "SELECT {NOTIFICATION_COLS} FROM notification {conds} ORDER BY created_at DESC"
    );
    let skip = match (page, page_size) {
        (Some(page), Some(page_size)) => {
            sql.push_str(" LIMIT $2 OFFSET $3");
            Some((page - 1).max(0) * page_size)
        }
        _ => None,
    };
    let mut q = sqlx::query_as::<_, NotificationRow>(&sql).bind(user_id);
    if let (Some(skip), Some(page_size)) = (skip, page_size) {
        q = q.bind(page_size).bind(skip);
    }
    let items = q.fetch_all(pool).await?;
    Ok((items, total.0))
}

pub async fn count_unread(pool: &PgPool, user_id: &str) -> Result<i64, AppError> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*)::bigint FROM notification WHERE user_id = $1 AND is_read = false"#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn mark_as_read(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<NotificationRow>, AppError> {
    let sql = format!(
        "UPDATE notification SET is_read = true, read_at = NOW()
         WHERE id = $1 AND user_id = $2
         RETURNING {NOTIFICATION_COLS}"
    );
    let row = sqlx::query_as::<_, NotificationRow>(&sql)
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn mark_all_as_read(pool: &PgPool, user_id: &str) -> Result<(), AppError> {
    sqlx::query(r#"UPDATE notification SET is_read = true, read_at = NOW() WHERE user_id = $1 AND is_read = false"#)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct NotificationInput {
    pub user_id: String,
    pub ty: String,
    pub title: String,
    pub body: Option<String>,
    pub link: Option<String>,
}

pub async fn create_notification(pool: &PgPool, input: NotificationInput) -> Result<(), AppError> {
    let id = nanoid16();
    sqlx::query(
        r#"INSERT INTO notification (id, user_id, "type", title, body, link)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(id)
    .bind(input.user_id)
    .bind(input.ty)
    .bind(input.title)
    .bind(input.body)
    .bind(input.link)
    .execute(pool)
    .await?;
    Ok(())
}
