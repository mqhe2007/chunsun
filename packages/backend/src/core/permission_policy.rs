//! 权限策略 SSOT（纯函数，无 DB / 框架依赖）。
//!
//! 全系统项目级权限的单一事实源，1:1 移植自 `packages/backend/src/lib/permissionPolicy.ts`。
//! 后续需把 `scripts/gen-permissions.ts` 的生成链改为以本模块为源生成前端镜像。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ProjectRole {
    Owner,
    Admin,
    Member,
}

impl ProjectRole {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectRole::Owner => "OWNER",
            ProjectRole::Admin => "ADMIN",
            ProjectRole::Member => "MEMBER",
        }
    }
}

/// 项目级权限档位（由低到高）：member < manager < owner。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectPrivilegeLevel {
    Member,
    Manager,
    Owner,
}

/// 项目动作全集。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectAction {
    RequirementWrite,
    DefectWrite,
    TestWrite,
    RunWrite,
    ScenarioWrite,
    RepositoryWrite,
    ContextWrite,
    PromptWrite,
    SecretKeyRead,
    EnvVarWrite,
    SecretKeyWrite,
    MemberInvite,
    MemberRemove,
    ProjectUpdate,
    MemberRole,
    ProjectDelete,
}

impl ProjectAction {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectAction::RequirementWrite => "requirement.write",
            ProjectAction::DefectWrite => "defect.write",
            ProjectAction::TestWrite => "test.write",
            ProjectAction::RunWrite => "run.write",
            ProjectAction::ScenarioWrite => "scenario.write",
            ProjectAction::RepositoryWrite => "repository.write",
            ProjectAction::ContextWrite => "context.write",
            ProjectAction::PromptWrite => "prompt.write",
            ProjectAction::SecretKeyRead => "secretKey.read",
            ProjectAction::EnvVarWrite => "envVar.write",
            ProjectAction::SecretKeyWrite => "secretKey.write",
            ProjectAction::MemberInvite => "member.invite",
            ProjectAction::MemberRemove => "member.remove",
            ProjectAction::ProjectUpdate => "project.update",
            ProjectAction::MemberRole => "member.role",
            ProjectAction::ProjectDelete => "project.delete",
        }
    }
}

pub struct ProjectActionMeta {
    pub level: ProjectPrivilegeLevel,
    /// 文档展示用中文标签
    #[allow(dead_code)]
    pub label: &'static str,
    /// 文档分层：collab / config / owner
    #[allow(dead_code)]
    pub group: &'static str,
}

/// 动作 → 最低档位 矩阵（SSOT）。
/// 注意：member.remove 语义为「移除他人」= manager；本人退出由路由单独处理，不走本矩阵。
pub const PROJECT_ACTIONS: &[(ProjectAction, ProjectActionMeta)] = &[
    (
        ProjectAction::RequirementWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "需求增删改", group: "collab" },
    ),
    (
        ProjectAction::DefectWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "缺陷增删改", group: "collab" },
    ),
    (
        ProjectAction::TestWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "测试场景 / 用例读写", group: "collab" },
    ),
    (
        ProjectAction::RunWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "轮次 / 步骤 读写（自主交付）", group: "collab" },
    ),
    (
        ProjectAction::ScenarioWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "验收场景 / 用例读写（需求挂载）", group: "collab" },
    ),
    (
        ProjectAction::RepositoryWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "代码仓管理", group: "collab" },
    ),
    (
        ProjectAction::ContextWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "上下文文档 / 宪法 / 需求工作记忆", group: "collab" },
    ),
    (
        ProjectAction::PromptWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "提示词配置", group: "collab" },
    ),
    (
        ProjectAction::SecretKeyRead,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Member, label: "查看 / 复制 Secret Key", group: "collab" },
    ),
    (
        ProjectAction::EnvVarWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Manager, label: "环境变量增删改", group: "config" },
    ),
    (
        ProjectAction::SecretKeyWrite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Manager, label: "Secret Key 生成 / 撤销", group: "config" },
    ),
    (
        ProjectAction::MemberInvite,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Manager, label: "邀请成员", group: "config" },
    ),
    (
        ProjectAction::MemberRemove,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Manager, label: "移除其他成员", group: "config" },
    ),
    (
        ProjectAction::ProjectUpdate,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Manager, label: "项目名称 / 描述 / 测试开关", group: "config" },
    ),
    (
        ProjectAction::MemberRole,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Owner, label: "修改成员角色", group: "owner" },
    ),
    (
        ProjectAction::ProjectDelete,
        ProjectActionMeta { level: ProjectPrivilegeLevel::Owner, label: "删除项目", group: "owner" },
    ),
];

pub fn action_level(action: ProjectAction) -> ProjectPrivilegeLevel {
    PROJECT_ACTIONS
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, m)| m.level)
        .expect("PROJECT_ACTIONS 矩阵必须覆盖全部动作")
}

