<script setup lang="ts">
import {
  CircleCheck,
  CircleQuestionMark,
  Flag,
  Info,
  Keyboard,
  Lightbulb,
  RefreshCw,
  type LucideIcon,
} from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { api } from "@/utils/api";

type Run = {
  id: string;
  requirementId: string;
  index: number;
  status: string;
  endReason: string | null;
  startedAt: string;
  endedAt: string | null;
};

type Step = {
  id: string;
  runId: string;
  seq: number;
  kind: string;
  summary: string;
  detail: string | null;
  createdAt: string;
};

type Scenario = {
  id: string;
  key: string;
  title: string;
  description: string | null;
  status: string;
  cases?: CaseRow[];
};

type CaseRow = {
  id: string;
  title: string;
  kind: string;
  status: string;
};

type ContextRow = {
  id: string;
  requirementId: string;
  snapshot: {
    openDecisions?: Array<{
      raisedAt?: string;
      question?: string;
      context?: string;
    }>;
    codeLandmarks?: Array<{ path?: string; symbol?: string; note?: string }>;
    lastRunSummary?: unknown;
  };
  updatedAt: string;
};

const props = defineProps<{ requirementId: string }>();

const route = useRoute();
const projectId = () => (route.params as Record<string, string>).id;

const loading = ref(false);
const runs = ref<Run[]>([]);
const scenarios = ref<Scenario[]>([]);
const context = ref<ContextRow | null>(null);
const expandedRun = ref<string | null>(null);
const stepsByRun = ref<Record<string, Step[]>>({});
const stepsLoading = ref<Record<string, boolean>>({});

const runStatusLabel: Record<string, string> = {
  running: "运行中",
  finished: "已结束",
  completed: "已完成",
  abandoned: "已放弃",
};

const scenarioStatusLabel: Record<string, string> = {
  pending: "待验收",
  passing: "通过",
  failing: "失败",
  blocked: "受阻",
  waived: "已豁免",
};

const caseStatusLabel: Record<string, string> = {
  pending: "待执行",
  passed: "通过",
  failed: "失败",
  blocked: "受阻",
  skipped: "跳过",
};

const stepKindLabel: Record<string, string> = {
  think: "思考",
  code: "编码",
  test: "测试",
  verify: "验收",
  ask_user: "询问",
  info: "留痕",
  reflect: "反思",
};

const stepKindIcon: Record<string, LucideIcon> = {
  think: Lightbulb,
  code: Keyboard,
  test: CircleCheck,
  verify: Flag,
  ask_user: CircleQuestionMark,
  info: Info,
  reflect: RefreshCw,
};

const sortedRuns = computed(() =>
  [...runs.value].sort((a, b) => b.index - a.index),
);

const latestRun = computed(() => sortedRuns.value[0] ?? null);

const scenarioPassingCount = computed(
  () =>
    scenarios.value.filter(s => s.status === "passing" || s.status === "waived")
      .length,
);

const openDecisionCount = computed(
  () => context.value?.snapshot.openDecisions?.length ?? 0,
);

const latestRunLabel = computed(() => {
  const run = latestRun.value;
  if (!run) return "—";
  return runStatusLabel[run.status] ?? run.status;
});

function runBadgeClass(status: string) {
  if (status === "running") return "badge-info";
  if (status === "completed") return "badge-success";
  if (status === "finished") return "badge-warning";
  return "badge-ghost";
}

function scenarioBadgeClass(status: string) {
  if (status === "passing") return "badge-success";
  if (status === "failing") return "badge-error";
  if (status === "waived") return "badge-warning";
  if (status === "blocked") return "badge-error";
  return "badge-ghost";
}

function caseBadgeClass(status: string) {
  if (status === "passed") return "badge-success";
  if (status === "failed") return "badge-error";
  if (status === "blocked") return "badge-error";
  return "badge-ghost";
}

function caseProgress(s: Scenario) {
  const cases = s.cases ?? [];
  if (!cases.length) return null;
  const done = cases.filter(
    c => c.status === "passed" || c.status === "skipped",
  ).length;
  return {
    done,
    total: cases.length,
    pct: Math.round((done / cases.length) * 100),
  };
}

