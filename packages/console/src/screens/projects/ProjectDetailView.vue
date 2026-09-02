<script setup lang="ts">
import { BookOpen, Key, Search, type LucideIcon } from "@lucide/vue";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useToast } from "@/ui";
import { useRoute, useRouter } from "vue-router";
import { api } from "@/utils/api";
import { useProjectPermissions } from "@/utils/permissions";
import ProjectDashboardNav from "@/components/projects/ProjectDashboardNav.vue";
import SettingsNav from "@/components/projects/SettingsNav.vue";
import type { Project, ProjectMember } from "@/types/project";

type WorkflowAction = {
  key: "secret-key" | "help";
  label: string;
  icon: LucideIcon;
  outlined?: boolean;
};

const DASHBOARD_TAB_RE = /\/(overview|requirements|defects|dependencies)(\/|$)/;

const route = useRoute();
const router = useRouter();
const toast = useToast();
const loading = ref(false);
const project = ref<Project | null>(null);
const members = ref<ProjectMember[]>([]);

const { can } = useProjectPermissions(
  () => project.value?.userId ?? null,
  members,
);
const projectId = computed(() => (route.params as Record<string, string>).id ?? "");

const isDashboardIndex = computed(() => {
  if (!projectId.value) return false;
  return route.path.replace(/\/$/, "") === `/projects/${projectId.value}`;
});

const isDashboardTabRoute = computed(() => DASHBOARD_TAB_RE.test(route.path));

const isSettingsRoute = computed(() =>
  Boolean(projectId.value) &&
  route.path.includes(`/projects/${projectId.value}/settings`),
);

const isRecordRoute = computed(() => {
  const params = route.params as Record<string, string>;
  return Boolean(params.requirementId || params.defectId);
});

async function fetchProject() {
  const id = projectId.value;
  if (!id) return;

  loading.value = true;
  secretKeyReady.value = false;
  try {
    const { data } = await api.get<{ success: boolean; data: Project }>(`/projects/${id}`);
    if (data.success) {
      project.value = data.data;
      secretKeyReady.value = true;
      void fetchMembers();
    } else {
      project.value = null;
      toast.add({ severity: "error", summary: "获取失败", detail: "项目不存在", life: 3000 });
    }
  } catch {
    project.value = null;
    toast.add({ severity: "error", summary: "获取失败", detail: "获取项目失败", life: 3000 });
  } finally {
    loading.value = false;
  }
}

async function fetchMembers() {
  const id = projectId.value;
  if (!id) return;
  try {
    const { data } = await api.get<{ success: boolean; data: ProjectMember[] }>(
      `/projects/${id}/members`,
    );
    if (data.success) members.value = data.data;
  } catch {
    members.value = [];
  }
}

function handleWorkflowAction(key: WorkflowAction["key"]) {
  if (key === "secret-key") {
    router.push(`/projects/${projectId.value}/settings/secret-key`);
    return;
  }
  // 使用帮助已迁移至营销侧文档中心
  location.assign("/docs");
}

const secretKeyReady = ref(false);

const canManageSecretKey = computed(() => can("secretKey.write"));

const hasSecretKey = computed(() => Boolean(project.value?.hasSecretKey));
const isProjectReady = computed(() => hasSecretKey.value);

const workflowGuide = computed<{
  title: string;
  summary: string;
  hint?: string;
  actions: WorkflowAction[];
}>(() => {
  const helpAction: WorkflowAction = {
    key: "help",
    label: "查看文档",
    icon: BookOpen,
    outlined: true,
  };

  if (!hasSecretKey.value && !canManageSecretKey.value) {
    return {
      title: "等待管理员配置项目密钥",
      summary: "当前账号无密钥管理权限，且项目尚未配置密钥。请联系项目管理员生成密钥后再继续。",
      hint: "可通过页头「设置」进入项目密钥页查看当前状态。",
      actions: [
        { key: "secret-key", label: "查看密钥状态", icon: Key },
        helpAction,
      ],
    };
  }

  return {
    title: "先生成项目密钥",
    summary: "补齐项目密钥后，再按使用帮助配置本地 .env 并安装 Skill，即可完成接入。",
    hint: "密钥生成后请妥善保管，重新生成会使旧密钥失效。",
    actions: [
      { key: "secret-key", label: "生成项目密钥", icon: Key },
      helpAction,
    ],
  };
});

async function refreshSecretKeyState() {
  if (!project.value) return;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: { secretKey: string | null; hasSecretKey: boolean };
    }>(`/projects/${project.value.id}/secret-key`);
    if (data.success && project.value) {
      project.value = {
        ...project.value,
        hasSecretKey: data.data.hasSecretKey,
      };
    }
  } catch {
    // 非成员无权限时静默忽略
  } finally {
    secretKeyReady.value = true;
  }
}

function syncDashboardRoute() {
  if (!project.value || !secretKeyReady.value) return;

  if (isProjectReady.value) {
    if (isDashboardIndex.value) {
      router.replace(`/projects/${project.value.id}/overview`);
    }
    return;
  }

  // 未就绪时允许停留在 settings（尤其是项目密钥页）
  if (isSettingsRoute.value) return;

  if (isDashboardTabRoute.value) {
    router.replace(`/projects/${project.value.id}`);
  }
}

watch(
  [isProjectReady, secretKeyReady, () => route.path, project],
  () => {
    syncDashboardRoute();
  },
);

// 从密钥页返回后刷新就绪状态
watch(
  () => route.path,
  async (path, prev) => {
    if (
      prev?.includes("/settings/secret-key") &&
      !path.includes("/settings/secret-key")
    ) {
      await refreshSecretKeyState();
    }
  },
);

