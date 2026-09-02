<script setup lang="ts">
import { Link2 } from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { api } from "@/utils/api";

type DependencyNode = {
  id: string;
  kind: "requirement" | "defect";
  description: string | null;
  status: string | null;
};

type DependencySummary = {
  blocking: DependencyNode[];
  blockedBy: DependencyNode[];
  transitiveBlocking: DependencyNode[];
  transitiveBlockedBy: DependencyNode[];
};

const props = defineProps<{
  projectId: string;
  nodeType: "requirement" | "defect";
  nodeId: string;
}>();

const router = useRouter();
const summary = ref<DependencySummary | null>(null);
const loading = ref(false);

const blocking = computed(() => summary.value?.blocking ?? []);
const blockedBy = computed(() => summary.value?.blockedBy ?? []);

async function fetchSummary() {
  loading.value = true;
  try {
    const { data } = await api.get<{
      success: boolean;
      data?: DependencySummary;
    }>(`/projects/${props.projectId}/dependencies/${props.nodeType}/${props.nodeId}`);
    if (data.success && data.data) summary.value = data.data;
    else summary.value = null;
  } catch {
    summary.value = null;
  } finally {
    loading.value = false;
  }
}

function kindLabel(kind: "requirement" | "defect"): string {
  return kind === "requirement" ? "需求" : "缺陷";
}

function goToNode(n: DependencyNode) {
  if (n.kind === "requirement") {
    void router.push(`/projects/${props.projectId}/requirements/${n.id}`);
  } else {
    void router.push(`/projects/${props.projectId}/defects`);
  }
}

watch(
  () => [props.projectId, props.nodeType, props.nodeId],
  () => void fetchSummary(),
);

onMounted(() => void fetchSummary());

defineExpose({ refresh: fetchSummary });
</script>

<template>
  <section class="panel">
    <div class="panel-head">
      <h2 class="panel-title">依赖关系</h2>
    </div>

    <div v-if="loading" class="dep-loading">
      <span class="loading loading-spinner loading-sm" />
    </div>

    <div v-else-if="blocking.length === 0 && blockedBy.length === 0" class="dep-empty">
      <Link2 :size="18" aria-hidden="true" class="text-base-content/40" />
      <span class="text-base-content/60">暂无依赖关系</span>
    </div>

    <div v-else class="dep-grid">
      <div class="dep-col">
        <h3 class="dep-col-title">被谁阻塞</h3>
        <ul v-if="blockedBy.length" class="dep-list">
          <li v-for="n in blockedBy" :key="`bb-${n.kind}-${n.id}`">
            <button type="button" class="dep-pill" :class="n.kind === 'defect' ? 'is-defect' : 'is-requirement'" @click="goToNode(n)">
              <span class="dep-pill-kind">{{ kindLabel(n.kind) }}</span>
              <span class="dep-pill-id">{{ n.id }}</span>
            </button>
          </li>
        </ul>
        <span v-else class="dep-none text-base-content/50">—</span>
      </div>

      <div class="dep-col">
        <h3 class="dep-col-title">阻塞谁</h3>
        <ul v-if="blocking.length" class="dep-list">
          <li v-for="n in blocking" :key="`b-${n.kind}-${n.id}`">
            <button type="button" class="dep-pill" :class="n.kind === 'defect' ? 'is-defect' : 'is-requirement'" @click="goToNode(n)">
              <span class="dep-pill-kind">{{ kindLabel(n.kind) }}</span>
              <span class="dep-pill-id">{{ n.id }}</span>
            </button>
          </li>
        </ul>
        <span v-else class="dep-none text-base-content/50">—</span>
      </div>
    </div>
  </section>
</template>

<style scoped>
.panel {
  border-radius: 12px;
  background: var(--color-base-100);
  padding: 1rem 1.1rem;
  min-width: 0;
}

.panel-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.35rem 0.75rem;
  margin-bottom: 0.85rem;
}

.panel-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 650;
}

.dep-loading {
  display: flex;
  justify-content: center;
  padding: 1rem;
}

.dep-empty {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0;
  font-size: 0.85rem;
}

.dep-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 1rem;
}

@media (min-width: 768px) {
  .dep-grid {
    grid-template-columns: 1fr 1fr;
  }
}

.dep-col-title {
  margin: 0 0 0.5rem;
  font-size: 0.8rem;
  font-weight: 600;
  color: color-mix(in oklab, var(--color-base-content) 70%, transparent);
}

.dep-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.dep-pill {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.3rem 0.6rem;
  border-radius: 999px;
  border: 1px solid transparent;
  background: color-mix(in oklab, var(--color-base-content) 6%, transparent);
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease;
  font-size: 0.78rem;
  line-height: 1.4;
}

.dep-pill:hover {
  background: color-mix(in oklab, var(--color-base-content) 12%, transparent);
  border-color: color-mix(in oklab, var(--color-base-content) 15%, transparent);
}

.dep-pill.is-requirement {
  background: color-mix(in oklab, var(--color-info, #3b82f6) 12%, transparent);
}

.dep-pill.is-requirement:hover {
  background: color-mix(in oklab, var(--color-info, #3b82f6) 20%, transparent);
  border-color: color-mix(in oklab, var(--color-info, #3b82f6) 35%, transparent);
}

.dep-pill.is-defect {
  background: color-mix(in oklab, var(--color-warning, #f59e0b) 14%, transparent);
}

.dep-pill.is-defect:hover {
  background: color-mix(in oklab, var(--color-warning, #f59e0b) 22%, transparent);
  border-color: color-mix(in oklab, var(--color-warning, #f59e0b) 35%, transparent);
}

.dep-pill-kind {
  font-weight: 600;
  font-size: 0.72rem;
  padding: 0.05rem 0.35rem;
  border-radius: 4px;
  background: color-mix(in oklab, var(--color-base-content) 12%, transparent);
  color: var(--color-base-content);
  flex-shrink: 0;
}

.dep-pill.is-requirement .dep-pill-kind {
  background: color-mix(in oklab, var(--color-info, #3b82f6) 30%, transparent);
  color: var(--color-info-content, #fff);
}

.dep-pill.is-defect .dep-pill-kind {
  background: color-mix(in oklab, var(--color-warning, #f59e0b) 35%, transparent);
  color: var(--color-warning-content, #000);
}

.dep-pill-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.72rem;
  color: color-mix(in oklab, var(--color-base-content) 75%, transparent);
}

.dep-none {
  font-size: 0.82rem;
}
</style>