// —— 验收场景折叠 ——
// 场景卡片默认收起（只显示 key/状态/标题/进度），点击头部展开看描述与用例；
// 单个场景用例超过 CASE_PREVIEW 条时默认截断，可展开全部。
const CASE_PREVIEW = 8;
const expandedScenarios = ref<Set<string>>(new Set());
const expandedCases = ref<Set<string>>(new Set());

function toggleScenario(id: string) {
  const set = expandedScenarios.value;
  if (set.has(id)) set.delete(id);
  else set.add(id);
}

function toggleCases(id: string) {
  const set = expandedCases.value;
  if (set.has(id)) set.delete(id);
  else set.add(id);
}

function isScenarioOpen(s: Scenario): boolean {
  return expandedScenarios.value.has(s.id);
}

function visibleCases(s: Scenario): CaseRow[] {
  const cases = s.cases ?? [];
  if (expandedCases.value.has(s.id)) return cases;
  return cases.length > CASE_PREVIEW ? cases.slice(0, CASE_PREVIEW) : cases;
}

function formatDateTime(value: string) {
  return new Date(value).toLocaleString();
}

async function loadSteps(runId: string) {
  if (stepsByRun.value[runId]) return;
  stepsLoading.value[runId] = true;
  try {
    const { data } = await api.get<{ success: boolean; data: Step[] }>(
      `/projects/${projectId()}/requirements/${props.requirementId}/runs/${runId}/steps`,
    );
    if (data.success) stepsByRun.value[runId] = data.data;
  } finally {
    stepsLoading.value[runId] = false;
  }
}

async function fetchAll() {
  loading.value = true;
  stepsByRun.value = {};
  expandedRun.value = null;
  try {
    const [runsRes, scenariosRes, ctxRes] = await Promise.all([
      api.get<{ success: boolean; data: Run[] }>(
        `/projects/${projectId()}/requirements/${props.requirementId}/runs`,
      ),
      api.get<{ success: boolean; data: Scenario[] }>(
        `/projects/${projectId()}/requirements/${props.requirementId}/scenarios?includeCases=true`,
      ),
      api
        .get<{ success: boolean; data: ContextRow }>(
          `/projects/${projectId()}/requirements/${props.requirementId}/context`,
        )
        .catch(() => ({
          data: { success: false, data: null as unknown as ContextRow },
        })),
    ]);
    if (runsRes.data.success) runs.value = runsRes.data.data;
    if (scenariosRes.data.success) scenarios.value = scenariosRes.data.data;
    if (ctxRes.data.success) context.value = ctxRes.data.data;

    const first = sortedRuns.value[0] ?? runs.value[runs.value.length - 1];
    if (first) {
      expandedRun.value = first.id;
      await loadSteps(first.id);
    }
  } finally {
    loading.value = false;
  }
}

async function toggleRun(run: Run) {
  if (expandedRun.value === run.id) {
    expandedRun.value = null;
    return;
  }
  expandedRun.value = run.id;
  await loadSteps(run.id);
}

watch(
  () => props.requirementId,
  () => {
    void fetchAll();
  },
);

onMounted(fetchAll);
</script>

