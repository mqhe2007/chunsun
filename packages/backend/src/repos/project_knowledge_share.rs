//! 知识文档公开分享表访问。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct KnowledgeShareRow {
    pub id: String,
    pub project_id: String,
    pub document_id: String,
    pub token_hash: String,
    pub enabled: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PublicSharePayloadRow {
    pub title: String,
    pub content: String,
    pub updated_at: DateTime<Utc>,
    pub project_name: String,
    pub enabled: bool,
    pub expires_at: Option<DateTime<Utc>>,
}

const SHARE_COLS: &str = "id, project_id, document_id, token_hash, enabled, expires_at, created_by, created_at, updated_at";

pub async fn get_by_project_document(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
) -> Result<Option<KnowledgeShareRow>, AppError> {
    let row = sqlx::query_as::<_, KnowledgeShareRow>(&format!(
        "SELECT {SHARE_COLS} FROM project_knowledge_share \
         WHERE project_id = $1 AND document_id = $2"
    ))
    .bind(project_id)
    .bind(document_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn upsert_share(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
    token_hash: &str,
    enabled: bool,
    expires_at: Option<DateTime<Utc>>,
    created_by: &str,
) -> Result<KnowledgeShareRow, AppError> {
    let ts = now();
    let row = sqlx::query_as::<_, KnowledgeShareRow>(&format!(
        "INSERT INTO project_knowledge_share \
         (id, project_id, document_id, token_hash, enabled, expires_at, created_by, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) \
         ON CONFLICT (project_id, document_id) DO UPDATE SET \
           token_hash = EXCLUDED.token_hash, \
           enabled = EXCLUDED.enabled, \
           expires_at = EXCLUDED.expires_at, \
           updated_at = EXCLUDED.updated_at \
         RETURNING {SHARE_COLS}"
    ))
    .bind(nanoid(12))
    .bind(project_id)
    .bind(document_id)
    .bind(token_hash)
    .bind(enabled)
    .bind(expires_at)
    .bind(created_by)
    .bind(ts)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update_share_meta(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
    enabled: Option<bool>,
    expires_at: Option<Option<DateTime<Utc>>>,
) -> Result<Option<KnowledgeShareRow>, AppError> {
    let existing = get_by_project_document(pool, project_id, document_id).await?;
    let Some(row) = existing else {
        return Ok(None);
    };
    let enabled = enabled.unwrap_or(row.enabled);
    let expires_at = match expires_at {
        Some(v) => v,
        None => row.expires_at,
    };
    let ts = now();
    let updated = sqlx::query_as::<_, KnowledgeShareRow>(&format!(
        "UPDATE project_knowledge_share SET enabled = $3, expires_at = $4, updated_at = $5 \
         WHERE project_id = $1 AND document_id = $2 \
         RETURNING {SHARE_COLS}"
    ))
    .bind(project_id)
    .bind(document_id)
    .bind(enabled)
    .bind(expires_at)
    .bind(ts)
    .fetch_optional(pool)
    .await?;
    Ok(updated)
}

pub async fn disable_share(
    pool: &PgPool,
    project_id: &str,
    document_id: &str,
) -> Result<Option<KnowledgeShareRow>, AppError> {
    update_share_meta(pool, project_id, document_id, Some(false), None).await
}

/// 按 token_hash 取公开载荷；调用方负责校验 enabled / expires。
pub async fn get_public_by_token_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<PublicSharePayloadRow>, AppError> {
    let row = sqlx::query_as::<_, PublicSharePayloadRow>(
        "SELECT d.title, d.content, d.updated_at, p.name AS project_name, \
                s.enabled, s.expires_at \
         FROM project_knowledge_share s \
         INNER JOIN project_knowledge_document d ON d.id = s.document_id \
         INNER JOIN project p ON p.id = s.project_id \
         WHERE s.token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
