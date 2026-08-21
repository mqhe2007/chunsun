/* eslint-disable */
/**
 * AUTO-GENERATED —— 请勿手改。
 * 由 `scripts/gen-permissions.ts` 从后端 SSOT
 * `packages/backend/src/core/permission_policy.rs` 生成。
 * 修改权限策略：改后端 SSOT，再运行 `pnpm gen:permissions`。
 */

export type ProjectRole = "OWNER" | "ADMIN" | "MEMBER";

export type ProjectPrivilegeLevel = "member" | "manager" | "owner";

export type ProjectAction =
  | "requirement.write"
  | "defect.write"
  | "test.write"
  | "run.write"
  | "scenario.write"
  | "repository.write"
  | "context.write"
  | "prompt.write"
  | "secretKey.read"
  | "envVar.write"
  | "secretKey.write"
  | "member.invite"
  | "member.remove"
  | "project.update"
  | "member.role"
  | "project.delete";

export type ProjectActionMeta = {
  level: ProjectPrivilegeLevel;
  /** 文档展示用中文标签 */
  label: string;
  /** 文档分层：collab / config / owner */
  group: "collab" | "config" | "owner";
};

export const PROJECT_ACTIONS: Record<ProjectAction, ProjectActionMeta> = {
  "requirement.write": { level: "member", label: "需求增删改", group: "collab" },
  "defect.write": { level: "member", label: "缺陷增删改", group: "collab" },
  "test.write": { level: "member", label: "测试场景 / 用例读写", group: "collab" },
  "run.write": { level: "member", label: "轮次 / 步骤 读写（自主交付）", group: "collab" },
  "scenario.write": { level: "member", label: "验收场景 / 用例读写（需求挂载）", group: "collab" },
  "repository.write": { level: "member", label: "代码仓管理", group: "collab" },
  "context.write": { level: "member", label: "上下文文档 / 宪法 / 需求工作记忆", group: "collab" },
  "prompt.write": { level: "member", label: "提示词配置", group: "collab" },
  "secretKey.read": { level: "member", label: "查看 / 复制 Secret Key", group: "collab" },
  "envVar.write": { level: "manager", label: "环境变量增删改", group: "config" },
  "secretKey.write": { level: "manager", label: "Secret Key 生成 / 撤销", group: "config" },
  "member.invite": { level: "manager", label: "邀请成员", group: "config" },
  "member.remove": { level: "manager", label: "移除其他成员", group: "config" },
  "project.update": { level: "manager", label: "项目名称 / 描述 / 测试开关", group: "config" },
  "member.role": { level: "owner", label: "修改成员角色", group: "owner" },
  "project.delete": { level: "owner", label: "删除项目", group: "owner" },
};

export type ProjectAccessContext = {
  /** 平台超管（UserRole.ADMIN），对所有项目全量放行 */
  isPlatformAdmin: boolean;
  /** 是否项目创建者（project.userId === userId） */
  isCreator: boolean;
  /** 当前用户在该项目的成员角色；非成员为 null */
  memberRole: ProjectRole | null;
};

/**
 * 后端 `meets_level` 的判定真值表，覆盖全部上下文组合。
 * 前端不重写判定逻辑，直接查表，确保与后端逐位一致。
 */
const DECISIONS: Record<string, boolean> = {
  "manager|0|0|ADMIN": true,
  "manager|0|0|MEMBER": false,
  "manager|0|0|NONE": false,
  "manager|0|0|OWNER": true,
  "manager|0|1|ADMIN": true,
  "manager|0|1|MEMBER": true,
  "manager|0|1|NONE": true,
  "manager|0|1|OWNER": true,
  "manager|1|0|ADMIN": true,
  "manager|1|0|MEMBER": true,
  "manager|1|0|NONE": true,
  "manager|1|0|OWNER": true,
  "manager|1|1|ADMIN": true,
  "manager|1|1|MEMBER": true,
  "manager|1|1|NONE": true,
  "manager|1|1|OWNER": true,
  "member|0|0|ADMIN": true,
  "member|0|0|MEMBER": true,
  "member|0|0|NONE": false,
  "member|0|0|OWNER": true,
  "member|0|1|ADMIN": true,
  "member|0|1|MEMBER": true,
  "member|0|1|NONE": true,
  "member|0|1|OWNER": true,
  "member|1|0|ADMIN": true,
  "member|1|0|MEMBER": true,
  "member|1|0|NONE": true,
  "member|1|0|OWNER": true,
  "member|1|1|ADMIN": true,
  "member|1|1|MEMBER": true,
  "member|1|1|NONE": true,
  "member|1|1|OWNER": true,
  "owner|0|0|ADMIN": false,
  "owner|0|0|MEMBER": false,
  "owner|0|0|NONE": false,
  "owner|0|0|OWNER": false,
  "owner|0|1|ADMIN": true,
  "owner|0|1|MEMBER": true,
  "owner|0|1|NONE": true,
  "owner|0|1|OWNER": true,
  "owner|1|0|ADMIN": true,
  "owner|1|0|MEMBER": true,
  "owner|1|0|NONE": true,
  "owner|1|0|OWNER": true,
  "owner|1|1|ADMIN": true,
  "owner|1|1|MEMBER": true,
  "owner|1|1|NONE": true,
  "owner|1|1|OWNER": true,
};

export function meetsLevel(
  level: ProjectPrivilegeLevel,
  ctx: ProjectAccessContext,
): boolean {
  const key = `${level}|${ctx.isPlatformAdmin ? 1 : 0}|${ctx.isCreator ? 1 : 0}|${ctx.memberRole ?? "NONE"}`;
  return DECISIONS[key] ?? false;
}

export function canProjectAction(
  action: ProjectAction,
  ctx: ProjectAccessContext,
): boolean {
  return meetsLevel(PROJECT_ACTIONS[action].level, ctx);
}