watch(projectId, async (id, prevId) => {
  if (!id || id === prevId) return;
  await fetchProject();
  syncDashboardRoute();
});

onMounted(async () => {
  await fetchProject();
  syncDashboardRoute();
  window.addEventListener("chunsun:secret-key-changed", onSecretKeyChanged);
  window.addEventListener("chunsun:project-updated", onProjectUpdated);
});

onUnmounted(() => {
  window.removeEventListener("chunsun:secret-key-changed", onSecretKeyChanged);
  window.removeEventListener("chunsun:project-updated", onProjectUpdated);
});

async function onSecretKeyChanged() {
  await refreshSecretKeyState();
  syncDashboardRoute();
}

async function onProjectUpdated() {
  const id = projectId.value;
  if (!id) return;
  try {
    const { data } = await api.get<{ success: boolean; data: Project }>(`/projects/${id}`);
    if (data.success) project.value = data.data;
  } catch {
    // 编辑后同步刷新失败时静默保留现有数据
  }
}
</script>

<template>
  <div class="project-detail flex min-w-0 flex-col gap-5">
    <div v-if="loading" class="flex flex-col items-center justify-center gap-4 py-16 text-center">
      <span class="loading loading-spinner loading-lg text-primary" />
      <p class="text-base-content/60">加载中...</p>
    </div>

    <template v-else-if="project">
      <div class="flex shrink-0 flex-wrap items-center justify-between gap-4">
        <div class="flex min-w-0 flex-col gap-1">
          <h1 class="text-2xl font-bold tracking-tight">{{ project.name }}</h1>
          <p class="text-sm text-base-content/60">
            {{ project.description || `${new Date(project.createdAt).toLocaleDateString()} 创建` }}
          </p>
        </div>
        <ProjectDashboardNav
          v-if="!isRecordRoute"
          class="shrink-0"
          :project-id="project.id"
        />
      </div>
      <SettingsNav
        v-if="isSettingsRoute && !isRecordRoute"
        class="shrink-0"
        :project-id="project.id"
      />

      <template v-if="isSettingsRoute || isProjectReady">
        <div class="dashboard-outlet">
          <RouterView />
        </div>
      </template>

      <!-- 仅在密钥状态确认后再展示引导卡，避免已配置项目闪一下未就绪态 -->
      <div v-else-if="secretKeyReady" class="card workflow-card">
        <div class="card-body workflow-shell">
          <div class="workflow-head">
            <div>
              <h2 class="workflow-title">{{ workflowGuide.title }}</h2>
              <p class="workflow-summary">{{ workflowGuide.summary }}</p>
            </div>
          </div>
          <div v-if="workflowGuide.actions.length" class="workflow-actions">
            <button
              v-for="(action, index) in workflowGuide.actions"
              :key="action.key"
              type="button"
              class="btn"
              :class="(action.outlined ?? index !== 0) ? 'btn-ghost' : 'btn-primary'"
              @click="handleWorkflowAction(action.key)"
            >
              <component :is="action.icon" :size="14" aria-hidden="true" />
              {{ action.label }}
            </button>
          </div>
          <p v-if="workflowGuide.hint" class="workflow-hint">{{ workflowGuide.hint }}</p>
        </div>
      </div>
    </template>

    <div v-else class="console-empty-state">
      <Search class="empty-icon text-base-content/40" :size="40" aria-hidden="true" />
      <p class="text-base-content/60">项目不存在或已被删除</p>
      <button type="button" class="btn btn-ghost" @click="router.push('/projects')">
        返回项目列表
      </button>
    </div>
  </div>
</template>

<style scoped>
/* 项目内容区默认撑满剩余高度，使依赖图等需要全屏的子页面获得确定高度 */
.dashboard-outlet {
  flex: 1 1 auto;
  min-block-size: 0;
  min-inline-size: 0;
}

/* 需求看板 / 依赖图等需要撑满视口的子页面：打通从 project-detail 到子页面的高度链 */
.project-detail:has(.req-page--board),
.project-detail:has(.dep-graph) {
  flex: 1 1 auto;
  min-block-size: 0;
  overflow: hidden;
}

.project-detail:has(.req-page--board) .dashboard-outlet,
.project-detail:has(.dep-graph) .dashboard-outlet {
  display: flex;
  flex: 1 1 auto;
  min-block-size: 0;
  flex-direction: column;
  overflow: hidden;
}

.project-detail:has(.req-page--board) .dashboard-outlet > *,
.project-detail:has(.dep-graph) .dashboard-outlet > * {
  display: flex;
  flex: 1 1 auto;
  min-block-size: 0;
  min-inline-size: 0;
  flex-direction: column;
  overflow: hidden;
}

.workflow-card {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--color-primary) 8%, var(--color-base-100)) 0%,
    var(--color-base-100) 68%
  );
}

.workflow-shell {
  display: grid;
  gap: 1rem;
}

.workflow-head {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  align-items: flex-start;
}

.workflow-title {
  margin: 0.35rem 0 0;
  font-size: 1.35rem;
  color: var(--color-base-content);
}

.workflow-summary {
  margin: 0.45rem 0 0;
  max-width: 46rem;
  line-height: 1.65;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.workflow-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.6rem;
}

.workflow-hint {
  margin: 0;
  font-size: 0.85rem;
  line-height: 1.55;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.empty-icon {
  font-size: 3rem;
  line-height: 1;
}

@media (max-width: 560px) {
  .workflow-head {
    flex-direction: column;
    align-items: flex-start;
  }

  .workflow-actions {
    flex-direction: column;
  }

  .workflow-actions .btn {
    width: 100%;
  }
}
</style>
