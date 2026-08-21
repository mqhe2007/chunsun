//! 邀请码仓储。
//!
//! - 列表按 `created_at DESC`；`code` 为 16 随机字节的 hex（32 字符）。
//! - 删除不存在的 id 时返回 0 行，handler 映射为 500。

use crate::api::AppError;
use crate::core::datetime::to_value as dt_value;
use crate::core::ids::nanoid16;
use chrono::{DateTime, Utc};
use rand::Rng;
use serde_json::Value;
use sqlx::{FromRow, PgPool};

const INVITATION_COLS: &str =
    "id, code, inviter_id, role::text AS role, max_uses, used_count, expires_at, created_at";

#[derive(Debug, Clone, FromRow)]
pub struct InvitationRow {
    pub id: String,
    pub code: String,
    pub inviter_id: String,
    pub role: String,
    pub max_uses: i32,
    pub used_count: i32,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub fn generate_invitation_code() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub async fn list_invitation_codes(pool: &PgPool) -> Result<Vec<InvitationRow>, AppError> {
    let sql = format!(
        "SELECT {INVITATION_COLS} FROM invitation_code ORDER BY created_at DESC"
    );
    let rows = sqlx::query_as::<_, InvitationRow>(&sql)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn create_invitation_code(
    pool: &PgPool,
    code: &str,
    inviter_id: &str,
    role: &str,
    max_uses: i32,
    expires_at: Option<DateTime<Utc>>,
) -> Result<InvitationRow, AppError> {
    let id = nanoid16();
    let sql = format!(
        "INSERT INTO invitation_code (id, code, inviter_id, role, max_uses, expires_at)
         VALUES ($1, $2, $3, $4::\"UserRole\", $5, $6)
         RETURNING {INVITATION_COLS}"
    );
    let row = sqlx::query_as::<_, InvitationRow>(&sql)
        .bind(id)
        .bind(code)
        .bind(inviter_id)
        .bind(role)
        .bind(max_uses)
        .bind(expires_at)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub async fn delete_invitation_code(pool: &PgPool, id: &str) -> Result<u64, AppError> {
    let n = sqlx::query(r#"DELETE FROM invitation_code WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(n)
}

pub async fn get_valid_invitation_code_by_code(
    pool: &PgPool,
    code: &str,
) -> Result<Option<InvitationRow>, AppError> {
    let sql = format!(
        "SELECT {INVITATION_COLS} FROM invitation_code WHERE code = $1"
    );
    let row = sqlx::query_as::<_, InvitationRow>(&sql)
        .bind(code)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|inv| {
        if let Some(exp) = inv.expires_at {
            if exp < Utc::now() {
                return None;
            }
        }
        if inv.used_count >= inv.max_uses {
            return None;
        }
        Some(inv)
    }))
}

pub async fn increment_invitation_used_count(
    pool: &PgPool,
    id: &str,
) -> Result<InvitationRow, AppError> {
    let sql = format!(
        "UPDATE invitation_code SET used_count = used_count + 1
         WHERE id = $1
         RETURNING {INVITATION_COLS}"
    );
    let row = sqlx::query_as::<_, InvitationRow>(&sql)
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

pub fn invitation_dto(d: &InvitationRow) -> Value {
    serde_json::json!({
        "id": d.id,
        "code": d.code,
        "inviterId": d.inviter_id,
        "role": d.role,
        "maxUses": d.max_uses,
        "usedCount": d.used_count,
        "expiresAt": d.expires_at.map_or(Value::Null, |dt| dt_value(&dt)),
        "createdAt": dt_value(&d.created_at),
    })
}
