//! 知识库文档批注表访问（需求 u-WPvdvYh4Fw）。
//!
//! 批注挂「文档引用」三元组 `(project_id, doc_kind, document_id)`：
//! `constitution` / `memory` 是项目级单例（`document_id` 为 NULL），其余为自定义文档 id。
//! 因为宪法/记忆没有物理行，`document_id` **不加外键**——删除自定义文档时批注随文档
//! 一并删除由 `delete_annotations_for_document` 显式处理。
//!
//! 「批注永不自动删除」指不引入自动 GC；人显式删批注走 [`delete_annotation`]，
//! 文档被删导致批注失去宿主属于宿主级联，不算 GC。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, FromRow)]
pub struct KnowledgeAnnotationRow {
    pub id: String,
    /// SELECT 带出但代码不直接读（鉴权前已用 project_id 过滤）；缺了 FromRow 会炸。
    #[allow(dead_code)]
    pub project_id: String,
    pub doc_kind: String,
    pub document_id: Option<String>,
    pub anchor_text: Option<String>,
    pub anchor_prefix: Option<String>,
    pub anchor_suffix: Option<String>,
    pub body: String,
    pub status: String,
    pub outcome: Option<String>,
    pub resolved_note: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLS: &str = "id, project_id, doc_kind, document_id, anchor_text, anchor_prefix, anchor_suffix, \
                    body, status, outcome, resolved_note, resolved_by, resolved_at, created_by, \
                    created_at, updated_at";

/// 列表排序：创建时间升序（阅读页按文档从上到下逐条对照，且顺序稳定）。
pub async fn list_annotations(
    pool: &PgPool,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
) -> Result<Vec<KnowledgeAnnotationRow>, AppError> {
    // document_id IS NOT DISTINCT FROM $3：NULL 与 NULL 相等，非 NULL 按等值匹配，
    // 一条 SQL 同时覆盖「系统文档（无 document_id）」与「自定义文档」两种宿主。
    let sql = format!(
        "SELECT {COLS} FROM project_knowledge_annotation \
         WHERE project_id = $1 AND doc_kind = $2 AND document_id IS NOT DISTINCT FROM $3 \
         ORDER BY created_at ASC, id ASC"
    );
    let rows = sqlx::query_as::<_, KnowledgeAnnotationRow>(&sql)
        .bind(project_id)
        .bind(doc_kind)
        .bind(document_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// 只取 open 批注（Agent 注入用）：resolved / stale 不进 prompt。
pub async fn list_open_annotations(
    pool: &PgPool,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
) -> Result<Vec<KnowledgeAnnotationRow>, AppError> {
    let sql = format!(
        "SELECT {COLS} FROM project_knowledge_annotation \
         WHERE project_id = $1 AND doc_kind = $2 AND document_id IS NOT DISTINCT FROM $3 \
           AND status = 'open' \
         ORDER BY created_at ASC, id ASC"
    );
    let rows = sqlx::query_as::<_, KnowledgeAnnotationRow>(&sql)
        .bind(project_id)
        .bind(doc_kind)
        .bind(document_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// eager 列表注入：一次取回项目下**全部** open 批注，调用方按 (doc_kind, document_id) 分组。
///
/// 避免「每篇文档一次查询」的 N+1（eager 文档动辄十余篇）。
pub async fn list_open_annotations_for_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<KnowledgeAnnotationRow>, AppError> {
    let sql = format!(
        "SELECT {COLS} FROM project_knowledge_annotation \
         WHERE project_id = $1 AND status = 'open' \
         ORDER BY created_at ASC, id ASC"
    );
    let rows = sqlx::query_as::<_, KnowledgeAnnotationRow>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn find_annotation(
    pool: &PgPool,
    project_id: &str,
    id: &str,
) -> Result<Option<KnowledgeAnnotationRow>, AppError> {
    let sql = format!(
        "SELECT {COLS} FROM project_knowledge_annotation WHERE id = $1 AND project_id = $2"
    );
    let row = sqlx::query_as::<_, KnowledgeAnnotationRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_annotation(
    pool: &PgPool,
    project_id: &str,
    doc_kind: &str,
    document_id: Option<&str>,
    anchor_text: Option<&str>,
    anchor_prefix: Option<&str>,
    anchor_suffix: Option<&str>,
    body: &str,
    created_by: &str,
) -> Result<KnowledgeAnnotationRow, AppError> {
    let ts = now();
    let sql = format!(
        "INSERT INTO project_knowledge_annotation \
           (id, project_id, doc_kind, document_id, anchor_text, anchor_prefix, anchor_suffix, \
            body, status, outcome, resolved_note, resolved_by, resolved_at, created_by, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'open', NULL, NULL, NULL, NULL, $9, $10, $10) \
         RETURNING {COLS}"
    );
    let row = sqlx::query_as::<_, KnowledgeAnnotationRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .bind(doc_kind)
        .bind(document_id)
        .bind(anchor_text)
        .bind(anchor_prefix)
        .bind(anchor_suffix)
        .bind(body)
        .bind(created_by)
        .bind(ts)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// PATCH 的写入部分（调用方已确认行存在且已通过作者校验）。
///
/// `status` / `outcome` 两个字段以**显式动作**表达，而不是直接写枚举值，避免
/// 「status=resolved 但没带 outcome」这类半截状态被写进库：
/// - `Some(target_status)`：迁移状态。resolved 时按 `outcome` / `note` / `by` 写入
///   结案依据；open 时（重新打开）清空 outcome / note / resolved_by / resolved_at；
///   stale 时只改状态（stale 不参与结案，保留原结案字段无意义，故一并清空）。
/// - `None`：不动状态。
pub struct StatusChange<'a> {
    pub status: &'a str,
    pub outcome: Option<&'a str>,
    pub note: Option<&'a str>,
    pub by: &'a str,
}

pub async fn update_annotation(
    pool: &PgPool,
    existing: &KnowledgeAnnotationRow,
    body: Option<&str>,
    status_change: Option<StatusChange<'_>>,
) -> Result<KnowledgeAnnotationRow, AppError> {
    // 全 None（空 PATCH）时仍刷新 updated_at —— 批注是「人给文档的反馈」，
    // 与知识文档的 Prisma 空 data 语义无关，这里按新域自定，取「有请求即刷新」。
    let mut sets: Vec<String> = Vec::new();
    let mut idx = 1usize;
    let mut binds_body: Option<String> = None;
    if let Some(b) = body {
        sets.push(format!("body = ${idx}"));
        binds_body = Some(b.to_string());
        idx += 1;
    }

    // 状态相关列
    let (status, outcome, note, resolved_by, resolved_at): (
        Option<String>,
        Option<Option<String>>,
        Option<Option<String>>,
        Option<Option<String>>,
        Option<Option<DateTime<Utc>>>,
    ) = match &status_change {
        None => (None, None, None, None, None),
        Some(sc) => {
            let ts = now();
            match sc.status {
                "resolved" => (
                    Some("resolved".to_string()),
                    Some(sc.outcome.map(|o| o.to_string())),
                    Some(sc.note.map(|n| n.to_string())),
                    Some(Some(sc.by.to_string())),
                    Some(Some(ts)),
                ),
                _ => (
                    Some(sc.status.to_string()),
                    Some(None),
                    Some(None),
                    Some(None),
                    Some(None),
                ),
            }
        }
    };

    if let Some(s) = &status {
        sets.push(format!("status = ${idx}"));
        let _ = s;
        idx += 1;
    }
    if outcome.is_some() {
        sets.push(format!("outcome = ${idx}"));
        idx += 1;
    }
    if note.is_some() {
        sets.push(format!("resolved_note = ${idx}"));
        idx += 1;
    }
    if resolved_by.is_some() {
        sets.push(format!("resolved_by = ${idx}"));
        idx += 1;
    }
    if resolved_at.is_some() {
        sets.push(format!("resolved_at = ${idx}"));
        idx += 1;
    }

    sets.push(format!("updated_at = ${idx}"));
    idx += 1;

    let sql = format!(
        "UPDATE project_knowledge_annotation SET {} WHERE id = ${idx} RETURNING {COLS}",
        sets.join(", ")
    );
    let mut q = sqlx::query_as::<_, KnowledgeAnnotationRow>(&sql);
    if let Some(b) = binds_body {
        q = q.bind(b);
    }
    if let Some(s) = status {
        q = q.bind(s);
    }
    if let Some(v) = outcome {
        q = q.bind(v);
    }
    if let Some(v) = note {
        q = q.bind(v);
    }
    if let Some(v) = resolved_by {
        q = q.bind(v);
    }
    if let Some(v) = resolved_at {
        q = q.bind(v);
    }
    let row = q.bind(now()).bind(&existing.id).fetch_one(pool).await?;
    Ok(row)
}

/// 锚点漂移：把一批批注置 stale（**不删除**）。
pub async fn mark_stale(pool: &PgPool, ids: &[String]) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "UPDATE project_knowledge_annotation SET status = 'stale', updated_at = $2 WHERE id = ANY($1)",
    )
    .bind(ids)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_annotation(pool: &PgPool, id: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM project_knowledge_annotation WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 自定义文档被删除时，随宿主一并清掉其批注（否则会留下指向不存在文档的孤儿批注）。
pub async fn delete_annotations_for_document(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
) -> Result<u64, AppError> {
    let res = sqlx::query(
        "DELETE FROM project_knowledge_annotation \
         WHERE project_id = $1 AND doc_kind = 'document' AND document_id = $2",
    )
    .bind(project_id)
    .bind(document_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cols_list_matches_row_fields() {
        // 防止 SELECT 列与 FromRow 字段漂移（少一列会在运行期才炸）
        for col in [
            "id",
            "project_id",
            "doc_kind",
            "document_id",
            "anchor_text",
            "anchor_prefix",
            "anchor_suffix",
            "body",
            "status",
            "outcome",
            "resolved_note",
            "resolved_by",
            "resolved_at",
            "created_by",
            "created_at",
            "updated_at",
        ] {
            assert!(COLS.contains(col), "COLS 缺列 {col}");
        }
    }
}