<template>
  <div v-if="loading" class="harness-loading">
    <span class="loading loading-spinner loading-md text-primary" />
  </div>
  <div v-else class="harness">
    <div class="stat-strip" data-testid="harness-summary">
      <div class="strip-item">
        <span class="strip-value">{{ runs.length }}</span>
        <span class="strip-label">Run 次数</span>
      </div>
      <div class="strip-item">
        <span class="strip-value strip-value--text">{{ latestRunLabel }}</span>
        <span class="strip-label">最新 Run</span>
      </div>
      <div class="strip-item">
        <span class="strip-value">
          {{ scenarioPassingCount }}
          <span class="strip-den">/ {{ scenarios.length }}</span>
        </span>
        <span class="strip-label">场景通过</span>
      </div>
      <div class="strip-item">
        <span
          class="strip-value"
          :class="{ 'strip-value--warn': openDecisionCount > 0 }"
        >
          {{ openDecisionCount }}
        </span>
        <span class="strip-label">未决决策</span>
      </div>
    </div>

    <section class="panel panel--context">
      <div class="panel-head">
        <h2 class="panel-title">工作记忆</h2>
        <span v-if="context" class="panel-meta text-base-content/60">
          {{ formatDateTime(context.updatedAt) }}
        </span>
      </div>

      <p v-if="!context" class="empty-hint text-base-content/60">暂无 Context。</p>
      <template v-else>
        <div class="context-blocks">
          <div class="context-block">
            <h3 class="context-label">未决决策</h3>
            <div
              v-if="context.snapshot.openDecisions?.length"
              class="open-decisions"
            >
              <div
                v-for="(d, i) in context.snapshot.openDecisions"
                :key="i"
                class="decision-item"
              >
                <CircleQuestionMark :size="16" aria-hidden="true" />
                <span>{{ d.question }}</span>
              </div>
            </div>
            <p v-else class="empty-hint text-base-content/60">无未决 open decision。</p>
          </div>

          <div
            v-if="context.snapshot.codeLandmarks?.length"
            class="context-block"
          >
            <h3 class="context-label">代码标记</h3>
            <div class="landmarks">
              <span
                v-for="(l, i) in context.snapshot.codeLandmarks"
                :key="i"
                class="landmark-item"
                :title="l.note"
              >
                {{ l.path }}<template v-if="l.symbol">:{{ l.symbol }}</template>
              </span>
            </div>
          </div>
        </div>
      </template>
    </section>

    <div class="harness-main">
      <section class="panel panel--runs">
        <div class="panel-head">
          <h2 class="panel-title">交付轮次</h2>
          <span v-if="sortedRuns.length" class="panel-meta text-base-content/60">
            共 {{ sortedRuns.length }} 次 · 点击展开步骤
          </span>
        </div>

        <p v-if="!sortedRuns.length" class="empty-hint text-base-content/60">
          尚未发起过自主交付——用 /chunsun &lt;需求ID&gt; 启动。
        </p>

        <ol v-else class="run-timeline">
          <li
            v-for="run in sortedRuns"
            :key="run.id"
            class="run-node"
            :class="{
              'run-node--active': expandedRun === run.id,
              [`run-node--${run.status}`]: true,
            }"
          >
            <button
              type="button"
              class="run-trigger"
              :aria-expanded="expandedRun === run.id"
              @click="toggleRun(run)"
            >
              <span class="run-rail" aria-hidden="true">
                <span class="run-dot" />
              </span>
              <span class="run-body">
                <span class="run-title-row">
                  <span class="run-index">#{{ run.index }}</span>
                  <span class="badge" :class="runBadgeClass(run.status)">
                    {{ runStatusLabel[run.status] ?? run.status }}
                  </span>
                  <span class="run-time text-base-content/60">{{
                    formatDateTime(run.startedAt)
                  }}</span>
                  <span class="run-chevron text-base-content/60" aria-hidden="true">
                    {{ expandedRun === run.id ? "▲" : "▼" }}
                  </span>
                </span>
                <span
                  v-if="run.endReason && (run.status === 'finished' || run.status === 'abandoned')"
                  class="run-reason text-base-content/60"
                >
                  结束原因：{{ run.endReason }}
                </span>
              </span>
            </button>

            <div v-if="expandedRun === run.id" class="step-rail">
              <div v-if="stepsLoading[run.id]" class="harness-loading harness-loading--inline">
                <span class="loading loading-spinner loading-sm text-primary" />
              </div>
              <p
                v-else-if="!stepsByRun[run.id]?.length"
                class="empty-hint text-base-content/60"
              >
                该 Run 暂无 Step。
              </p>
              <ol v-else class="step-list">
                <li
                  v-for="s in stepsByRun[run.id]"
                  :key="s.id"
                  class="step-item"
                >
                  <span
                    class="step-icon"
                    :title="stepKindLabel[s.kind] ?? s.kind"
                  >
                    <component
                      :is="stepKindIcon[s.kind] ?? CircleQuestionMark"
                      :size="14"
                    />
                  </span>
                  <div class="step-content">
                    <div class="step-head">
                      <span class="step-seq">#{{ s.seq }}</span>
                      <span class="badge badge-ghost">
                        {{ stepKindLabel[s.kind] ?? s.kind }}
                      </span>
                      <span class="step-time text-base-content/60">{{
                        formatDateTime(s.createdAt)
                      }}</span>
                    </div>
                    <p class="step-summary">{{ s.summary }}</p>
                  </div>
                </li>
              </ol>
            </div>
          </li>
        </ol>
      </section>

      <section class="panel panel--scenarios">
        <div class="panel-head">
          <h2 class="panel-title">验收场景</h2>
          <span v-if="scenarios.length" class="panel-meta text-base-content/60">
            {{ scenarios.length }} 个
          </span>
        </div>

        <p v-if="!scenarios.length" class="empty-hint text-base-content/60">
          循环中由 Agent 涌现，尚未 upsert。
        </p>

        <div v-else class="scenario-list">
          <article
            v-for="s in scenarios"
            :key="s.id"
            class="scenario-card"
            :class="{ 'scenario-card--open': isScenarioOpen(s) }"
          >
            <button
              type="button"
              class="scenario-toggle"
              :aria-expanded="isScenarioOpen(s)"
              @click="toggleScenario(s.id)"
            >
              <span class="scenario-head">
                <span class="scenario-key">{{ s.key }}</span>
                <span class="badge" :class="scenarioBadgeClass(s.status)">
                  {{ scenarioStatusLabel[s.status] ?? s.status }}
                </span>
                <span class="scenario-chevron text-base-content/60" aria-hidden="true">
                  {{ isScenarioOpen(s) ? "▲" : "▼" }}
                </span>
              </span>
              <span class="scenario-title">{{ s.title }}</span>
            </button>

            <div
              v-if="s.cases?.length && caseProgress(s)"
              class="case-progress"
            >
              <div class="case-progress-meta text-base-content/60">
                用例 {{ caseProgress(s)!.done }}/{{ caseProgress(s)!.total }}
              </div>
              <progress
                class="progress progress-primary w-full"
                :value="caseProgress(s)!.pct"
                max="100"
              />
            </div>

            <template v-if="isScenarioOpen(s)">
              <p v-if="s.description" class="scenario-desc text-base-content/60">
                {{ s.description }}
              </p>
              <template v-if="s.cases?.length">
                <ul class="case-list">
                  <li v-for="c in visibleCases(s)" :key="c.id" class="case-item">
                    <span class="case-title">{{ c.title }}</span>
                    <span class="badge" :class="caseBadgeClass(c.status)">
                      {{ caseStatusLabel[c.status] ?? c.status }}
                    </span>
                  </li>
                </ul>
                <button
                  v-if="(s.cases ?? []).length > CASE_PREVIEW"
                  type="button"
                  class="case-more"
                  @click="toggleCases(s.id)"
                >
                  {{
                    expandedCases.has(s.id)
                      ? "收起用例"
                      : `展开全部 ${s.cases!.length} 条用例`
                  }}
                </button>
              </template>
            </template>
          </article>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.harness {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
  min-width: 0;
}

