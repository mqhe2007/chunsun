//! email_log 表访问（对齐 `emailLogRepository.ts`）。
//! 注意：`to` 是 PG 保留字，SQL 中必须加引号。

use sqlx::PgPool;

use crate::api::AppError;
use crate::core::ids::nanoid;

pub async fn create_email_log(pool: &PgPool, input: EmailLogInput) -> Result<(), AppError> {
    sqlx::query(
        r#"INSERT INTO email_log (id, "to", subject, template, status, error)
           VALUES ($6, $1, $2, $3, $4, $5)"#,
    )
    .bind(&input.to)
    .bind(&input.subject)
    .bind(&input.template)
    .bind(&input.status)
    .bind(&input.error)
    .bind(nanoid(16))
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EmailLogInput {
    pub to: String,
    pub subject: String,
    pub template: String,
    pub status: String,
    pub error: Option<String>,
}
