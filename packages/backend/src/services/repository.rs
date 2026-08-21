//! 仓库域业务服务（1:1 移植自 `packages/backend/src/routes/repository.ts` 的 handler 逻辑）。
//!
//! 三条端点共享同一个前置动作：先用 `getProjectById`（创建者 ∪ 项目成员，ADMIN 全通）
//! 判可见性，**看不见就 404 `PROJECT_NOT_FOUND`**。仓库本身没有独立权限档——能看到
//! 项目就能读写它的仓库列表，这是旧实现的既有口径，不在移植中收紧。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::repository::{self, RepositoryRow};
use crate::services::project_access::visible_project_id;

/// GET `/projects/:projectId/repositories`
///
/// **读接口里有隐式写**：列表前会 `ensureDefaultRepository`，首次访问的项目会被补一条
/// default 仓库。对拍时必须先预热，否则「旧后端建、新后端读」会被误判成 DIFF。
pub async fn list_repositories(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<Vec<RepositoryRow>, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    repository::ensure_default_repository_for_project_id(pool, &project_id).await?;
    repository::list_repositories_by_project(pool, &project_id).await
}

/// POST `/projects/:projectId/repositories`
///
/// 旧实现是 `try { create } catch { 409 }`——**任何**异常（唯一键冲突、连接中断、
/// 字段超长）都会被吞成 `REPOSITORY_SLUG_CONFLICT`。这里刻意照搬这个「过宽」的
/// catch，而不是只识别 23505：对拍要的是逐字节一致，收紧留到后续统一治理。
pub async fn create_repository(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    name: &str,
    slug: Option<&str>,
    root_hint: Option<&str>,
) -> Result<RepositoryRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    repository::create_repository(pool, &project_id, name, slug, root_hint)
        .await
        .map_err(|e| {
            tracing::warn!(error = ?e, "create repository failed, mapped to 409");
            AppError::conflict("REPOSITORY_SLUG_CONFLICT")
        })
}

/// GET `/projects/:projectId/repositories/:repositoryId`
pub async fn get_repository(
    pool: &PgPool,
    project_id: &str,
    repository_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<RepositoryRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    repository::get_repository_by_id(pool, repository_id, &project_id)
        .await?
        .ok_or_else(|| AppError::not_found("REPOSITORY_NOT_FOUND"))
}