.harness-loading {
  display: flex;
  justify-content: center;
  padding: 2rem;
}

.harness-loading--inline {
  padding: 0.85rem;
}

.stat-strip {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(8.5rem, 1fr));
  gap: 0.65rem;
}

.strip-item {
  display: grid;
  gap: 0.15rem;
  padding: 0.85rem 1rem;
  border-radius: 12px;
  background: var(--color-base-100);
  text-align: left;
}

.strip-value {
  font-size: 1.45rem;
  font-weight: 700;
  line-height: 1.15;
}

.strip-value--text {
  font-size: 1.15rem;
}

.strip-value--warn {
  color: var(--color-warning);
}

.strip-den {
  font-size: 0.85rem;
  font-weight: 500;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.strip-label {
  font-size: 0.8rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.harness-main {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.85rem;
  align-items: start;
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
  margin-bottom: 0.85rem;
}

.panel-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 650;
}

.panel-meta {
  font-size: 0.8rem;
}

.empty-hint {
  margin: 0;
  font-size: 0.85rem;
}

.run-timeline {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.run-trigger {
  display: grid;
  grid-template-columns: 1.25rem 1fr;
  gap: 0.65rem;
  width: 100%;
  margin: 0;
  padding: 0.35rem 0.25rem;
  border: none;
  border-radius: 10px;
  background: transparent;
  text-align: left;
  cursor: pointer;
  color: inherit;
}

.run-trigger:hover {
  background: var(--color-base-200);
}

.run-node--active > .run-trigger {
  background: color-mix(in srgb, var(--color-primary) 6%, var(--color-base-100));
}

.run-rail {
  display: flex;
  justify-content: center;
  padding-top: 0.45rem;
}

.run-dot {
  width: 0.65rem;
  height: 0.65rem;
  border-radius: 999px;
  background: var(--color-base-300);
  box-shadow: 0 0 0 3px var(--color-base-200);
}

.run-node--running .run-dot {
  background: var(--color-primary);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-primary) 18%, transparent);
}

