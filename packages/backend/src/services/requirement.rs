//! 需求域业务服务（1:1 移植自 `packages/backend/src/routes/requirement.ts` 的 handler 逻辑）。
//!
//! 权限档比 projectEnvVar 简单：**五条端点都只判项目可见性**，没有细粒度动作校验。
//! 能看见项目就能读写它的需求，这是旧实现的既有口径，移植中不收紧。
//!
//! 三个必须照搬的旧实现怪癖（都有对拍用例兜着）：
//! 1. `create` 返回体的 `owner` 恒为 `null`——旧实现 `prisma.requirement.create` 没有
//!    include owner，即使刚把 `ownerId` 写进去，响应里也看不到 owner 对象；`patch` 才有。
//! 2. `patch` 的成员校验发生在「需求是否存在」之前。对不存在的需求 PATCH 一个非成员
//!    ownerId，拿到的是 **400 OWNER_NOT_MEMBER 而不是 404**。顺序不能换。
//! 3. `ownerId` 走 JS 真值判断：空串 `""` 是 falsy，因此
//!    - 不触发成员校验；
//!    - 在 patch 里等价于 `disconnect`（清空负责人）。

use sqlx::PgPool;

use crate::api::AppError;
use crate::repos::project_member;
use crate::repos::repository as repository_repo;
use crate::repos::requirement::{
    self, CreateRequirementInput, RequirementListFilters, RequirementListResult, RequirementRow,
    UpdateRequirementPatch,
};
use crate::services::activity_log::{log_activity, ActivityAction, LogActivityOptions};
use crate::services::notification::{notify, NotifyRequest};
use crate::services::project_access::visible_project_id;

/// 需求域的失败分支。
#[derive(Debug, Clone, Copy)]
pub enum RequirementFailure {
    /// 负责人不是当前项目成员 → 400
    OwnerNotMember,
    /// 需求不存在（或不属于该项目）→ 404
    RequirementNotFound,
}

impl From<RequirementFailure> for AppError {
    fn from(f: RequirementFailure) -> Self {
        match f {
            RequirementFailure::OwnerNotMember => AppError::bad_request("OWNER_NOT_MEMBER"),
            RequirementFailure::RequirementNotFound => {
                AppError::not_found("REQUIREMENT_NOT_FOUND")
            }
        }
    }
}

/// 对齐 `if (body.ownerId) { … }`：只有**非空字符串**才触发成员校验。
async fn assert_owner_is_member(
    pool: &PgPool,
    project_id: &str,
    owner_id: Option<&str>,
) -> Result<(), AppError> {
    let Some(owner_id) = owner_id.filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let member = project_member::get_project_member(pool, project_id, owner_id).await?;
    if member.is_none() {
        return Err(RequirementFailure::OwnerNotMember.into());
    }
    Ok(())
}

pub struct ListRequirementsQuery<'a> {
    pub status: Option<Vec<&'a str>>,
    pub id: Option<&'a str>,
    pub owner_id: Option<&'a str>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

/// GET `/projects/:projectId/requirements`
pub async fn list_requirements(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    query: ListRequirementsQuery<'_>,
) -> Result<RequirementListResult, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    requirement::list_requirements_by_project(
        pool,
        &project_id,
        RequirementListFilters {
            status: query.status,
            id: query.id,
            owner_id: query.owner_id,
            page: query.page,
            page_size: query.page_size,
        },
    )
    .await
}

pub struct CreateRequirementArgs<'a> {
    pub repository_id: Option<&'a str>,
    pub description: &'a str,
    pub source_text: Option<&'a str>,
    pub client_notes: Option<&'a str>,
    pub coverage: Option<&'a str>,
    /// `Option<Option<…>>`：缺省与显式 null 在旧实现里都摊平成 null（`body.ownerId ?? null`）。
    pub owner_id: Option<Option<&'a str>>,
}

/// POST `/projects/:projectId/requirements` → 201
///
/// `status` 不接受入参：新建固定 `pending`，真实状态由自主交付轮次投影回写。
pub async fn create_requirement(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
    args: CreateRequirementArgs<'_>,
) -> Result<RequirementRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    let owner_id = args.owner_id.flatten();
    assert_owner_is_member(pool, &project_id, owner_id).await?;

    // 对齐 `resolveRepositoryIdForProject`：传了 repositoryId 就必须命中，
    // 否则旧实现**裸抛** `REPOSITORY_NOT_FOUND:<id>`，路由没有 try/catch → 500。
    // 这里同样以 500 抛出，保持状态码一致（报文形态差异归入既有 ACCEPTED 类）。
    let resolved_repository_id = match args.repository_id.filter(|s| !s.is_empty()) {
        Some(rid) => {
            let found = repository_repo::get_repository_by_id(pool, rid, &project_id).await?;
            match found {
                Some(r) => Some(r.id),
                None => {
                    return Err(AppError::internal(format!("REPOSITORY_NOT_FOUND:{rid}")));
                }
            }
        }
        None => None,
    };

    let row = requirement::create_requirement(
        pool,
        CreateRequirementInput {
            project_id: &project_id,
            repository_id: resolved_repository_id.as_deref(),
            description: args.description,
            source_text: args.source_text,
            client_notes: args.client_notes,
            status: Some("pending"),
            coverage: args.coverage,
            origin: None,
            owner_id,
        },
    )
    .await?;

    let desc = format!("创建需求 {}", row.id);
    log_activity(
        pool,
        &project_id,
        user_id,
        ActivityAction::RequirementCreated,
        LogActivityOptions {
            entity_type: Some("requirement"),
            entity_id: Some(&row.id),
            description: Some(&desc),
            ..Default::default()
        },
    )
    .await?;

    Ok(row)
}