#[derive(Debug, Clone)]
pub struct ProjectAccessContext {
    /// 平台超管（UserRole.ADMIN），对所有项目全量放行
    pub is_platform_admin: bool,
    /// 是否项目创建者（project.userId === userId）
    pub is_creator: bool,
    /// 当前用户在该项目的成员角色；非成员为 None
    pub member_role: Option<ProjectRole>,
}

/// 纯判定：给定档位与访问上下文，是否放行。
pub fn meets_level(level: ProjectPrivilegeLevel, ctx: &ProjectAccessContext) -> bool {
    if ctx.is_platform_admin {
        return true;
    }
    if ctx.is_creator {
        return true; // 创建者满足全部档位
    }
    match level {
        ProjectPrivilegeLevel::Member => ctx.member_role.is_some(),
        ProjectPrivilegeLevel::Manager => {
            matches!(ctx.member_role, Some(ProjectRole::Owner | ProjectRole::Admin))
        }
        ProjectPrivilegeLevel::Owner => false, // owner 档仅创建者 / 平台 ADMIN
    }
}

/// 纯判定：当前上下文能否执行某动作。
pub fn can_project_action(action: ProjectAction, ctx: &ProjectAccessContext) -> bool {
    meets_level(action_level(action), ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(is_platform_admin: bool, is_creator: bool, member_role: Option<ProjectRole>) -> ProjectAccessContext {
        ProjectAccessContext { is_platform_admin, is_creator, member_role }
    }

    #[test]
    fn matrix_covers_all_actions_with_levels() {
        assert_eq!(action_level(ProjectAction::RequirementWrite), ProjectPrivilegeLevel::Member);
        assert_eq!(action_level(ProjectAction::EnvVarWrite), ProjectPrivilegeLevel::Manager);
        assert_eq!(action_level(ProjectAction::MemberRole), ProjectPrivilegeLevel::Owner);
        assert_eq!(action_level(ProjectAction::ProjectDelete), ProjectPrivilegeLevel::Owner);
        // member 档 9 个、manager 档 5 个、owner 档 2 个（与 TS SSOT 一致）
        let member_count = PROJECT_ACTIONS
            .iter()
            .filter(|(_, m)| matches!(m.level, ProjectPrivilegeLevel::Member))
            .count();
        let manager_count = PROJECT_ACTIONS
            .iter()
            .filter(|(_, m)| matches!(m.level, ProjectPrivilegeLevel::Manager))
            .count();
        let owner_count = PROJECT_ACTIONS
            .iter()
            .filter(|(_, m)| matches!(m.level, ProjectPrivilegeLevel::Owner))
            .count();
        assert_eq!((member_count, manager_count, owner_count), (9, 5, 2));
        assert_eq!(PROJECT_ACTIONS.len(), 16);
    }

    #[test]
    fn platform_admin_passes_everything() {
        let c = ctx(true, false, None);
        assert!(can_project_action(ProjectAction::ProjectDelete, &c));
        assert!(can_project_action(ProjectAction::EnvVarWrite, &c));
        assert!(can_project_action(ProjectAction::RequirementWrite, &c));
    }

    #[test]
    fn creator_passes_everything_even_not_member() {
        let c = ctx(false, true, None);
        assert!(can_project_action(ProjectAction::ProjectDelete, &c));
        assert!(can_project_action(ProjectAction::MemberRole, &c));
    }

    #[test]
    fn member_level_requires_membership() {
        assert!(can_project_action(ProjectAction::RequirementWrite, &ctx(false, false, Some(ProjectRole::Member))));
        assert!(!can_project_action(ProjectAction::RequirementWrite, &ctx(false, false, None)));
    }

    #[test]
    fn manager_level_requires_owner_or_admin() {
        let c_owner = ctx(false, false, Some(ProjectRole::Owner));
        let c_admin = ctx(false, false, Some(ProjectRole::Admin));
        let c_member = ctx(false, false, Some(ProjectRole::Member));
        assert!(can_project_action(ProjectAction::EnvVarWrite, &c_owner));
        assert!(can_project_action(ProjectAction::EnvVarWrite, &c_admin));
        assert!(!can_project_action(ProjectAction::EnvVarWrite, &c_member));
        assert!(!can_project_action(ProjectAction::EnvVarWrite, &ctx(false, false, None)));
    }

    #[test]
    fn owner_level_only_creator_or_admin() {
        let c_owner = ctx(false, false, Some(ProjectRole::Owner));
        assert!(!can_project_action(ProjectAction::MemberRole, &c_owner));
        assert!(!can_project_action(ProjectAction::ProjectDelete, &c_owner));
        assert!(can_project_action(ProjectAction::ProjectDelete, &ctx(false, true, None)));
        assert!(can_project_action(ProjectAction::ProjectDelete, &ctx(true, false, None)));
    }
}
