//! project_env_var 表访问（对齐 `projectEnvVarRepository.ts`）。
//!
//! 兼容要点：
//! - 主键 `nanoid(12)`（与 Prisma `@default(nanoid(12))` 对齐，**不是** 16）。
//! - `updated_at` 是 Prisma `@updatedAt`（应用层维护），INSERT/UPDATE 都必须显式写，
//!   否则 UPDATE 后时间戳不动，前端「最近修改」排序会失真。
//! - 唯一键 `(project_id, key)`：冲突由上层统一收敛成 409，本层只管把错误抛上去。
//! - `value` 列存的是**封套后**的串（`enc:v1:...` 或遗留明文），本层不做任何加解密——
//!   密文边界收敛在 `core::env_var_crypto`，仓储层永远只搬字节。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;

#[derive(Debug, Clone, FromRow)]
pub struct ProjectEnvVarRow {
    pub id: String,
    pub key: String,
    pub value: String,
    pub description: Option<String>,
    pub is_secret: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const ENV_VAR_COLS: &str =
    "id, key, value, description, is_secret, created_at, updated_at";

/// listEnvVarsByProject：按 key 升序（旧实现 `orderBy: [{ key: "asc" }]`）。
pub async fn list_env_vars_by_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<ProjectEnvVarRow>, AppError> {
    let sql = format!(
        "SELECT {ENV_VAR_COLS} FROM project_env_var WHERE project_id = $1 ORDER BY key ASC"
    );
    let rows = sqlx::query_as::<_, ProjectEnvVarRow>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

pub async fn count_env_vars_by_project(pool: &PgPool, project_id: &str) -> Result<i64, AppError> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM project_env_var WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    Ok(count)
}

/// getEnvVarById：**id + projectId 双条件**，避免跨项目拿 id 越权读写。
pub async fn get_env_var_by_id(
    pool: &PgPool,
    project_id: &str,
    id: &str,
) -> Result<Option<ProjectEnvVarRow>, AppError> {
    let sql = format!("SELECT {ENV_VAR_COLS} FROM project_env_var WHERE id = $1 AND project_id = $2");
    let row = sqlx::query_as::<_, ProjectEnvVarRow>(&sql)
        .bind(id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// getEnvVarByKey：走 `(project_id, key)` 唯一键。
pub async fn get_env_var_by_key(
    pool: &PgPool,
    project_id: &str,
    key: &str,
) -> Result<Option<ProjectEnvVarRow>, AppError> {
    let sql =
        format!("SELECT {ENV_VAR_COLS} FROM project_env_var WHERE project_id = $1 AND key = $2");
    let row = sqlx::query_as::<_, ProjectEnvVarRow>(&sql)
        .bind(project_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub struct CreateEnvVarInput<'a> {
    pub project_id: &'a str,
    pub key: &'a str,
    /// 已封套的存储值（调用方负责 seal）。
    pub value: &'a str,
    pub description: Option<&'a str>,
    pub is_secret: bool,
}

pub async fn create_env_var(
    pool: &PgPool,
    input: CreateEnvVarInput<'_>,
) -> Result<ProjectEnvVarRow, AppError> {
    let sql = format!(
        "INSERT INTO project_env_var \
         (id, project_id, key, value, description, is_secret, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW()) RETURNING {ENV_VAR_COLS}"
    );
    let row = sqlx::query_as::<_, ProjectEnvVarRow>(&sql)
        .bind(nanoid(12))
        .bind(input.project_id)
        .bind(input.key)
        .bind(input.value)
        .bind(input.description)
        .bind(input.is_secret)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// 部分更新的补丁；`None` = 该列不动，`Some(None)`（仅 description）= 写 NULL。
#[derive(Debug, Default)]
pub struct UpdateEnvVarPatch<'a> {
    pub key: Option<&'a str>,
    /// 已封套的存储值。
    pub value: Option<&'a str>,
    pub description: Option<Option<&'a str>>,
    pub is_secret: Option<bool>,
}

impl UpdateEnvVarPatch<'_> {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.key.is_none()
            && self.value.is_none()
            && self.description.is_none()
            && self.is_secret.is_none()
    }
}

/// updateEnvVarById：先按 id+projectId 确认存在（不存在返回 None），再动态拼列更新。
///
/// 旧实现即使 patch 为空也会 `prisma.update({ data: {} })`——那仍会因 `@updatedAt`
/// 刷新 `updated_at`。这里保留同样的语义：空补丁也走一次 `SET updated_at = NOW()`。
pub async fn update_env_var_by_id(
    pool: &PgPool,
    project_id: &str,
    id: &str,
    patch: UpdateEnvVarPatch<'_>,
) -> Result<Option<ProjectEnvVarRow>, AppError> {
    if get_env_var_by_id(pool, project_id, id).await?.is_none() {
        return Ok(None);
    }

    let mut sets: Vec<String> = Vec::new();
    let mut idx = 1;
    if patch.key.is_some() {
        sets.push(format!("key = ${idx}"));
        idx += 1;
    }
    if patch.value.is_some() {
        sets.push(format!("value = ${idx}"));
        idx += 1;
    }
    if patch.description.is_some() {
        sets.push(format!("description = ${idx}"));
        idx += 1;
    }
    if patch.is_secret.is_some() {
        sets.push(format!("is_secret = ${idx}"));
        idx += 1;
    }
    sets.push("updated_at = NOW()".to_string());

    let sql = format!(
        "UPDATE project_env_var SET {} WHERE id = ${idx} RETURNING {ENV_VAR_COLS}",
        sets.join(", ")
    );

    let mut q = sqlx::query_as::<_, ProjectEnvVarRow>(&sql);
    if let Some(v) = patch.key {
        q = q.bind(v);
    }
    if let Some(v) = patch.value {
        q = q.bind(v);
    }
    if let Some(v) = patch.description {
        q = q.bind(v);
    }
    if let Some(v) = patch.is_secret {
        q = q.bind(v);
    }
    q = q.bind(id);

    let row = q.fetch_optional(pool).await?;
    Ok(row)
}

/// deleteEnvVarById：先确认存在再删，返回是否删掉（对齐旧实现的 boolean）。
pub async fn delete_env_var_by_id(
    pool: &PgPool,
    project_id: &str,
    id: &str,
) -> Result<bool, AppError> {
    if get_env_var_by_id(pool, project_id, id).await?.is_none() {
        return Ok(false);
    }
    let res = sqlx::query("DELETE FROM project_env_var WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_patch_is_detected() {
        assert!(UpdateEnvVarPatch::default().is_empty());
        assert!(!UpdateEnvVarPatch {
            key: Some("A"),
            ..Default::default()
        }
        .is_empty());
        // 显式写 NULL 也算非空补丁
        assert!(!UpdateEnvVarPatch {
            description: Some(None),
            ..Default::default()
        }
        .is_empty());
    }
}
