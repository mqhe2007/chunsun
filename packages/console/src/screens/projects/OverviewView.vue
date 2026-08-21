<script setup lang="ts">
import { ChevronRight, RefreshCw } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { AppPage, useToast } from "@/ui";
import ActivityHeatmap from "@/components/projects/ActivityHeatmap.vue";
import UserAvatar from "@/components/common/UserAvatar.vue";
import { api } from "@/utils/api";
import type { Project, ProjectActivity } from "@/types/project";
import { REQUIREMENT_STATUS_LABEL } from "@/utils/workflow";
import { activityIcon, activityLink } from "@/utils/activity";
import { formatRelativeTime } from "@/utils/time";

/** Meter 段需要具体色值；对齐 green primary + olive surface（docs/ui_design.md） */
const REQ_STATUS_COLOR: Record<string, string> = {
  pending: "#a3ab96",
  running: "#2563eb",
  abandoned: "#6b7280",
  completed: "#15803d",
};

const route = useRoute();
const router = useRouter();
const toast = useToast();

const initialLoading = ref(true);
const refreshing = ref(false);
const project = ref<Project | null>(null);
const activities = ref<ProjectActivity[]>([]);

type HeatmapEntry = { date: string; count: number };
type HeatmapData = {
  windowDays: number;
  max: number;
  entries: HeatmapEntry[];
};
const heatmap = ref<HeatmapData | null>(null);

const projectId = () => (route.params as Record<string, string>).id;

const req = computed(() => project.value?.statistics?.requirements);
const rates = computed(() => project.value?.statistics?.rates);
const defects = computed(() => project.value?.statistics?.defects);

const heatmapTotal = computed(() =>
  heatmap.value?.entries.reduce((sum, e) => sum + e.count, 0) ?? 0,
);
const heatmapPeak = computed(() => {
  const entries = heatmap.value?.entries ?? [];
  if (entries.length === 0 || (heatmap.value?.max ?? 0) === 0) return null;
  const peak = entries.find(e => e.count === heatmap.value!.max);
  return peak ?? null;
});

const reqTotal = computed(() => Math.max(req.value?.total ?? 0, 0));
const reqRunning = computed(() => (req.value?.running ?? 0) + (req.value?.processing ?? 0));

/** 未解决缺陷 = 待处理 + 处理中 */
const defectUnresolved = computed(
  () => (defects.value?.open ?? 0) + (defects.value?.processing ?? 0),
);

const reqCompletion = computed(
  () => rates.value?.requirementCompletionPct ?? 0,
);

const reqMeter = computed(() => {
  const total = reqTotal.value || 1;
  return [
    {
      key: "pending",
      label: REQUIREMENT_STATUS_LABEL.pending,
      count: req.value?.pending ?? 0,
      color: REQ_STATUS_COLOR.pending,
    },
    {
      key: "running",
      label: REQUIREMENT_STATUS_LABEL.running,
      count: reqRunning.value,
      color: REQ_STATUS_COLOR.running,
    },
    {
      key: "abandoned",
      label: REQUIREMENT_STATUS_LABEL.abandoned,
      count: req.value?.abandoned ?? 0,
      color: REQ_STATUS_COLOR.abandoned,
    },
    {
      key: "completed",
      label: REQUIREMENT_STATUS_LABEL.completed,
      count: req.value?.completed ?? 0,
      color: REQ_STATUS_COLOR.completed,
    },
  ]
    .filter(r => r.count > 0)
    .map(r => ({
      label: `${r.label} · ${r.count}`,
      value: Math.round((r.count / total) * 100),
      color: r.color,
    }));
});

async function fetchProject(isRefresh = false) {
  if (isRefresh) refreshing.value = true;
  else initialLoading.value = true;
  try {
    const [projRes, actRes, heatRes] = await Promise.all([
      api.get<{ success: boolean; data: Project }>(`/projects/${projectId()}`),
      api.get<{ success: boolean; data: ProjectActivity[] }>(
        `/projects/${projectId()}/activities?limit=20`,
      ),
      api.get<{ success: boolean; data: HeatmapData }>(
        `/projects/${projectId()}/activity-heatmap?year=${new Date().getFullYear()}`,
      ),
    ]);
    if (projRes.data.success) {
      project.value = projRes.data.data;
    } else {
      toast.add({
        severity: "error",
        summary: "获取失败",
        detail: "项目不存在",
        life: 3000,
      });
    }
    if (actRes.data.success) activities.value = actRes.data.data;
    if (heatRes.data.success) heatmap.value = heatRes.data.data;
  } catch {
    toast.add({
      severity: "error",
      summary: "获取失败",
      detail: "加载项目统计失败",
      life: 3000,
    });
  } finally {
    initialLoading.value = false;
    refreshing.value = false;
  }
}

