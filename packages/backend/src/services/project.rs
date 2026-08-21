//! 项目域业务服务（1:1 移植自 `routes/project.ts` 与 `routes/projectSecretKey.ts` 的 handler 逻辑）。
//!
//! 与旧实现逐条对齐的行为约定：
//! - **可见性**：列表 / 详情 / PATCH 的 404 判定基于「创建者 ∪ 项目成员」；DELETE 更严格，
//!   普通用户只能删自己创建的项目，成员删他人项目同样是 404（不是 403）。
//! - **prompt 子路由**没有独立权限档，只要能看到项目就能读写（沿用旧实现）。
//! - **secret-key 子路由**才走 `secretKey.read` / `secretKey.write` 权限矩阵，
//!   且 SK 通道（CLI）禁止调用写接口，避免用旧密钥换新密钥的自举链。

use axum::http::StatusCode;
use serde_json::Value;
use sqlx::PgPool;

use crate::api::AppError;
use crate::core::js_number::Pagination;
use crate::core::permission_policy::ProjectAction;
use crate::core::tokens::generate_secure_token;
use crate::repos::project::{self, ProjectRow};
use crate::repos::project_member;
use crate::repos::prompt::{self, PromptRow};
use crate::repos::repository::{self, RepositoryRow};
use crate::services::activity_log::{log_activity, ActivityAction, LogActivityOptions};
use crate::services::notification::{notify_user, NotificationData};
use crate::services::project_access::can_project_action_db;
use crate::services::project_stats::get_project_statistics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectFailure {
    ProjectNotFound,
    Forbidden,
    /// 分页参数落到旧实现会让 Prisma 抛错的区间（如 `page=-1`）。
    InvalidPagination,
}

impl ProjectFailure {
    pub fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::ProjectNotFound => (StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::InvalidPagination => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR")
            }
        }
    }
}

impl From<ProjectFailure> for AppError {
    fn from(f: ProjectFailure) -> Self {
        let (status, code) = f.status_and_code();
        AppError::new(status, code)
    }
}

// ── 列表 ────────────────────────────────────────────────────────────────

pub struct ProjectListResult {
    pub items: Vec<ProjectRow>,
    pub total: i64,
}

pub async fn list_projects(
    pool: &PgPool,
    user_id: &str,
    is_admin: bool,
    page: f64,
    page_size: f64,
) -> Result<ProjectListResult, AppError> {
    let pagination = Pagination::resolve(page, page_size)
        .map_err(|_| AppError::from(ProjectFailure::InvalidPagination))?;
    let (items, total) =
        project::list_projects_by_user(pool, user_id, is_admin, pagination).await?;
    Ok(ProjectListResult { items, total })
}

// ── 详情 ────────────────────────────────────────────────────────────────

pub struct ProjectDetail {
    pub project: ProjectRow,
    pub statistics: Value,
    pub repositories: Vec<RepositoryRow>,
}

