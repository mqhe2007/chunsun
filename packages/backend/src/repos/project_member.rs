//! project_member 表访问（对齐 `projectMemberRepository.ts`）。
//!
//! 兼容要点同 `user.rs`：枚举列 `role` 需 `::"ProjectMemberRole"` 绑定 / `::text` 投影；
//! `updated_at` 无 DB DEFAULT，INSERT 必须显式 `NOW()`；主键由应用层 `nanoid(16)` 生成。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// 读取投影：`pm` 为成员列，`u` 为 JOIN 的 user 列（别名避免与 `pm.user_id` 冲突）。
const MEMBER_COLS: &str = r#"pm.id AS id,
    pm.user_id AS user_id,
    pm.role::text AS role,
    pm.created_at AS created_at,
    u.id AS u_id,
    u.email AS u_email,
    u.nickname AS u_nickname,
    u.qq AS u_qq"#;

#[derive(Debug, Clone, FromRow)]
pub struct MemberWithUser {
    pub id: String,
    pub user_id: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub u_id: String,
    pub u_email: String,
    pub u_nickname: Option<String>,
    pub u_qq: Option<String>,
}

pub async fn get_project_member(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<Option<MemberWithUser>, AppError> {
    let sql = format!(
        r#"SELECT {MEMBER_COLS}
           FROM project_member pm
           JOIN "user" u ON u.id = pm.user_id
           WHERE pm.project_id = $1 AND pm.user_id = $2"#
    );
    let row = sqlx::query_as::<_, MemberWithUser>(&sql)
        .bind(project_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn list_project_members(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<MemberWithUser>, AppError> {
    let sql = format!(
        r#"SELECT {MEMBER_COLS}
           FROM project_member pm
           JOIN "user" u ON u.id = pm.user_id
           WHERE pm.project_id = $1
           ORDER BY pm.created_at DESC"#
    );
    let rows = sqlx::query_as::<_, MemberWithUser>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn add_project_member(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    role: &str,
) -> Result<MemberWithUser, AppError> {
    let insert = r#"INSERT INTO project_member (id, project_id, user_id, role, updated_at)
        VALUES ($1, $2, $3, $4::"ProjectMemberRole", NOW())"#;
    sqlx::query(insert)
        .bind(nanoid(16))
        .bind(project_id)
        .bind(user_id)
        .bind(role)
        .execute(pool)
        .await?;
    // INSERT 不能 JOIN 返回，单独回查拼接后的完整投影
    get_project_member(pool, project_id, user_id)
        .await?
        .ok_or_else(|| AppError::internal("成员写入后回查失败"))
}

pub async fn update_project_member_role(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    role: &str,
) -> Result<Option<MemberWithUser>, AppError> {
    sqlx::query(
        r#"UPDATE project_member SET role = $3::"ProjectMemberRole", updated_at = NOW()
           WHERE project_id = $1 AND user_id = $2"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await?;
    get_project_member(pool, project_id, user_id).await
}

pub async fn remove_project_member(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<bool, AppError> {
    let res = sqlx::query(
        r#"DELETE FROM project_member WHERE project_id = $1 AND user_id = $2"#,
    )
    .bind(project_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}