function go(path: string) {
  router.push(`/projects/${projectId()}/${path}`);
}

function openActivity(a: ProjectActivity) {
  const link = activityLink(projectId(), a.entityType, a.entityId);
  if (link) router.push(link);
}

function activityHref(a: ProjectActivity): string | null {
  return activityLink(projectId(), a.entityType, a.entityId);
}

onMounted(() => fetchProject(false));
</script>

<template>
  <div class="tab-page">
    <div v-if="initialLoading" class="loading-state">
      <span class="loading loading-spinner loading-lg text-primary" />
      <p class="text-base-content/60">加载中...</p>
    </div>

    <AppPage v-else-if="project" title="项目总览">
      <template #actions>
        <button
          type="button"
          class="btn btn-ghost"
          :disabled="refreshing"
          @click="fetchProject(true)"
        >
          <span v-if="refreshing" class="loading loading-spinner loading-xs" />
          <RefreshCw v-if="!refreshing" :size="14" aria-hidden="true" />
          刷新
        </button>
      </template>

      <div class="stat-strip">
        <button type="button" class="strip-item" @click="go('requirements')">
          <span class="strip-value">{{ req?.total ?? 0 }}</span>
          <span class="strip-label">需求总数</span>
        </button>
        <button type="button" class="strip-item" @click="go('requirements')">
          <span class="strip-value">{{ reqRunning }}</span>
          <span class="strip-label">运行中</span>
        </button>
        <button type="button" class="strip-item" @click="go('requirements')">
          <span class="strip-value">{{ req?.completed ?? 0 }}</span>
          <span class="strip-label">已完成</span>
        </button>
        <button type="button" class="strip-item" @click="go('defects')">
          <span class="strip-value">{{ defectUnresolved }}</span>
          <span class="strip-label">缺陷</span>
        </button>
      </div>

      <div class="overview-main">
        <section class="panel panel--req">
          <div class="panel-head">
            <h2 class="panel-title">需求进度</h2>
          </div>
          <template v-if="reqTotal > 0">
            <p class="meter-caption">完成率</p>
            <div class="progress-wrap">
              <progress
                class="progress progress-primary w-full"
                :value="reqCompletion"
                max="100"
              />
              <span
                v-if="rates?.requirementCompletionPct != null"
                class="progress-value"
              >
                {{ reqCompletion }}%
              </span>
            </div>
            <p class="meter-caption meter-caption--spaced">状态分布</p>
            <div class="meter-stack" role="img" :aria-label="`需求状态分布，共 ${reqTotal} 项`">
              <div
                v-for="seg in reqMeter"
                :key="seg.label"
                class="meter-segment"
                :style="{ width: `${seg.value}%`, backgroundColor: seg.color }"
                :title="seg.label"
              />
            </div>
            <ul class="meter-legend">
              <li v-for="seg in reqMeter" :key="seg.label">
                <span class="meter-dot" :style="{ backgroundColor: seg.color }" />
                {{ seg.label }}
              </li>
            </ul>
          </template>
          <p v-else class="empty-hint text-base-content/60">暂无需求，可在「需求」页创建</p>
        </section>

        <section class="panel panel--heatmap">
          <div class="panel-head">
            <h2 class="panel-title">活跃图</h2>
            <span class="heatmap-summary text-base-content/60">
              区间合计 {{ heatmapTotal }} 条
              <template v-if="heatmapPeak">
                · 峰值 {{ heatmapPeak.date }}（{{ heatmapPeak.count }} 条）
              </template>
            </span>
          </div>
          <ActivityHeatmap
            v-if="heatmap && heatmap.entries.length > 0"
            :entries="heatmap.entries"
            :max="heatmap.max"
          />
          <p v-else class="empty-hint text-base-content/60">暂无活动</p>
        </section>

        <section class="panel panel--activity">
          <div class="panel-head">
            <h2 class="panel-title">最近活动</h2>
          </div>
          <div v-if="activities.length === 0" class="empty-hint text-base-content/60">暂无活动</div>
          <ul v-else class="activity-list">
            <li
              v-for="a in activities"
              :key="a.id"
              class="activity-item"
              :class="{ 'activity-item--link': !!activityHref(a) }"
              @click="openActivity(a)"
            >
              <span
                class="activity-icon"
                :style="{ color: activityIcon(a.action).color }"
              >
                <component :is="activityIcon(a.action).icon" :size="16" />
              </span>
              <div class="activity-body">
                <span class="activity-desc" :title="a.description">{{ a.description }}</span>
                <span class="activity-meta text-base-content/60">
                  <UserAvatar v-if="a.user" :qq="a.user.qq" :size="16" />
                  <span class="activity-author" :title="a.user?.nickname || '系统'">
                    {{ a.user?.nickname || "系统" }}
                  </span>
                  <span class="activity-sep">·</span>
                  <span class="activity-time">{{ formatRelativeTime(a.createdAt) }}</span>
                </span>
              </div>
              <ChevronRight
                v-if="activityHref(a)"
                class="activity-chevron text-base-content/60"
                :size="16"
                aria-hidden="true"
              />
            </li>
          </ul>
        </section>
      </div>
    </AppPage>
  </div>
