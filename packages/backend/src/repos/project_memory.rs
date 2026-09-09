//! 项目级记忆表访问（1:1 每项目一份，仅可编辑不可删除）。
//!
//! 对齐 `requirement_memory` 的 snapshot 语义：snapshot 为自由 Markdown 文本
//! （TEXT，可 NULL = 清空）；upsert 走 `ON CONFLICT (project_id)` 或先查后写，
//! 与需求级实现保持同一行为（空 data 不发 UPDATE、不刷 updated_at）。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

const MEMORY_COLS: &str = "id, project_id, snapshot, updated_at";

#[derive(Debug, Clone, FromRow)]
pub struct ProjectMemoryRow {
    pub id: String,
    pub project_id: String,
    pub snapshot: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// 响应序列化：对齐需求级 `memory_dto` 的形状（id/projectId/snapshot/updatedAt）。
pub fn project_memory_dto(m: &ProjectMemoryRow) -> serde_json::Value {
    serde_json::json!({
        "id": m.id,
        "projectId": m.project_id,
        "snapshot": m.snapshot,
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

/// upsert 项目记忆：snapshot 全量覆盖（Markdown 文本）。
///
/// `snapshot` 三态与需求级一致：
/// - `None`（字段缺失 → JS undefined）：update 路径不发 UPDATE，连 `@updatedAt` 都不动；
///   create 路径必填缺参 → 500（路由层已先 400 SNAPSHOT_REQUIRED，这里是兜底）。
/// - `Some(None)`（显式 null）：置 NULL（清空记忆）。
/// - `Some(Some(v))`：正常写入 Markdown 文本。
pub async fn upsert_project_memory(
    pool: &PgPool,
    project_id: &str,
    snapshot: Option<Option<&str>>,
) -> Result<ProjectMemoryRow, AppError> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT id FROM project_memory WHERE project_id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if let Some((id,)) = existing {
        let Some(value) = snapshot else {
            // 空 data：不发 UPDATE，不刷 updated_at。
            let row = sqlx::query_as::<_, ProjectMemoryRow>(&format!(
                "SELECT {MEMORY_COLS} FROM project_memory WHERE id = $1"
            ))
            .bind(&id)
            .fetch_one(pool)
            .await?;
            return Ok(row);
        };
        let row = sqlx::query_as::<_, ProjectMemoryRow>(&format!(
            "UPDATE project_memory SET snapshot = $2, updated_at = NOW() WHERE id = $1 RETURNING {MEMORY_COLS}"
        ))
        .bind(&id)
        .bind(value)
        .fetch_one(pool)
        .await?;
        Ok(row)
    } else {
        let Some(value) = snapshot else {
            return Err(AppError::internal(
                "Argument `snapshot` is missing.".to_string(),
            ));
        };
        let id = nanoid(12);
        let row = sqlx::query_as::<_, ProjectMemoryRow>(&format!(
            "INSERT INTO project_memory (id, project_id, snapshot, updated_at) \
             VALUES ($1, $2, $3, NOW()) RETURNING {MEMORY_COLS}"
        ))
        .bind(&id)
        .bind(project_id)
        .bind(value)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }
}
