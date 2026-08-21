//! 项目成员域业务服务（1:1 移植自 `packages/backend/src/routes/projectMember.ts`）。
//!
//! 权限判定统一走 `project_access`（对齐 `lib/projectAccess.ts`）。
//! 注意：路径参数 `:memberId` 在旧后端实际就是 `userId`（前端传 `member.userId`），
//! 故本服务的所有 `*_by_user_id` 操作均以 user_id 为键。

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::api::AppError;
use crate::core::permission_policy::{
    can_project_action, ProjectAccessContext, ProjectAction, ProjectRole,
};
use crate::repos::project;
use crate::repos::project_member::{self, MemberWithUser};
use crate::repos::user;
use crate::services::email;
use crate::services::notification::{notify_user, NotificationData};
use crate::services::project_access;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberFailure {
    ProjectNotFound,
    UserNotFound,
    CannotInviteSelf,
    UserIsOwner,
    #[allow(dead_code)]
    EmptyPatch,
    CannotUpdateSelf,
    CannotRemoveOwner,
    MemberNotFound,
    Forbidden,
}

impl MemberFailure {
    pub fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::ProjectNotFound => (StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND"),
            Self::UserNotFound => (StatusCode::NOT_FOUND, "USER_NOT_FOUND"),
            Self::CannotInviteSelf => (StatusCode::BAD_REQUEST, "CANNOT_INVITE_SELF"),
            Self::UserIsOwner => (StatusCode::BAD_REQUEST, "USER_IS_OWNER"),
            Self::EmptyPatch => (StatusCode::BAD_REQUEST, "EMPTY_PATCH"),
            Self::CannotUpdateSelf => (StatusCode::BAD_REQUEST, "CANNOT_UPDATE_SELF"),
            Self::CannotRemoveOwner => (StatusCode::BAD_REQUEST, "CANNOT_REMOVE_OWNER"),
            Self::MemberNotFound => (StatusCode::NOT_FOUND, "MEMBER_NOT_FOUND"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "FORBIDDEN"),
        }
    }
}

impl From<MemberFailure> for AppError {
    fn from(f: MemberFailure) -> Self {
        let (status, code) = f.status_and_code();
        AppError::new(status, code)
    }
}

/// GET /projects/:projectId/members —— 列出成员。
pub async fn list(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<Vec<MemberWithUser>, AppError> {
    if !is_admin {
        let ctx = project_access::resolve_project_access(pool, project_id, user_id, is_admin).await?;
        match ctx {
            None => return Err(MemberFailure::ProjectNotFound.into()),
            Some(c) => {
                let is_owner = c.is_creator;
                let is_member = c.member_role.is_some();
                if !is_owner && !is_member {
                    return Err(MemberFailure::Forbidden.into());
                }
            }
        }
    }
    project_member::list_project_members(pool, project_id).await
}

pub struct InviteResult {
    pub member: MemberWithUser,
}

/// POST /projects/:projectId/members —— 邀请成员（按 email 解析）。
pub async fn invite(
    pool: &PgPool,
    project_id: &str,
    actor_user_id: &str,
    is_platform_admin: bool,
    identifier: &str,
    role: Option<String>,
) -> Result<InviteResult, AppError> {
    // 权限判定先于目标用户解析（对齐旧后端：403 优先于 404 USER_NOT_FOUND）。
    let allowed = project_access::can_project_action_db(
        pool,
        ProjectAction::MemberInvite,
        project_id,
        actor_user_id,
        is_platform_admin,
    )
    .await?;
    if !allowed {
        return Err(MemberFailure::Forbidden.into());
    }

    let target = user::get_user_by_email(pool, identifier).await?;
    let target = target.ok_or(MemberFailure::UserNotFound)?;

    if target.id == actor_user_id {
        return Err(MemberFailure::CannotInviteSelf.into());
    }
    let project = project::get_project_by_id_only(pool, project_id).await?;
    if project.as_ref().map(|p| p.user_id == target.id).unwrap_or(false) {
        return Err(MemberFailure::UserIsOwner.into());
    }

    let role = role.unwrap_or_else(|| "MEMBER".to_string());

    let member = match project_member::get_project_member(pool, project_id, &target.id).await? {
        Some(existing) => project_member::update_project_member_role(pool, project_id, &target.id, &role)
            .await?
            .unwrap_or(existing),
        None => {
            let created = project_member::add_project_member(pool, project_id, &target.id, &role).await?;
            let project_name = project
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "未知项目".to_string());
            let link = format!("/projects/{project_id}");
            notify_user(
                pool,
                NotificationData {
                    user_id: target.id.clone(),
                    ty: "project_invitation".into(),
                    title: format!("你被邀请加入项目「{project_name}」"),
                    body: Some(format!("你已被邀请以 {role} 身份加入项目。")),
                    link: Some(link.clone()),
                },
            )
            .await?;
            // 邮件失败静默吞掉，不影响主流程（对齐旧后端 .catch(()=>{})）
            email::send_notification_email(
                pool,
                &target.email,
                &format!("你被邀请加入项目「{project_name}」"),
                &format!("你已被邀请以 {role} 身份加入项目「{project_name}」。"),
                &link,
            )
            .await;
            created
        }
    };

    Ok(InviteResult { member })
}

