//! project 表访问（对齐 `projectRepository.ts`）。
//!
//! 兼容要点：
//! - `status` 是 PG 枚举 `ProjectStatus`，绑定需 `::"ProjectStatus"`；应用层已不暴露该字段，
//!   新建统一写 `ACTIVE`（与旧 `createProject` 一致，注意 **不是** schema 默认的 `INITIALIZING`）。
//! - `updated_at` 是 Prisma `@updatedAt`（应用层维护，DDL 无 DEFAULT），INSERT/UPDATE 必须显式写。
//! - 主键 `nanoid(16)` 由应用层生成。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;
use crate::core::js_number::Pagination;

/// secret-key-info 与项目成员域都需要的最小投影。
#[derive(Debug, Clone, FromRow)]
pub struct ProjectBrief {
    pub id: String,
    pub name: String,
    /// 项目创建者（owner）的 user_id，用于成员域判 owner。
    pub user_id: String,
}

/// `serializeProject` 所需的完整投影。
#[derive(Debug, Clone, FromRow)]
pub struct ProjectRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub secret_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const PROJECT_COLS: &str =
    "id, user_id, name, description, secret_key, created_at, updated_at";

/// getProjectByIdOnly：取 secret-key-info / 成员域需要的字段。
pub async fn get_project_by_id_only(
    pool: &PgPool,
    id: &str,
) -> Result<Option<ProjectBrief>, AppError> {
    let row = sqlx::query_as::<_, ProjectBrief>(
        r#"SELECT id, name, user_id FROM project WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// getProjectByIdOnly 的完整投影版本（secret-key 路由需要读 secret_key）。
pub async fn get_project_row_by_id(
    pool: &PgPool,
    id: &str,
) -> Result<Option<ProjectRow>, AppError> {
    let sql = format!("SELECT {PROJECT_COLS} FROM project WHERE id = $1");
    let row = sqlx::query_as::<_, ProjectRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// listProjectsByUser：ADMIN 看全量，普通用户看「自己创建的 ∪ 参与的」。
///
/// `pagination` 为 `None` 时返回全量（对齐旧实现 page/pageSize 落 falsy 的分支）。
pub async fn list_projects_by_user(
    pool: &PgPool,
    user_id: &str,
    is_admin: bool,
    pagination: Option<Pagination>,
) -> Result<(Vec<ProjectRow>, i64), AppError> {
    // 旧实现先查 memberProjectIds 再用 OR 拼 where；这里等价改写成 EXISTS 子查询，
    // 语义相同但少一次往返，且避免 id IN (超长列表) 的参数膨胀。
    let (where_clause, bind_user) = if is_admin {
        ("TRUE".to_string(), false)
    } else {
        (
            "(p.user_id = $1 OR EXISTS (\
               SELECT 1 FROM project_member pm \
               WHERE pm.project_id = p.id AND pm.user_id = $1))"
                .to_string(),
            true,
        )
    };

    let count_sql = format!("SELECT COUNT(*) FROM project p WHERE {where_clause}");
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql);
    if bind_user {
        count_q = count_q.bind(user_id);
    }
    let total: i64 = count_q.fetch_one(pool).await?;

    let cols = PROJECT_COLS
        .split(", ")
        .map(|c| format!("p.{c}"))
        .collect::<Vec<_>>()
        .join(", ");

    let rows = match pagination {
        None => {
            let sql =
                format!("SELECT {cols} FROM project p WHERE {where_clause} ORDER BY p.updated_at DESC");
            let mut q = sqlx::query_as::<_, ProjectRow>(&sql);
            if bind_user {
                q = q.bind(user_id);
            }
            q.fetch_all(pool).await?
        }
        Some(window) => {
            // Prisma 的负 take 是「反向取 N 条，再把结果还原成原排序」，
            // 对应 SQL 层把 ORDER BY 翻向 + 内存里 reverse。
            let (flipped, limit) = window.sql_window();
            let dir = if flipped { "ASC" } else { "DESC" };
            let (p1, p2) = if bind_user { ("$2", "$3") } else { ("$1", "$2") };
            let sql = format!(
                "SELECT {cols} FROM project p WHERE {where_clause} \
                 ORDER BY p.updated_at {dir} LIMIT {p2} OFFSET {p1}"
            );
            let mut q = sqlx::query_as::<_, ProjectRow>(&sql);
            if bind_user {
                q = q.bind(user_id);
            }
            let mut rows = q.bind(window.skip).bind(limit).fetch_all(pool).await?;
            if flipped {
                rows.reverse();
            }
            rows
        }
    };

    Ok((rows, total))
}

/// getProjectById：ADMIN 直取；否则限制在「创建者 ∪ 成员」范围内。
pub async fn get_project_by_id(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<Option<ProjectRow>, AppError> {
    if is_admin {
        return get_project_row_by_id(pool, id).await;
    }
    let sql = format!(
        "SELECT {} FROM project p \
         WHERE p.id = $1 AND (p.user_id = $2 OR EXISTS (\
            SELECT 1 FROM project_member pm WHERE pm.project_id = p.id AND pm.user_id = $2))",
        PROJECT_COLS
            .split(", ")
            .map(|c| format!("p.{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let row = sqlx::query_as::<_, ProjectRow>(&sql)
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn create_project(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<ProjectRow, AppError> {
    let sql = format!(
        r#"INSERT INTO project (id, user_id, name, description, status, created_at, updated_at)
           VALUES ($1, $2, $3, $4, 'ACTIVE'::"ProjectStatus", NOW(), NOW())
           RETURNING {PROJECT_COLS}"#
    );
    let row = sqlx::query_as::<_, ProjectRow>(&sql)
        .bind(nanoid(16))
        .bind(user_id)
        .bind(name)
        .bind(description)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// updateProjectById：ADMIN 直改；否则要求创建者或 OWNER/ADMIN 成员。
///
/// `name` / `description` 为 `None` 表示「本次不更新该字段」（对齐 Prisma 的 undefined 语义）。
pub async fn update_project_by_id(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    is_admin: bool,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<Option<ProjectRow>, AppError> {
    if !is_admin {
        let allowed: bool = sqlx::query_scalar(
            r#"SELECT EXISTS (
                 SELECT 1 FROM project p
                 WHERE p.id = $1 AND (
                   p.user_id = $2
                   OR EXISTS (SELECT 1 FROM project_member pm
                              WHERE pm.project_id = p.id AND pm.user_id = $2
                                AND pm.role IN ('OWNER','ADMIN'))
                 ))"#,
        )
        .bind(id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        if !allowed {
            return Ok(None);
        }
    }

    // COALESCE 无法区分「传 null」与「不传」，故用参数化的布尔开关显式表达 undefined。
    let sql = format!(
        r#"UPDATE project SET
             name = CASE WHEN $2 THEN $3 ELSE name END,
             description = CASE WHEN $4 THEN $5 ELSE description END,
             updated_at = NOW()
           WHERE id = $1
           RETURNING {PROJECT_COLS}"#
    );
    let row = sqlx::query_as::<_, ProjectRow>(&sql)
        .bind(id)
        .bind(name.is_some())
        .bind(name)
        .bind(description.is_some())
        .bind(description)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// deleteProjectById：ADMIN 可删任意；普通用户**仅限自己创建的**（成员身份不够）。
pub async fn delete_project_by_id(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<Option<ProjectRow>, AppError> {
    let sql = if is_admin {
        format!("DELETE FROM project WHERE id = $1 RETURNING {PROJECT_COLS}")
    } else {
        format!("DELETE FROM project WHERE id = $1 AND user_id = $2 RETURNING {PROJECT_COLS}")
    };
    let mut q = sqlx::query_as::<_, ProjectRow>(&sql).bind(id);
    if !is_admin {
        q = q.bind(user_id);
    }
    let row = q.fetch_optional(pool).await?;
    Ok(row)
}

pub async fn set_project_secret_key(
    pool: &PgPool,
    id: &str,
    key: &str,
) -> Result<ProjectRow, AppError> {
    let sql = format!(
        "UPDATE project SET secret_key = $2, updated_at = NOW() WHERE id = $1 \
         RETURNING {PROJECT_COLS}"
    );
    let row = sqlx::query_as::<_, ProjectRow>(&sql)
        .bind(id)
        .bind(key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::internal("项目不存在，无法写入 secret key"))?;
    Ok(row)
}

/// 注意：项目不存在时旧实现的 `prisma.update` 会抛 P2025（未捕获 → 500），
/// 所以这里必须把「0 行受影响」也当成错误，否则平台 ADMIN 撤销不存在项目的密钥
/// 会在新后端拿到 200，与旧后端产生 DIFF。
pub async fn clear_project_secret_key(pool: &PgPool, id: &str) -> Result<(), AppError> {
    let res = sqlx::query("UPDATE project SET secret_key = NULL, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::internal("项目不存在，无法撤销 secret key"));
    }
    Ok(())
}
