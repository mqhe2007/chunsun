//! project_activity 表访问（对齐 `projectActivityRepository.ts` 与 `routes/activity.ts`）。
//!
//! 该表只有 `created_at`，没有 `updated_at`；主键 `nanoid(10)`。
//!
//! 读取侧两条查询直接写在 `routes/activity.ts` 里（旧实现没走 repository），
//! 这里统一收进仓储层：
//! - [`list_project_activities`]：按 `created_at DESC` 取 `take` 条，`include: { user }`。
//!   `take` 允许为负（Prisma 反向取），走 [`Pagination::sql_window`]。
//! - [`list_activity_created_ats`]：`select: { createdAt: true }` 的区间扫描，供活跃图聚合。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;
use crate::core::js_number::Pagination;

pub struct CreateProjectActivityInput<'a> {
    pub project_id: &'a str,
    pub user_id: &'a str,
    pub action: &'a str,
    pub description: &'a str,
    pub entity_type: Option<&'a str>,
    pub entity_id: Option<&'a str>,
    pub metadata: Option<serde_json::Value>,
}

pub async fn create_project_activity(
    pool: &PgPool,
    input: CreateProjectActivityInput<'_>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO project_activity
             (id, project_id, user_id, action, entity_type, entity_id, description, metadata, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())"#,
    )
    .bind(nanoid(10))
    .bind(input.project_id)
    .bind(input.user_id)
    .bind(input.action)
    .bind(input.entity_type)
    .bind(input.entity_id)
    .bind(input.description)
    .bind(input.metadata)
    .execute(pool)
    .await?;
    Ok(())
}

// ── 读取侧 ──────────────────────────────────────────────────────────────

/// `include: { user: { select: { id, nickname, qq } } }` 折叠出来的关联用户。
///
/// `user_id` 是非空外键且 `onDelete: Cascade`，Prisma 按**必填关系**生成 INNER JOIN，
/// 这里照做（不做 LEFT JOIN + Option 折叠）。
#[derive(Debug, Clone)]
pub struct ActivityUser {
    pub id: String,
    pub nickname: Option<String>,
    pub qq: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectActivityRow {
    pub id: String,
    pub action: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub description: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub user: ActivityUser,
}

#[derive(Debug, Clone, FromRow)]
struct ActivityJoinRow {
    id: String,
    action: String,
    entity_type: Option<String>,
    entity_id: Option<String>,
    description: String,
    metadata: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    u_id: String,
    u_nickname: Option<String>,
    u_qq: Option<String>,
}

impl ActivityJoinRow {
    fn into_row(self) -> ProjectActivityRow {
        ProjectActivityRow {
            id: self.id,
            action: self.action,
            entity_type: self.entity_type,
            entity_id: self.entity_id,
            description: self.description,
            metadata: self.metadata,
            created_at: self.created_at,
            user: ActivityUser {
                id: self.u_id,
                nickname: self.u_nickname,
                qq: self.u_qq,
            },
        }
    }
}

const ACTIVITY_COLS: &str = "a.id, a.action, a.entity_type, a.entity_id, a.description, \
     a.metadata, a.created_at, \
     u.id AS u_id, u.nickname AS u_nickname, u.qq AS u_qq";

/// 项目活动清单：`orderBy: { createdAt: "desc" }` + `take`。
///
/// `take` 为负时是 Prisma 的「从末尾往回取 N 条、结果仍按原排序」语义，
/// 对应 SQL 把 ORDER BY 翻向再在内存里 reverse 还原。
pub async fn list_project_activities(
    pool: &PgPool,
    project_id: &str,
    take: i64,
) -> Result<Vec<ProjectActivityRow>, AppError> {
    let (flipped, limit) = Pagination { skip: 0, take }.sql_window();
    let dir = if flipped { "ASC" } else { "DESC" };
    let sql = format!(
        "SELECT {ACTIVITY_COLS} FROM project_activity a \
         JOIN \"user\" u ON u.id = a.user_id \
         WHERE a.project_id = $1 \
         ORDER BY a.created_at {dir} LIMIT $2"
    );
    let mut rows = sqlx::query_as::<_, ActivityJoinRow>(&sql)
        .bind(project_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
    if flipped {
        rows.reverse();
    }
    Ok(rows.into_iter().map(ActivityJoinRow::into_row).collect())
}

/// 活跃图数据源：`select: { createdAt: true }`，`createdAt ∈ [start, end_exclusive)`。
/// 不排序（旧实现也没给 orderBy，聚合是纯函数按天分桶，与顺序无关）。
pub async fn list_activity_created_ats(
    pool: &PgPool,
    project_id: &str,
    start: DateTime<Utc>,
    end_exclusive: DateTime<Utc>,
) -> Result<Vec<DateTime<Utc>>, AppError> {
    let rows: Vec<(DateTime<Utc>,)> = sqlx::query_as(
        "SELECT a.created_at FROM project_activity a \
         WHERE a.project_id = $1 AND a.created_at >= $2 AND a.created_at < $3",
    )
    .bind(project_id)
    .bind(start)
    .bind(end_exclusive)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}