/// PATCH /projects/:projectId/members/:memberId —— 修改成员角色。
pub async fn update_role(
    pool: &PgPool,
    project_id: &str,
    actor_user_id: &str,
    is_platform_admin: bool,
    member_id: &str,
    role: String,
) -> Result<MemberWithUser, AppError> {
    let project = project::get_project_by_id_only(pool, project_id)
        .await?
        .ok_or(MemberFailure::ProjectNotFound)?;
    // 旧后端此处**显式传 memberRole: null**：改角色只认「项目创建者 / 平台 ADMIN」，
    // 不看操作者自己的成员角色。因此这里必须用内存版判定，不能走 DB 版（会解析出 memberRole）。
    let can = can_project_action(
        ProjectAction::MemberRole,
        &ProjectAccessContext {
            is_platform_admin,
            is_creator: project.user_id == actor_user_id,
            member_role: None,
        },
    );
    if !can {
        return Err(MemberFailure::Forbidden.into());
    }
    if member_id == actor_user_id {
        return Err(MemberFailure::CannotUpdateSelf.into());
    }
    let updated = project_member::update_project_member_role(pool, project_id, member_id, &role)
        .await?
        .ok_or(MemberFailure::MemberNotFound)?;
    Ok(updated)
}

/// DELETE /projects/:projectId/members/:memberId —— 移除成员。
pub async fn remove(
    pool: &PgPool,
    project_id: &str,
    actor_user_id: &str,
    is_platform_admin: bool,
    member_id: &str,
) -> Result<(), AppError> {
    let project = project::get_project_by_id_only(pool, project_id)
        .await?
        .ok_or(MemberFailure::ProjectNotFound)?;
    let is_self = member_id == actor_user_id;
    // 移除他人 = manager 档，需要操作者自身的成员角色参与判定；本人退出单独放行。
    let self_member = project_member::get_project_member(pool, project_id, actor_user_id).await?;
    let can_remove = can_project_action(
        ProjectAction::MemberRemove,
        &ProjectAccessContext {
            is_platform_admin,
            is_creator: project.user_id == actor_user_id,
            member_role: self_member.as_ref().map(|m| match m.role.as_str() {
                "OWNER" => ProjectRole::Owner,
                "ADMIN" => ProjectRole::Admin,
                _ => ProjectRole::Member,
            }),
        },
    );
    if !can_remove && !is_self {
        return Err(MemberFailure::Forbidden.into());
    }
    if member_id == project.user_id {
        return Err(MemberFailure::CannotRemoveOwner.into());
    }
    let ok = project_member::remove_project_member(pool, project_id, member_id).await?;
    if !ok {
        return Err(MemberFailure::MemberNotFound.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_failure_maps_to_same_status_and_code_as_legacy() {
        let cases: Vec<(MemberFailure, StatusCode, &str)> = vec![
            (MemberFailure::ProjectNotFound, StatusCode::NOT_FOUND, "PROJECT_NOT_FOUND"),
            (MemberFailure::UserNotFound, StatusCode::NOT_FOUND, "USER_NOT_FOUND"),
            (MemberFailure::CannotInviteSelf, StatusCode::BAD_REQUEST, "CANNOT_INVITE_SELF"),
            (MemberFailure::UserIsOwner, StatusCode::BAD_REQUEST, "USER_IS_OWNER"),
            (MemberFailure::EmptyPatch, StatusCode::BAD_REQUEST, "EMPTY_PATCH"),
            (MemberFailure::CannotUpdateSelf, StatusCode::BAD_REQUEST, "CANNOT_UPDATE_SELF"),
            (MemberFailure::CannotRemoveOwner, StatusCode::BAD_REQUEST, "CANNOT_REMOVE_OWNER"),
            (MemberFailure::MemberNotFound, StatusCode::NOT_FOUND, "MEMBER_NOT_FOUND"),
            (MemberFailure::Forbidden, StatusCode::FORBIDDEN, "FORBIDDEN"),
        ];
        for (failure, status, code) in cases {
            let err: AppError = failure.clone().into();
            assert_eq!(err.status, status, "status mismatch for {failure:?}");
            assert_eq!(err.code, code, "code mismatch for {failure:?}");
        }
    }
}
