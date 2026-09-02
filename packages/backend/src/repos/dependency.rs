//! dependency 边表访问（需求/缺陷 Blocking / Blocked By 依赖关系）。
//!
//! 兼容要点（对齐本仓库 repo 约定）：
//! - 主键 `nanoid(12)`（与 requirement / defect 一致）。
//! - 边方向语义：`source blocks target`（source 不完成，target 无法开始）。
//!   即 `Blocking` 视角 = 从当前节点出发的出边（source=当前，target=被阻塞者）；
//!   `Blocked By` 视角 = 指向当前节点的入边（source=阻塞者，target=当前）。
//! - 多态引用：`source_type` / `target_type` ∈ {requirement, defect}，不建 FK。
//!   删除需求/缺陷时应用层级联清理相关边（见 `delete_dependencies_for_node`）。
//! - 接受泛型 `Executor`（供 service 事务内复用）。

use chrono::{DateTime, Utc};
use sqlx::{Executor, FromRow, PgPool, Postgres};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// 一条依赖边。`source blocks target`（source → target）。
#[derive(Debug, Clone)]
pub struct DependencyRow {
    pub id: String,
    pub project_id: String,
    pub source_type: String,
    pub source_id: String,
    pub target_type: String,
    pub target_id: String,
    pub created_at: DateTime<Utc>,
}

const DEP_COLS: &str = "id, project_id, source_type, source_id, target_type, target_id, created_at";

#[derive(Debug, Clone, FromRow)]
struct DependencyPlainRow {
    id: String,
    project_id: String,
    source_type: String,
    source_id: String,
    target_type: String,
    target_id: String,
    created_at: DateTime<Utc>,
}

impl From<DependencyPlainRow> for DependencyRow {
    fn from(r: DependencyPlainRow) -> Self {
        DependencyRow {
            id: r.id,
            project_id: r.project_id,
            source_type: r.source_type,
            source_id: r.source_id,
            target_type: r.target_type,
            target_id: r.target_id,
            created_at: r.created_at,
        }
    }
}

pub struct CreateDependencyInput<'a> {
    pub project_id: &'a str,
    pub source_type: &'a str,
    pub source_id: &'a str,
    pub target_type: &'a str,
    pub target_id: &'a str,
}

/// 新建一条边 source → target。`id = nanoid(12)`。
/// 唯一约束 `(source_type, source_id, target_type, target_id)` 由 DB 兜底防重复。
/// 接受泛型 `Executor`（事务内复用）。
pub async fn create_dependency<'e, E>(
    executor: E,
    input: CreateDependencyInput<'_>,
) -> Result<DependencyRow, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let id = nanoid(12);
    let sql = format!(
        "INSERT INTO dependency \
           (id, project_id, source_type, source_id, target_type, target_id, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW()) \
         RETURNING {DEP_COLS}"
    );
    let row = sqlx::query_as::<_, DependencyPlainRow>(&sql)
        .bind(&id)
        .bind(input.project_id)
        .bind(input.source_type)
        .bind(input.source_id)
        .bind(input.target_type)
        .bind(input.target_id)
        .fetch_one(executor)
        .await?;
    Ok(row.into())
}

/// 删除一条指定边，返回被删的行（`None` = 边不存在）。
pub async fn delete_dependency<'e, E>(
    executor: E,
    project_id: &str,
    source_type: &str,
    source_id: &str,
    target_type: &str,
    target_id: &str,
) -> Result<Option<DependencyRow>, AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    let sql = format!(
        "DELETE FROM dependency \
         WHERE project_id = $1 AND source_type = $2 AND source_id = $3 \
           AND target_type = $4 AND target_id = $5 \
         RETURNING {DEP_COLS}"
    );
    let row = sqlx::query_as::<_, DependencyPlainRow>(&sql)
        .bind(project_id)
        .bind(source_type)
        .bind(source_id)
        .bind(target_type)
        .bind(target_id)
        .fetch_optional(executor)
        .await?;
    Ok(row.map(Into::into))
}

/// 查询某节点的全部**直接出边**（该节点阻塞了谁 = Blocking 列表）。
pub async fn list_outgoing(
    pool: &PgPool,
    project_id: &str,
    node_type: &str,
    node_id: &str,
) -> Result<Vec<DependencyRow>, AppError> {
    let sql = format!(
        "SELECT {DEP_COLS} FROM dependency \
         WHERE project_id = $1 AND source_type = $2 AND source_id = $3 \
         ORDER BY created_at ASC, id ASC"
    );
    let rows = sqlx::query_as::<_, DependencyPlainRow>(&sql)
        .bind(project_id)
        .bind(node_type)
        .bind(node_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// 查询某节点的全部**直接入边**（谁阻塞了该节点 = Blocked By 列表）。
pub async fn list_incoming(
    pool: &PgPool,
    project_id: &str,
    node_type: &str,
    node_id: &str,
) -> Result<Vec<DependencyRow>, AppError> {
    let sql = format!(
        "SELECT {DEP_COLS} FROM dependency \
         WHERE project_id = $1 AND target_type = $2 AND target_id = $3 \
         ORDER BY created_at ASC, id ASC"
    );
    let rows = sqlx::query_as::<_, DependencyPlainRow>(&sql)
        .bind(project_id)
        .bind(node_type)
        .bind(node_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// 查询项目内该节点的全部边（出边 + 入边），用于构建子图做传递依赖/循环检测。
#[allow(dead_code)]
pub async fn list_all_for_node(
    pool: &PgPool,
    project_id: &str,
    node_type: &str,
    node_id: &str,
) -> Result<Vec<DependencyRow>, AppError> {
    let sql = format!(
        "SELECT {DEP_COLS} FROM dependency \
         WHERE project_id = $1 AND \
           ((source_type = $2 AND source_id = $3) OR (target_type = $2 AND target_id = $3))"
    );
    let rows = sqlx::query_as::<_, DependencyPlainRow>(&sql)
        .bind(project_id)
        .bind(node_type)
        .bind(node_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// 查询项目内**所有**依赖边（构建完整图，用于传递依赖查询与循环检测）。
pub async fn list_all_in_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<DependencyRow>, AppError> {
    let sql = format!("SELECT {DEP_COLS} FROM dependency WHERE project_id = $1");
    let rows = sqlx::query_as::<_, DependencyPlainRow>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// 级联删除某节点的所有相关边（出边 + 入边）。删除需求/缺陷时调用。
/// 接受泛型 `Executor`（可在删除节点的同一事务内复用）。
pub async fn delete_dependencies_for_node<'e, E>(
    executor: E,
    project_id: &str,
    node_type: &str,
    node_id: &str,
) -> Result<(), AppError>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "DELETE FROM dependency \
         WHERE project_id = $1 AND \
           ((source_type = $2 AND source_id = $3) OR (target_type = $2 AND target_id = $3))",
    )
    .bind(project_id)
    .bind(node_type)
    .bind(node_id)
    .execute(executor)
    .await?;
    Ok(())
}
