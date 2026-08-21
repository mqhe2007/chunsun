//! 项目上下文文档 + 项目宪法的表访问
//! （对齐 `projectContextDocumentRepository.ts` / `projectPolicyRepository.ts`）。
//!
//! 兼容要点：
//! - 两张表主键都是 `nanoid(12)`，`updated_at` 由应用层维护（Prisma `@updatedAt`）。
//! - 列表排序固定 `sortOrder asc, createdAt desc`——**两级排序缺一不可**，
//!   同 sortOrder 时新文档在前。
//! - [`update_context_document`] 在「没有任何字段要写」时**不发 UPDATE**：
//!   Prisma 对空 `data` 会退化成纯读，`@updatedAt` 不刷新。实测确认
//!   （`PUT {}` 间隔 1.3s 两次，`updatedAt` 逐字节相同），而同值写入
//!   （`PUT {"title":"B"}` 写回原值）**照常刷新**。这个区别必须复刻。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

/// Prisma 的 `@default(now())` / `@updatedAt` 由应用层生成，统一走这里。
fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, FromRow)]
pub struct ContextDocRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub sort_order: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ProjectPolicyRow {
    pub constitution_md: String,
    pub updated_at: DateTime<Utc>,
}

const DOC_COLS: &str = "id, title, content, sort_order, created_at, updated_at";

/// getProjectPolicy：`findUnique({ where: { projectId } })`，缺行返回 None（**不建行**）。
pub async fn get_project_policy(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<ProjectPolicyRow>, AppError> {
    let row = sqlx::query_as::<_, ProjectPolicyRow>(
        "SELECT constitution_md, updated_at FROM project_policy WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// upsertProjectPolicy：`ON CONFLICT (project_id)` 对齐 Prisma 的 upsert。
///
/// 路由层 `content` 是必填的，所以 create / update 两个分支都会写 `constitution_md`，
/// 不需要复刻 Prisma update 分支里那个 `?? {}` 的条件展开。
pub async fn upsert_project_policy(
    pool: &PgPool,
    project_id: &str,
    constitution_md: &str,
) -> Result<ProjectPolicyRow, AppError> {
    let ts = now();
    let row = sqlx::query_as::<_, ProjectPolicyRow>(
        "INSERT INTO project_policy (id, project_id, constitution_md, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $4) \
         ON CONFLICT (project_id) DO UPDATE SET constitution_md = EXCLUDED.constitution_md, \
           updated_at = EXCLUDED.updated_at \
         RETURNING constitution_md, updated_at",
    )
    .bind(nanoid(12))
    .bind(project_id)
    .bind(constitution_md)
    .bind(ts)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// listContextDocuments：`orderBy: [{ sortOrder: "asc" }, { createdAt: "desc" }]`。
pub async fn list_context_documents(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<ContextDocRow>, AppError> {
    let sql = format!(
        "SELECT {DOC_COLS} FROM project_context_document WHERE project_id = $1 \
         ORDER BY sort_order ASC, created_at DESC"
    );
    let rows = sqlx::query_as::<_, ContextDocRow>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// `findFirst({ where: { id, projectId } })`：**双条件**，防止跨项目拿到别人的文档。
pub async fn find_context_document(
    pool: &PgPool,
    project_id: &str,
    id: &str,
) -> Result<Option<ContextDocRow>, AppError> {
    let sql = format!("SELECT {DOC_COLS} FROM project_context_document WHERE id = $1 AND project_id = $2");
    let row = sqlx::query_as::<_, ContextDocRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// createContextDocument：`sortOrder = (aggregate._max.sortOrder ?? -1) + 1`。
///
/// 注意 max 取的是**当前项目**内的最大值，且可能是负数（旧文档被手工改成 -5 时，
/// 新文档就是 -4）——实测确认过递增基准就是这个 max，不是行数。
pub async fn create_context_document(
    pool: &PgPool,
    project_id: &str,
    title: &str,
    content: &str,
) -> Result<ContextDocRow, AppError> {
    let max: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(sort_order) FROM project_context_document WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    // JS 里 `max + 1` 不会溢出（IEEE754），Prisma 才在写库时报范围错。
    let sort_order = max.unwrap_or(-1).checked_add(1).ok_or_else(|| {
        AppError::internal("SORT_ORDER_OVERFLOW: max sortOrder is already i32::MAX")
    })?;

    let ts = now();
    let sql = format!(
        "INSERT INTO project_context_document \
           (id, project_id, title, content, sort_order, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6) RETURNING {DOC_COLS}"
    );
    let row = sqlx::query_as::<_, ContextDocRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .bind(title.trim())
        .bind(content)
        .bind(sort_order)
        .bind(ts)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// updateContextDocument 的写入部分（调用方已确认行存在）。
///
/// 三个字段都是 `Option`：`None` = 请求里没这个 key = 不写。**全 None 时直接不发
/// UPDATE**，对齐 Prisma 空 `data` 不刷新 `@updatedAt` 的行为。
pub async fn update_context_document(
    pool: &PgPool,
    existing: &ContextDocRow,
    title: Option<&str>,
    content: Option<&str>,
    sort_order: Option<i32>,
) -> Result<ContextDocRow, AppError> {
    if title.is_none() && content.is_none() && sort_order.is_none() {
        return Ok(existing.clone());
    }

    let mut sets: Vec<String> = Vec::new();
    let mut idx = 1;
    if title.is_some() {
        sets.push(format!("title = ${idx}"));
        idx += 1;
    }
    if content.is_some() {
        sets.push(format!("content = ${idx}"));
        idx += 1;
    }
    if sort_order.is_some() {
        sets.push(format!("sort_order = ${idx}"));
        idx += 1;
    }
    sets.push(format!("updated_at = ${idx}"));
    idx += 1;

    let sql = format!(
        "UPDATE project_context_document SET {} WHERE id = ${idx} RETURNING {DOC_COLS}",
        sets.join(", ")
    );
    let mut q = sqlx::query_as::<_, ContextDocRow>(&sql);
    if let Some(t) = title {
        // 旧仓储层的 `data.title.trim()`——路由层不 trim，trim 只在这里发生
        q = q.bind(t.trim().to_string());
    }
    if let Some(c) = content {
        q = q.bind(c.to_string());
    }
    if let Some(s) = sort_order {
        q = q.bind(s);
    }
    let row = q.bind(now()).bind(&existing.id).fetch_one(pool).await?;
    Ok(row)
}

/// deleteContextDocument 的删除部分（调用方已确认行存在）。
pub async fn delete_context_document(pool: &PgPool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM project_context_document WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
