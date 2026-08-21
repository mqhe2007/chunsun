import { computed, toValue, type MaybeRefOrGetter } from "vue";
import { useAuthStore } from "@/stores/auth";
import type { ProjectMember } from "@/types/project";
import {
  canProjectAction,
  type ProjectAccessContext,
  type ProjectAction,
} from "./permissionPolicy.generated";

export { canProjectAction } from "./permissionPolicy.generated";
export type {
  ProjectAction,
  ProjectAccessContext,
} from "./permissionPolicy.generated";

/**
 * 项目权限组合式：给定项目创建者 id 与成员列表，返回响应式的 `can(action)`。
 * 策略来自 `permissionPolicy.generated.ts`（由后端 SSOT 生成），与后端判定完全一致。
 *
 * @example
 * const { can } = useProjectPermissions(ownerId, members);
 * const canManage = computed(() => can("application.write"));
 */
export function useProjectPermissions(
  ownerId: MaybeRefOrGetter<string | null | undefined>,
  members: MaybeRefOrGetter<ProjectMember[]>,
) {
  const authStore = useAuthStore();

  const context = computed<ProjectAccessContext>(() => {
    const owner = toValue(ownerId);
    return {
      isPlatformAdmin: authStore.isAdmin,
      isCreator: owner != null && authStore.userId === owner,
      memberRole:
        toValue(members).find(m => m.userId === authStore.userId)?.role ?? null,
    };
  });

  const can = (action: ProjectAction): boolean =>
    canProjectAction(action, context.value);

  return { context, can };
}