</template>

<style scoped>
.tab-page {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
  min-width: 0;
}

.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 3rem;
}


.stat-strip {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(9rem, 1fr));
  gap: 0.65rem;
}

.strip-item {
  display: grid;
  gap: 0.15rem;
  padding: 0.9rem 1rem;
  border-radius: 12px;
  background: var(--color-base-100);
  text-align: left;
  cursor: pointer;
  transition: background-color 0.15s ease;
}

.strip-item:hover {
  background: color-mix(in srgb, var(--color-primary) 8%, var(--color-base-100));
}

.strip-value {
  font-size: 1.55rem;
  font-weight: 700;
  line-height: 1.1;
}

.strip-label {
  font-size: 0.8rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.overview-main {
  display: grid;
  grid-template-columns: 1fr 1fr;
  grid-template-areas:
    "req heatmap"
    "activity activity";
  gap: 0.85rem;
  align-items: stretch;
}

.panel--req {
  grid-area: req;
}

.panel--heatmap {
  grid-area: heatmap;
}

.panel--activity {
  grid-area: activity;
}

.panel {
  border-radius: 12px;
  background: var(--color-base-100);
  padding: 1rem 1.1rem;
  min-width: 0;
}

.panel-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.35rem 0.75rem;
  margin-bottom: 0.75rem;
}

.panel-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 650;
}

.heatmap-summary {
  font-size: 0.8rem;
}

.meter-caption {
  margin: 0 0 0.45rem;
  font-size: 0.8rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.meter-caption--spaced {
  margin-top: 1rem;
}

.progress-wrap {
  display: flex;
  align-items: center;
  gap: 0.65rem;
}

.progress-value {
  flex-shrink: 0;
  font-size: 0.85rem;
  font-weight: 600;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.meter-stack {
  display: flex;
  height: 0.65rem;
  border-radius: 999px;
  overflow: hidden;
  background: var(--color-base-200);
}

.meter-segment {
  min-width: 2px;
  height: 100%;
  transition: width 0.2s ease;
}

.meter-legend {
  list-style: none;
  margin: 0.55rem 0 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem 0.85rem;
  font-size: 0.78rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.meter-legend li {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.meter-dot {
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 999px;
  flex-shrink: 0;
}

.empty-hint {
  margin: 0;
  font-size: 0.85rem;
}

.activity-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 0.3rem 0.85rem;
}

.activity-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 0.6rem;
  align-items: center;
  padding: 0.5rem 0.4rem;
  border-radius: 8px;
  min-width: 0;
}

.activity-item--link {
  cursor: pointer;
}

.activity-item--link:hover {
  background: var(--color-base-200);
}

.activity-icon {
  display: grid;
  place-items: center;
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 8px;
  background: var(--color-base-200);
  font-size: 0.95rem;
  line-height: 1;
}

.activity-body {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  min-width: 0;
}

.activity-desc {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 0.9rem;
  font-weight: 500;
  line-height: 1.35;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.activity-meta {
  flex: 0 1 auto;
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  white-space: nowrap;
  min-width: 0;
}

.activity-author {
  max-width: 7rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.activity-sep,
.activity-time {
  flex-shrink: 0;
}

.activity-chevron {
  align-self: center;
  font-size: 1.1rem;
  line-height: 1;
}

@media (max-width: 900px) {
  .stat-strip {
    grid-template-columns: 1fr 1fr;
  }

  .overview-main {
    grid-template-columns: 1fr;
    grid-template-areas:
      "req"
      "heatmap"
      "activity";
  }
}

@media (max-width: 560px) {
  .stat-strip {
    grid-template-columns: 1fr;
  }

  .activity-list {
    grid-template-columns: 1fr;
  }
}
</style>