/// GET `/projects/:projectId/requirements/:requirementId`
pub async fn get_requirement(
    pool: &PgPool,
    project_id: &str,
    requirement_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<RequirementRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    requirement::get_requirement_by_id(pool, requirement_id, &project_id)
        .await?
        .ok_or_else(|| RequirementFailure::RequirementNotFound.into())
}

#[derive(Default)]
pub struct UpdateRequirementArgs<'a> {
    pub description: Option<&'a str>,
    pub source_text: Option<&'a str>,
    pub client_notes: Option<&'a str>,
    pub status: Option<&'a str>,
    pub coverage: Option<&'a str>,
    /// 三态：不传 / 显式 null（清空） / 有值。原始串透传，解析延迟到 repo 的存在性校验之后。
    pub released_at: Option<Option<&'a str>>,
    /// 三态：不传 / 显式 null（断开） / 有值。空串走「断开」分支（JS falsy）。
    pub owner_id: Option<Option<&'a str>>,
}

/// PATCH `/projects/:projectId/requirements/:requirementId`
pub async fn update_requirement(
    pool: &PgPool,
    project_id: &str,
    requirement_id: &str,
    user_id: &str,
    is_admin: bool,
    args: UpdateRequirementArgs<'_>,
    public_origin: &str,
) -> Result<RequirementRow, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;

    // 注意：成员校验在存在性检查之前，顺序照搬旧实现
    assert_owner_is_member(pool, &project_id, args.owner_id.flatten()).await?;

    let previous = requirement::get_requirement_by_id(pool, requirement_id, &project_id).await?;

    // `ownerId: ""` 在旧实现里走 `disconnect` 分支，等价于置 NULL
    let owner_patch = args
        .owner_id
        .map(|v| v.filter(|s| !s.is_empty()));

    let row = requirement::update_requirement_by_id(
        pool,
        requirement_id,
        &project_id,
        UpdateRequirementPatch {
            description: args.description,
            source_text: args.source_text.map(Some),
            client_notes: args.client_notes.map(Some),
            status: args.status,
            coverage: args.coverage,
            // 空串在 JS 里是 falsy → 走 `: null` 分支，等价于清空（对齐 legacy）
            released_at: args
                .released_at
                .map(|v| v.filter(|s| !s.is_empty()).map(|s| s.to_string())),
            owner_id: owner_patch,
        },
    )
    .await?
    .ok_or::<AppError>(RequirementFailure::RequirementNotFound.into())?;

    let desc = format!("更新需求 {}", row.id);
    log_activity(
        pool,
        &project_id,
        user_id,
        ActivityAction::RequirementUpdated,
        LogActivityOptions {
            entity_type: Some("requirement"),
            entity_id: Some(&row.id),
            description: Some(&desc),
            ..Default::default()
        },
    )
    .await?;

    if let Some(prev) = previous {
        if prev.owner_id != row.owner_id {
            let mut recipients = Vec::new();
            if let Some(old) = prev.owner_id {
                recipients.push(old);
            }
            if let Some(new) = row.owner_id.clone() {
                recipients.push(new);
            }
            if !recipients.is_empty() {
                notify(
                    pool,
                    public_origin,
                    NotifyRequest {
                        event: "requirement_owner_changed".into(),
                        recipient_user_ids: recipients,
                        actor_user_id: Some(user_id.to_string()),
                        title: "需求负责人已变更".into(),
                        body: Some(format!("需求「{}」的负责人已更新。", row.description)),
                        link: Some(format!(
                            "/projects/{}/requirements/{}",
                            project_id, row.id
                        )),
                        email_link: None,
                    },
                )
                .await?;
            }
        }
    }

    Ok(row)
}

/// DELETE `/projects/:projectId/requirements/:requirementId`
///
/// 旧实现**没有**写活动日志（与 create/patch 不对称），照搬。
pub async fn delete_requirement(
    pool: &PgPool,
    project_id: &str,
    requirement_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<String, AppError> {
    let project_id = visible_project_id(pool, project_id, user_id, is_admin).await?;
    let row = requirement::delete_requirement_by_id(pool, requirement_id, &project_id)
        .await?
        .ok_or::<AppError>(RequirementFailure::RequirementNotFound.into())?;
    Ok(row.id)
}
