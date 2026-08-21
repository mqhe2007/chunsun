/**
 * 权限策略生成器：从后端 SSOT `packages/backend/src/core/permission_policy.rs` 生成
 * 前端镜像 `packages/console/src/utils/permissionPolicy.generated.ts`
 *
 * 后端从 TypeScript 换成 Rust 后，不能再像过去那样把纯策略模块逐字复制给前端。
 * 改为让后端导出 JSON（`cargo run --example dump_permissions`），其中除动作元数据
 * 外还含 `meets_level` 的完整判定真值表；前端镜像据此生成为纯查表实现，两端不再
 * 各写一份判定逻辑，也就不会漂移。
 *
 * 用法：`pnpm gen:permissions`
 */
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const BACKEND_DIR = path.join(ROOT, "packages/backend");
export const BACKEND_POLICY_PATH = path.join(
  BACKEND_DIR,
  "src/core/permission_policy.rs",
);
export const FRONTEND_GENERATED_PATH = path.join(
  ROOT,
  "packages/console/src/utils/permissionPolicy.generated.ts",
);

export type PolicyGroup = "collab" | "config" | "owner";
export type PolicyLevel = "member" | "manager" | "owner";

export type PolicyAction = {
  key: string;
  level: PolicyLevel;
  label: string;
  group: PolicyGroup;
};

export type PolicyDump = {
  actions: PolicyAction[];
  /** `level|isPlatformAdmin|isCreator|memberRole` -> 是否放行 */
  decisions: Record<string, boolean>;
};

/** 调后端 example 拿策略 JSON（SSOT 唯一出口）。 */
export function loadPolicy(): PolicyDump {
  const stdout = execFileSync(
    "cargo",
    ["run", "--quiet", "--example", "dump_permissions"],
    { cwd: BACKEND_DIR, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  );
  return JSON.parse(stdout) as PolicyDump;
}

const FRONTEND_BANNER = `/* eslint-disable */
/**
 * AUTO-GENERATED —— 请勿手改。
 * 由 \`scripts/gen-permissions.ts\` 从后端 SSOT
 * \`packages/backend/src/core/permission_policy.rs\` 生成。
 * 修改权限策略：改后端 SSOT，再运行 \`pnpm gen:permissions\`。
 */`;

/** 依据后端导出的矩阵与真值表，生成前端镜像模块。 */
export function renderFrontendModule(dump: PolicyDump): string {
  const actionUnion = dump.actions
    .map(a => `  | ${JSON.stringify(a.key)}`)
    .join("\n");

  const actionEntries = dump.actions
    .map(
      a =>
        `  ${JSON.stringify(a.key)}: { level: ${JSON.stringify(a.level)}, label: ${JSON.stringify(a.label)}, group: ${JSON.stringify(a.group)} },`,
    )
    .join("\n");

  const decisionEntries = Object.entries(dump.decisions)
    .map(([k, v]) => `  ${JSON.stringify(k)}: ${v},`)
    .join("\n");

  return `${FRONTEND_BANNER}

export type ProjectRole = "OWNER" | "ADMIN" | "MEMBER";

export type ProjectPrivilegeLevel = "member" | "manager" | "owner";

export type ProjectAction =
${actionUnion};

export type ProjectActionMeta = {
  level: ProjectPrivilegeLevel;
  /** 文档展示用中文标签 */
  label: string;
  /** 文档分层：collab / config / owner */
  group: "collab" | "config" | "owner";
};

export const PROJECT_ACTIONS: Record<ProjectAction, ProjectActionMeta> = {
${actionEntries}
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
 * 后端 \`meets_level\` 的判定真值表，覆盖全部上下文组合。
 * 前端不重写判定逻辑，直接查表，确保与后端逐位一致。
 */
const DECISIONS: Record<string, boolean> = {
${decisionEntries}
};

export function meetsLevel(
  level: ProjectPrivilegeLevel,
  ctx: ProjectAccessContext,
): boolean {
  const key = \`\${level}|\${ctx.isPlatformAdmin ? 1 : 0}|\${ctx.isCreator ? 1 : 0}|\${ctx.memberRole ?? "NONE"}\`;
  return DECISIONS[key] ?? false;
}

export function canProjectAction(
  action: ProjectAction,
  ctx: ProjectAccessContext,
): boolean {
  return meetsLevel(PROJECT_ACTIONS[action].level, ctx);
}
`;
}

/**
 * 防漂移校验：只比对不写盘，磁盘内容与 SSOT 生成结果不一致则退出码 1。
 * 用于 CI / 提交前，确保改了后端策略却忘了重跑生成器时能立刻暴露。
 */
function check(dump: PolicyDump): number {
  const expectedFrontend = renderFrontendModule(dump);
  if (readFileSync(FRONTEND_GENERATED_PATH, "utf8") !== expectedFrontend) {
    return 1;
  }

  // eslint-disable-next-line no-console
  console.log("[gen-permissions] ✓ 前端镜像与后端 SSOT 一致");
  return 0;
}

function main(): void {
  const dump = loadPolicy();

  if (process.argv.includes("--check")) {
    process.exit(check(dump));
  }

  writeFileSync(FRONTEND_GENERATED_PATH, renderFrontendModule(dump), "utf8");
  // eslint-disable-next-line no-console
  console.log(
    `[gen-permissions] 已从 ${path.relative(ROOT, BACKEND_POLICY_PATH)} 生成：\n  - ` +
      path.relative(ROOT, FRONTEND_GENERATED_PATH),
  );
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main();
}
