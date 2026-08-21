//! user 表访问（对齐 `userRepository.ts`）。
//!
//! 三个存量兼容要点（由 information_schema 内省确认，非猜测）：
//! 1. `id TEXT NOT NULL` 无 DB DEFAULT —— Prisma 的 `@default(nanoid(16))` 是应用层默认值，
//!    因此 INSERT 必须显式生成主键。
//! 2. `updated_at TIMESTAMPTZ NOT NULL` 无 DB DEFAULT —— Prisma 的 `@updatedAt` 同样在应用层，
//!    因此 INSERT/UPDATE 都必须显式写入。
//! 3. `role`/`status` 是 PG 枚举类型（`UserRole`/`UserStatus`）—— 绑定文本需显式 `::"UserRole"`，
//!    读取时用 `::text` 投影回字符串，避免 sqlx 解码失败。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// 读取投影：枚举列统一转 text，列顺序固定，避免 `SELECT *` 的隐式依赖。
const USER_COLS: &str = r#"id, email, password, qq, nickname,
    role::text AS role, status::text AS status, email_verified, created_at, updated_at"#;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password: String,
    pub qq: Option<String>,
    pub nickname: Option<String>,
    pub role: String,
    pub status: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct CreateUserInput {
    pub email: String,
    pub password: String,
    pub qq: Option<String>,
    pub nickname: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

pub async fn create_user(pool: &PgPool, input: CreateUserInput) -> Result<User, AppError> {
    create_user_raw(pool, input).await.map_err(AppError::from)
}

pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, AppError> {
    let sql = format!(r#"SELECT {USER_COLS} FROM "user" WHERE email = $1"#);
    let user = sqlx::query_as::<_, User>(&sql)
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn get_user_by_id(pool: &PgPool, id: &str) -> Result<Option<User>, AppError> {
    let sql = format!(r#"SELECT {USER_COLS} FROM "user" WHERE id = $1"#);
    let user = sqlx::query_as::<_, User>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn update_user_email_verified(
    pool: &PgPool,
    id: &str,
    verified: bool,
) -> Result<(), AppError> {
    sqlx::query(r#"UPDATE "user" SET email_verified = $2, updated_at = NOW() WHERE id = $1"#)
        .bind(id)
        .bind(verified)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_user_password(pool: &PgPool, id: &str, password: &str) -> Result<(), AppError> {
    sqlx::query(r#"UPDATE "user" SET password = $2, updated_at = NOW() WHERE id = $1"#)
        .bind(id)
        .bind(password)
        .execute(pool)
        .await?;
    Ok(())
}

/// 部分更新昵称/QQ：`None` 字段不改列（对齐 Prisma `undefined` 忽略语义，用 `COALESCE` 实现）。
pub async fn update_user_profile(
    pool: &PgPool,
    id: &str,
    nickname: Option<&str>,
    qq: Option<&str>,
) -> Result<User, AppError> {
    let sql = format!(
        r#"UPDATE "user"
           SET nickname = COALESCE($2, nickname), qq = COALESCE($3, qq), updated_at = NOW()
           WHERE id = $1
           RETURNING {USER_COLS}"#
    );
    let user = sqlx::query_as::<_, User>(&sql)
        .bind(id)
        .bind(nickname)
        .bind(qq)
        .fetch_one(pool)
        .await?;
    Ok(user)
}

pub async fn update_user_status(
    pool: &PgPool,
    id: &str,
    status: &str,
) -> Result<User, AppError> {
    let sql = format!(
        r#"UPDATE "user" SET status = $2::"UserStatus", updated_at = NOW() WHERE id = $1
           RETURNING {USER_COLS}"#
    );
    let user = sqlx::query_as::<_, User>(&sql)
        .bind(id)
        .bind(status)
        .fetch_one(pool)
        .await?;
    Ok(user)
}

pub async fn update_user_role(pool: &PgPool, id: &str, role: &str) -> Result<User, AppError> {
    let sql = format!(
        r#"UPDATE "user" SET role = $2::"UserRole", updated_at = NOW() WHERE id = $1
           RETURNING {USER_COLS}"#
    );
    let user = sqlx::query_as::<_, User>(&sql)
        .bind(id)
        .bind(role)
        .fetch_one(pool)
        .await?;
    Ok(user)
}

/// 模糊搜索（ILIKE `%q%` on email/nickname）+ 仅 ACTIVE + 排除自身，按 nickname 升序。
pub async fn search_users(
    pool: &PgPool,
    q: &str,
    exclude_id: &str,
    limit: i64,
) -> Result<Vec<User>, AppError> {
    let pattern = format!("%{q}%");
    let sql = format!(
        r#"SELECT {USER_COLS} FROM "user"
           WHERE (email ILIKE $1 OR nickname ILIKE $1)
             AND status = 'ACTIVE'::"UserStatus"
             AND id <> $2
           ORDER BY nickname ASC NULLS LAST, email ASC
           LIMIT $3"#
    );
    let rows = sqlx::query_as::<_, User>(&sql)
        .bind(&pattern)
        .bind(exclude_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub struct UserListResult {
    pub items: Vec<User>,
    pub total: i64,
}

/// 管理员用户列表（分页）。page/pageSize 由调用方保证 ≥1。
pub async fn list_all_users(
    pool: &PgPool,
    page: i64,
    page_size: i64,
) -> Result<UserListResult, AppError> {
    let page = page.max(1);
    let page_size = page_size.max(1);
    let offset = (page - 1) * page_size;
    let total: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "user""#)
        .fetch_one(pool)
        .await?;
    let sql = format!(
        r#"SELECT {USER_COLS} FROM "user" ORDER BY created_at DESC LIMIT $1 OFFSET $2"#
    );
    let items = sqlx::query_as::<_, User>(&sql)
        .bind(page_size)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    Ok(UserListResult { items, total })
}

pub async fn delete_user_by_id(pool: &PgPool, id: &str) -> Result<(), AppError> {
    sqlx::query(r#"DELETE FROM "user" WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 原始插入（返回 sqlx 错误），供上层区分唯一约束冲突（→ 409）与其余 DB 错误。
pub async fn create_user_raw(
    pool: &PgPool,
    input: CreateUserInput,
) -> Result<User, sqlx::Error> {
    let role = input.role.as_deref().unwrap_or("USER").to_string();
    let status = input.status.as_deref().unwrap_or("ACTIVE").to_string();
    let sql = format!(
        r#"INSERT INTO "user" (id, email, password, qq, nickname, role, status, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6::"UserRole", $7::"UserStatus", NOW())
           RETURNING {USER_COLS}"#
    );
    let user = sqlx::query_as::<_, User>(&sql)
        .bind(nanoid(16))
        .bind(&input.email)
        .bind(&input.password)
        .bind(&input.qq)
        .bind(&input.nickname)
        .bind(role)
        .bind(status)
        .fetch_one(pool)
        .await?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与 `register_user` 对齐：无邀请码时 role=None，落库前默认 USER。
    #[test]
    fn omitted_role_resolves_to_user() {
        let input = CreateUserInput {
            email: "a@example.com".into(),
            password: "hash".into(),
            ..Default::default()
        };
        assert!(input.role.is_none());
        assert_eq!(input.role.as_deref().unwrap_or("USER"), "USER");
    }
}
