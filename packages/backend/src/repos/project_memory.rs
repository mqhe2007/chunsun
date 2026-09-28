//! 项目级记忆表访问（1:1 每项目一份，仅可编辑不可删除）。
//!
//! 对齐 `requirement_memory` 的 snapshot 语义：snapshot 为自由 Markdown 文本
//! （TEXT，可 NULL = 清空）；upsert 走 `ON CONFLICT (project_id)` 或先查后写，
//! 与需求级实现保持同一行为。
//!
//! **2026-09-28 起 upsert 带乐观锁**（`revision` 列 + 条件 UPDATE），
//! 原先「空 data 不发 UPDATE、不刷 updated_at」的短路**已移除**：
//! 没有 snapshot 的写请求在路由层就被判成 400，到不了这里。
//! 「显式 null 清空记忆」仍是 `Some(None)` 路径，语义不变。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;
use crate::repos::project_knowledge::{classify_miss, revision_matches, WriteOutcome};

const MEMORY_COLS: &str = "id, project_id, snapshot, revision, updated_at";

#[derive(Debug, Clone, FromRow)]
pub struct ProjectMemoryRow {
    pub id: String,
    pub project_id: String,
    pub snapshot: Option<String>,
    pub revision: i32,
    pub updated_at: DateTime<Utc>,
}

/// 响应序列化：对齐需求级 `memory_dto` 的形状（id/projectId/snapshot/updatedAt）。
///
/// `revision` 是新增的乐观锁版本载体；`updatedAt` 照旧原样下发（含纳秒精度，
/// 与知识文档走 `dt_value` 的毫秒截断*不同*——这是既有行为，本次不动）。
pub fn project_memory_dto(m: &ProjectMemoryRow) -> serde_json::Value {
    serde_json::json!({
        "id": m.id,
        "projectId": m.project_id,
        "snapshot": m.snapshot,
        "revision": m.revision,
        "updatedAt": m.updated_at,
    })
}

/// 取项目记忆：缺行返回 None（**不建行**，对齐 `get_memory`）。
pub async fn get_project_memory(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<ProjectMemoryRow>, AppError> {
    let row = sqlx::query_as::<_, ProjectMemoryRow>(&format!(
        "SELECT {MEMORY_COLS} FROM project_memory WHERE project_id = $1"
    ))
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// upsert 项目记忆：snapshot 全量覆盖（Markdown 文本），带乐观锁。
///
/// `snapshot` 现在**只有两态**，三态里的「字段缺失」一态已上移到路由层：
/// - `None`：显式 null，置 NULL（清空记忆）。这是「字段缺失」那条短路被删掉之后
///   唯一剩下的空值语义，**仍然是一次真实写入**（会刷 `revision` 与 `updated_at`）。
/// - `Some(v)`：正常写入 Markdown 文本。
///
/// 「字段缺失」在 `validate_memory_snapshot` 里就是 400 `SNAPSHOT_REQUIRED`，
/// 到不了这里——所以仓储层不接 `Option<Option<&str>>`，平铺成 `Option<&str>`
/// 反而逼调用方在类型上把两件事分开。
///
/// `expected_revision = 0` 表示调用方认为项目记忆尚不存在（首次写入）。
///
/// **形态与知识文档统一**：原先的「先 `SELECT id` 再 `UPDATE ... WHERE id`」是
/// 典型的 check-then-act —— 两次查询之间别人写完了，本次照样覆盖。现在改成
/// 「条件 UPDATE → 落空则重查分类」，判定结果用 [`WriteOutcome`] 表达，
/// 与 `update_knowledge_document` / `upsert_project_policy` 逐行同构。
///
/// `updated_at` 仍用数据库 `NOW()`（不是应用层时钟）：这是既有行为，
/// 也是 `revision` 不用时间戳当载体的原因之一（时钟源不一致）。
pub async fn upsert_project_memory(
    pool: &PgPool,
    project_id: &str,
    snapshot: Option<&str>,
    expected_revision: i32,
) -> Result<WriteOutcome<ProjectMemoryRow>, AppError> {
    // 第一段：按 revision 条件更新。
    let updated = sqlx::query_as::<_, ProjectMemoryRow>(&format!(
        "UPDATE project_memory SET snapshot = $1, revision = revision + 1, updated_at = NOW() \
         WHERE project_id = $2 AND revision = $3 RETURNING {MEMORY_COLS}"
    ))
    .bind(snapshot)
    .bind(project_id)
    .bind(expected_revision)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = updated {
        return Ok(WriteOutcome::Ok(row));
    }

    // 第二段：落空。行在 → 版本不符；行不在 → 看调用方声称的版本是否为 0。
    let current = get_project_memory(pool, project_id).await?;
    if let Some(row) = current {
        return Ok(WriteOutcome::Conflict(Box::new(row)));
    }
    if !revision_matches(expected_revision, 0) {
        return Ok(WriteOutcome::NotFound);
    }

    // 并发下两个请求可能同时走到这里，`ON CONFLICT` 兜住后者。
    let id = nanoid(12);
    let inserted = sqlx::query_as::<_, ProjectMemoryRow>(&format!(
        "INSERT INTO project_memory (id, project_id, snapshot, revision, updated_at) \
         VALUES ($1, $2, $3, 1, NOW()) \
         ON CONFLICT (project_id) DO NOTHING \
         RETURNING {MEMORY_COLS}"
    ))
    .bind(&id)
    .bind(project_id)
    .bind(snapshot)
    .fetch_optional(pool)
    .await?;

    match inserted {
        Some(row) => Ok(WriteOutcome::Ok(row)),
        None => Ok(classify_miss(get_project_memory(pool, project_id).await?)),
    }
}