.run-node--completed .run-dot {
  background: var(--color-success);
}

.run-node--finished .run-dot {
  background: var(--color-warning);
}

.run-node--abandoned .run-dot {
  background: color-mix(in oklab, var(--color-base-content) 55%, transparent);
}

.run-body {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-width: 0;
}

.run-title-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.45rem;
}

.run-index {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-weight: 700;
}

.run-time {
  margin-left: auto;
  font-size: 0.78rem;
}

.run-chevron {
  font-size: 0.65rem;
  line-height: 1;
}

.run-reason {
  font-size: 0.8rem;
}

.step-rail {
  margin: 0.15rem 0 0.5rem 1.9rem;
  padding: 0.55rem 0.65rem 0.35rem;
  border-left: 1px dashed var(--color-base-300);
}

.step-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

.step-item {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.65rem;
  align-items: start;
}

.step-icon {
  display: grid;
  place-items: center;
  width: 1.75rem;
  height: 1.75rem;
  border-radius: 8px;
  background: var(--color-base-200);
  font-size: 0.95rem;
  line-height: 1;
}

.step-content {
  min-width: 0;
}

.step-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.2rem;
}

.step-seq {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.step-time {
  margin-left: auto;
  font-size: 0.75rem;
}

.step-summary {
  margin: 0;
  font-size: 0.88rem;
  line-height: 1.4;
}

.scenario-list {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.scenario-card {
  border-radius: 10px;
  padding: 0.75rem 0.85rem;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  background: color-mix(in srgb, var(--color-base-200) 55%, var(--color-base-100));
}

.scenario-card--open {
  background: color-mix(in srgb, var(--color-primary) 10%, var(--color-base-100));
}

.scenario-toggle {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  width: 100%;
  margin: 0;
  padding: 0;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
  color: inherit;
  font: inherit;
}

.scenario-toggle:hover .scenario-key {
  color: var(--color-primary);
}

.scenario-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.scenario-key {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.8rem;
  font-weight: 650;
}

.scenario-chevron {
  margin-left: auto;
  font-size: 0.65rem;
  line-height: 1;
}

.scenario-title {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 600;
  line-height: 1.35;
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scenario-card--open .scenario-title {
  white-space: normal;
  overflow: visible;
  text-overflow: clip;
}

.scenario-desc {
  margin: 0;
  font-size: 0.8rem;
  line-height: 1.4;
}

.case-progress {
  display: grid;
  gap: 0.3rem;
  margin-top: 0.25rem;
}

.case-progress-meta {
  font-size: 0.75rem;
}

.case-list {
  list-style: none;
  margin: 0.15rem 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.case-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  font-size: 0.8rem;
}

.case-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.case-more {
  align-self: flex-start;
  margin: 0.35rem 0 0;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--color-primary);
  font-size: 0.78rem;
  cursor: pointer;
}

.case-more:hover {
  text-decoration: underline;
}

.context-blocks {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

@media (min-width: 720px) {
  .context-blocks {
    flex-direction: row;
    gap: 1.5rem;
  }

  .context-blocks .context-block {
    flex: 1;
    min-width: 0;
  }
}

.context-block + .context-block {
  margin-top: 0;
}

.context-label {
  margin: 0 0 0.4rem;
  font-size: 0.78rem;
  font-weight: 600;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  text-transform: none;
  letter-spacing: 0;
}

.open-decisions {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.decision-item {
  display: flex;
  align-items: flex-start;
  gap: 0.4rem;
  font-size: 0.85rem;
  color: var(--color-warning);
}

.decision-icon {
  margin-top: 0.05rem;
  font-size: 0.85rem;
  line-height: 1;
}

.landmarks {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.landmark-item {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.75rem;
  background: var(--color-base-200);
  border-radius: 6px;
  padding: 0.15rem 0.4rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}


@media (max-width: 960px) {
  .harness-main {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 560px) {
  .stat-strip {
    grid-template-columns: 1fr 1fr;
  }

  .step-rail {
    margin-left: 0.85rem;
  }
}
</style>
