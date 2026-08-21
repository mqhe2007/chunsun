//! repository 表访问（对齐 `repositoryRepository.ts`）。
//!
//! 兼容要点：
//! - 主键是 `nanoid(12)`（**不是** 16），与 Prisma `@default(nanoid(12))` 对齐。
//! - `updated_at` 由应用层维护，INSERT 必须显式写。
//! - `ensureDefault` 的建库走事务：先把同项目已有 default 置 false，再插入新 default，
//!   与旧实现的 `prisma.$transaction` 一致；唯一键冲突时回落到再查一次。

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

use crate::api::AppError;
use crate::core::ids::nanoid;
use crate::core::repository_slug::normalize_repository_slug;

#[derive(Debug, Clone, FromRow)]
pub struct RepositoryRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub slug: String,
    pub root_hint: Option<String>,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const REPO_COLS: &str =
    "id, project_id, name, slug, root_hint, is_default, created_at, updated_at";

/// listRepositoriesByProject：默认仓库优先，其次按创建时间倒序。
pub async fn list_repositories_by_project(
    pool: &PgPool,
    project_id: &str,
) -> Result<Vec<RepositoryRow>, AppError> {
    let sql = format!(
        "SELECT {REPO_COLS} FROM repository WHERE project_id = $1 \
         ORDER BY is_default DESC, created_at DESC"
    );
    let rows = sqlx::query_as::<_, RepositoryRow>(&sql)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// getRepositoryById：**id + projectId 双条件**，避免拿别的项目的仓库 id 越权读。
pub async fn get_repository_by_id(
    pool: &PgPool,
    repository_id: &str,
    project_id: &str,
) -> Result<Option<RepositoryRow>, AppError> {
    let sql = format!("SELECT {REPO_COLS} FROM repository WHERE id = $1 AND project_id = $2");
    let row = sqlx::query_as::<_, RepositoryRow>(&sql)
        .bind(repository_id)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

pub async fn get_default_repository_by_project_id(
    pool: &PgPool,
    project_id: &str,
) -> Result<Option<RepositoryRow>, AppError> {
    let sql = format!(
        "SELECT {REPO_COLS} FROM repository WHERE project_id = $1 AND is_default = TRUE \
         ORDER BY created_at ASC LIMIT 1"
    );
    let row = sqlx::query_as::<_, RepositoryRow>(&sql)
        .bind(project_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// ensureDefaultRepositoryForProjectId：幂等地保证项目有一个默认仓库。
///
/// 项目不存在时旧实现抛 `PROJECT_NOT_FOUND:<id>` 未捕获异常（→ 500），这里对齐为
/// [`AppError::internal`]；调用方都会先做 404 判定，正常路径不会走到。
pub async fn ensure_default_repository_for_project_id(
    pool: &PgPool,
    project_id: &str,
) -> Result<RepositoryRow, AppError> {
    if let Some(existing) = get_default_repository_by_project_id(pool, project_id).await? {
        return Ok(existing);
    }

    let project_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM project WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    let Some(project_name) = project_name else {
        return Err(AppError::internal(format!(
            "PROJECT_NOT_FOUND:{project_id}"
        )));
    };

    match create_default_repository(pool, project_id, &project_name).await {
        Ok(created) => Ok(created),
        Err(_) => {
            // 并发下另一请求已建好：回落再查一次（对齐旧实现的 try/catch fallback）
            match get_default_repository_by_project_id(pool, project_id).await? {
                Some(fallback) => Ok(fallback),
                None => Err(AppError::internal(format!(
                    "DEFAULT_REPOSITORY_CREATE_FAILED:{project_id}"
                ))),
            }
        }
    }
}

/// createRepository 的 `isDefault` 缺省分支（路由 `POST /repositories` 的唯一入口）。
///
/// 与 default 分支的两处差异：
/// - slug 走 [`normalize_repository_slug`]（入参 `slug ?? name`），不是硬编码 `"default"`
/// - 不清除同项目既有的 default 标记，因此**不需要事务**（旧实现的
///   `$transaction` 在这个分支里只包了一条 INSERT）
///
/// `(project_id, slug)` 唯一键冲突时原样抛出，由 service 层收敛成 409。
pub async fn create_repository(
    pool: &PgPool,
    project_id: &str,
    name: &str,
    slug: Option<&str>,
    root_hint: Option<&str>,
) -> Result<RepositoryRow, AppError> {
    let slug = normalize_repository_slug(slug.unwrap_or(name));

    let sql = format!(
        r#"INSERT INTO repository
             (id, project_id, name, slug, root_hint, is_default, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, FALSE, NOW(), NOW())
           RETURNING {REPO_COLS}"#
    );
    let row = sqlx::query_as::<_, RepositoryRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .bind(name)
        .bind(slug)
        .bind(root_hint)
        .fetch_one(pool)
        .await?;
    Ok(row)
}

/// createRepository 的 `isDefault: true` 分支：slug 恒为 `"default"`。
async fn create_default_repository(
    pool: &PgPool,
    project_id: &str,
    name: &str,
) -> Result<RepositoryRow, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE repository SET is_default = FALSE, updated_at = NOW() \
         WHERE project_id = $1 AND is_default = TRUE",
    )
    .bind(project_id)
    .execute(&mut *tx)
    .await?;

    let sql = format!(
        r#"INSERT INTO repository
             (id, project_id, name, slug, root_hint, is_default, created_at, updated_at)
           VALUES ($1, $2, $3, 'default', '.', TRUE, NOW(), NOW())
           RETURNING {REPO_COLS}"#
    );
    let row = sqlx::query_as::<_, RepositoryRow>(&sql)
        .bind(nanoid(12))
        .bind(project_id)
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(row)
}
