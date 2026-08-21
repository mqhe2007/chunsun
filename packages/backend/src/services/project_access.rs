//! 项目级权限的 DB 判定层（对齐 `lib/projectAccess.ts`）。
//!
//! 把 `core/permission_policy`（纯 SSOT）与仓储查询串起来，供成员域路由统一取权限判断。

use sqlx::PgPool;

use crate::api::AppError;
use crate::core::permission_policy::{can_project_action, ProjectAccessContext, ProjectAction};
use crate::repos::project;
use crate::repos::project_member;

/// 解析当前用户对某项目的访问上下文。
/// 平台 ADMIN 直接返回全量放行上下文（不校验项目是否存在——由调用方按需 404）。
/// 项目不存在（且非平台 ADMIN）返回 None。
pub async fn resolve_project_access(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_platform_admin: bool,
) -> Result<Option<ProjectAccessContext>, AppError> {
    if is_platform_admin {
        return Ok(Some(ProjectAccessContext {
            is_platform_admin: true,
            is_creator: false,
            member_role: None,
        }));
    }
    let project = project::get_project_by_id_only(pool, project_id).await?;
    let Some(project) = project else {
        return Ok(None);
    };
    let member = project_member::get_project_member(pool, project_id, user_id).await?;
    use crate::core::permission_policy::ProjectRole;
    let member_role = member.map(|m| match m.role.as_str() {
        "OWNER" => ProjectRole::Owner,
        "ADMIN" => ProjectRole::Admin,
        _ => ProjectRole::Member,
    });
    Ok(Some(ProjectAccessContext {
        is_platform_admin: false,
        is_creator: project.user_id == user_id,
        member_role,
    }))
}

/// 判「项目对当前用户可见」并返回项目 id；不可见一律 **404 PROJECT_NOT_FOUND**（不是 403）。
///
/// 可见性口径 = `getProjectById`（创建者 ∪ 项目成员，平台 ADMIN 全通）。多个域
/// （repository / requirement / …）的每个 handler 开头都是这一句，抽出来避免各域各抄一份。
pub async fn visible_project_id(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
    is_admin: bool,
) -> Result<String, AppError> {
    use crate::services::project::ProjectFailure;
    let project = project::get_project_by_id(pool, project_id, user_id, is_admin)
        .await?
        .ok_or(ProjectFailure::ProjectNotFound)?;
    Ok(project.id)
}

/// DB 版动作判定：解析上下文并套用 SSOT 矩阵。
pub async fn can_project_action_db(
    pool: &PgPool,
    action: ProjectAction,
    project_id: &str,
    user_id: &str,
    is_platform_admin: bool,
) -> Result<bool, AppError> {
    let ctx = resolve_project_access(pool, project_id, user_id, is_platform_admin).await?;
    Ok(match ctx {
        Some(c) => can_project_action(action, &c),
        None => false,
    })
}