pub async fn get_project_detail(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<ProjectDetail, AppError> {
    let project = project::get_project_by_id(pool, project_id, user_id, is_admin)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;

    let statistics = get_project_statistics(pool, project_id).await?;
    repository::ensure_default_repository_for_project_id(pool, project_id).await?;
    let repositories = repository::list_repositories_by_project(pool, project_id).await?;

    Ok(ProjectDetail {
        project,
        statistics,
        repositories,
    })
}

// ── 创建 / 更新 / 删除 ──────────────────────────────────────────────────

pub async fn create_project(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    description: Option<&str>,
) -> Result<ProjectRow, AppError> {
    let project = project::create_project(pool, user_id, name, description).await?;

    log_activity(
        pool,
        &project.id,
        user_id,
        ActivityAction::ProjectCreated,
        LogActivityOptions {
            entity_type: Some("PROJECT"),
            entity_id: Some(&project.id),
            ..Default::default()
        },
    )
    .await?;

    // 创建者自动成为 OWNER 成员，并保证有一个默认仓库
    project_member::add_project_member(pool, &project.id, user_id, "OWNER").await?;
    repository::ensure_default_repository_for_project_id(pool, &project.id).await?;

    Ok(project)
}

pub async fn update_project(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<ProjectRow, AppError> {
    // 先按「可见性」判 404，再按「可写性」判；旧实现两步都返回 PROJECT_NOT_FOUND
    project::get_project_by_id(pool, project_id, user_id, is_admin)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;

    let updated =
        project::update_project_by_id(pool, project_id, user_id, is_admin, name, description)
            .await?
            .ok_or(ProjectFailure::ProjectNotFound)?;

    log_activity(
        pool,
        project_id,
        user_id,
        ActivityAction::ProjectUpdated,
        LogActivityOptions {
            entity_type: Some("PROJECT"),
            entity_id: Some(project_id),
            ..Default::default()
        },
    )
    .await?;

    Ok(updated)
}

pub async fn delete_project(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<ProjectRow, AppError> {
    let deleted = project::delete_project_by_id(pool, project_id, user_id, is_admin)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;
    Ok(deleted)
}

// ── 提示词 ──────────────────────────────────────────────────────────────

pub async fn get_prompt(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<PromptRow, AppError> {
    project::get_project_by_id(pool, project_id, user_id, is_admin)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;
    prompt::get_or_create_prompt(pool, project_id).await
}

pub async fn update_prompt(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    system_prompt: Option<&str>,
    user_prompt_template: Option<&str>,
) -> Result<PromptRow, AppError> {
    project::get_project_by_id(pool, project_id, user_id, is_admin)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;
    prompt::upsert_prompt(pool, project_id, system_prompt, user_prompt_template).await
}

// ── Secret Key ──────────────────────────────────────────────────────────

pub struct SecretKeyView {
    pub secret_key: Option<String>,
    pub has_secret_key: bool,
}

/// SK 通道调用时必须与路由上的 projectId 一致，否则 403（JWT 通道不做此校验）。
fn assert_sk_project_match(
    auth_project_id: Option<&str>,
    project_id: &str,
) -> Result<(), ProjectFailure> {
    match auth_project_id {
        Some(bound) if bound != project_id => Err(ProjectFailure::Forbidden),
        _ => Ok(()),
    }
}

pub async fn get_secret_key(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    auth_project_id: Option<&str>,
) -> Result<SecretKeyView, AppError> {
    assert_sk_project_match(auth_project_id, project_id)?;

    let project = project::get_project_row_by_id(pool, project_id)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;

    let allowed = can_project_action_db(
        pool,
        ProjectAction::SecretKeyRead,
        project_id,
        user_id,
        is_admin,
    )
    .await?;
    if !allowed {
        return Err(ProjectFailure::Forbidden.into());
    }

    Ok(SecretKeyView {
        has_secret_key: project.secret_key.is_some(),
        secret_key: project.secret_key,
    })
}

pub async fn generate_secret_key(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    auth_project_id: Option<&str>,
) -> Result<String, AppError> {
    // SK 通道禁止调用（避免用旧密钥自举出新密钥）
    if auth_project_id.is_some() {
        return Err(ProjectFailure::Forbidden.into());
    }

    let allowed = can_project_action_db(
        pool,
        ProjectAction::SecretKeyWrite,
        project_id,
        user_id,
        is_admin,
    )
    .await?;
    if !allowed {
        return Err(ProjectFailure::Forbidden.into());
    }

    let before = project::get_project_row_by_id(pool, project_id).await?;
    let key = format!("sk_{}", generate_secure_token(32));
    let updated = project::set_project_secret_key(pool, project_id, &key).await?;

    if let Some(before) = before {
        notify_user(
            pool,
            NotificationData {
                user_id: before.user_id.clone(),
                ty: "security_alert".to_string(),
                title: "项目 Secret Key 已重新生成".to_string(),
                body: Some(format!(
                    "项目「{}」的 Secret Key 已被重新生成。如非本人操作，请检查项目成员权限。",
                    before.name
                )),
                link: Some(format!("/projects/{project_id}/settings")),
            },
        )
        .await?;
    }

    updated
        .secret_key
        .ok_or_else(|| AppError::internal("secret key 写入后回读为空"))
}

pub async fn revoke_secret_key(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    auth_project_id: Option<&str>,
) -> Result<(), AppError> {
    if auth_project_id.is_some() {
        return Err(ProjectFailure::Forbidden.into());
    }

    let allowed = can_project_action_db(
        pool,
        ProjectAction::SecretKeyWrite,
        project_id,
        user_id,
        is_admin,
    )
    .await?;
    if !allowed {
        return Err(ProjectFailure::Forbidden.into());
    }

    project::clear_project_secret_key(pool, project_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_maps_to_legacy_status_and_code() {
        assert_eq!(
            ProjectFailure::ProjectNotFound.status_and_code(),
            (StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND")
        );
        assert_eq!(
            ProjectFailure::Forbidden.status_and_code(),
            (StatusCode::FORBIDDEN, "FORBIDDEN")
        );
    }

    #[test]
    fn sk_channel_must_match_route_project() {
        // JWT 通道：不做校验
        assert_eq!(assert_sk_project_match(None, "p1"), Ok(()));
        // SK 绑定同一项目：放行
        assert_eq!(assert_sk_project_match(Some("p1"), "p1"), Ok(()));
        // SK 绑定别的项目：403
        assert_eq!(
            assert_sk_project_match(Some("p2"), "p1"),
            Err(ProjectFailure::Forbidden)
        );
    }
}
