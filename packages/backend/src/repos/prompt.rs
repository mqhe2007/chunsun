//! project_prompt 表访问（对齐 `promptRepository.ts`）。
//!
//! `project_id` 上有唯一约束；主键 `nanoid(12)`，`updated_at` 由应用层维护。

use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

#[derive(Debug, Clone, FromRow)]
pub struct PromptRow {
    pub system_prompt: String,
    pub user_prompt_template: String,
}

const PROMPT_COLS: &str = "system_prompt, user_prompt_template";

pub async fn get_prompt_by_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<PromptRow>, AppError> {
    let sql = format!("SELECT {PROMPT_COLS} FROM project_prompt WHERE project_id = $1");
    let row = sqlx::query_as::<_, PromptRow>(&sql)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// getOrCreatePrompt：无记录时建一份空模板（两个字段均为空串，不是 NULL）。
pub async fn get_or_create_prompt(
    pool: &PgPool,
    project_id: &str,
) -> Result<PromptRow, AppError> {
    if let Some(existing) = get_prompt_by_project(pool, project_id).await? {
        return Ok(existing);
    }
    let sql = format!(
        r#"INSERT INTO project_prompt
             (id, project_id, system_prompt, user_prompt_template, created_at, updated_at)
           VALUES ($1, $2, '', '', NOW(), NOW())
           RETURNING {PROMPT_COLS}"#
    );
    let row = sqlx::query_as::<_, PromptRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// upsertPrompt：`None` 表示该字段本次不更新（对齐 JS 的 `!== undefined` 判断）。
/// 记录不存在时新建，缺省值同样是空串。
pub async fn upsert_prompt(
    pool: &PgPool,
    project_id: &str,
    system_prompt: Option<&str>,
    user_prompt_template: Option<&str>,
) -> Result<PromptRow, AppError> {
    if get_prompt_by_project(pool, project_id).await?.is_some() {
        let sql = format!(
            r#"UPDATE project_prompt SET
                 system_prompt = CASE WHEN $2 THEN $3 ELSE system_prompt END,
                 user_prompt_template = CASE WHEN $4 THEN $5 ELSE user_prompt_template END,
                 updated_at = NOW()
               WHERE project_id = $1
               RETURNING {PROMPT_COLS}"#
        );
        let row = sqlx::query_as::<_, PromptRow>(&sql)
            .bind(project_id)
            .bind(system_prompt.is_some())
            .bind(system_prompt)
            .bind(user_prompt_template.is_some())
            .bind(user_prompt_template)
            .fetch_one(pool)
            .await?;
        return Ok(row);
    }

    let sql = format!(
        r#"INSERT INTO project_prompt
             (id, project_id, system_prompt, user_prompt_template, created_at, updated_at)
           VALUES ($1, $2, $3, $4, NOW(), NOW())
           RETURNING {PROMPT_COLS}"#
    );
    let row = sqlx::query_as::<_, PromptRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .bind(system_prompt.unwrap_or(""))
        .bind(user_prompt_template.unwrap_or(""))
        .fetch_one(pool)
        .await?;
    Ok(row)
}
