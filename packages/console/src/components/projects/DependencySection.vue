<script setup lang="ts">
import { Link2 } from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
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

function nodeLabel(n: DependencyNode): string {
  return n.description || n.id;
}

function kindLabel(kind: "requirement" | "defect"): string {
  return kind === "requirement" ? "需求" : "缺陷";
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
        <h3 class="dep-col-title">阻塞了谁（Blocking）</h3>
        <ul v-if="blocking.length" class="dep-list">
          <li v-for="n in blocking" :key="`b-${n.kind}-${n.id}`" class="dep-item">
            <span class="badge badge-sm" :class="n.kind === 'defect' ? 'badge-warning' : 'badge-info'">
              {{ kindLabel(n.kind) }}
            </span>
            <span class="dep-item-id">{{ n.id }}</span>
            <span class="dep-item-desc">{{ nodeLabel(n) }}</span>
          </li>
        </ul>
        <span v-else class="dep-none text-base-content/50">—</span>
      </div>

      <div class="dep-col">
        <h3 class="dep-col-title">被谁阻塞（Blocked By）</h3>
        <ul v-if="blockedBy.length" class="dep-list">
          <li v-for="n in blockedBy" :key="`bb-${n.kind}-${n.id}`" class="dep-item">
            <span class="badge badge-sm" :class="n.kind === 'defect' ? 'badge-warning' : 'badge-info'">
              {{ kindLabel(n.kind) }}
            </span>
            <span class="dep-item-id">{{ n.id }}</span>
            <span class="dep-item-desc">{{ nodeLabel(n) }}</span>
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
  flex-direction: column;
  gap: 0.4rem;
}

.dep-item {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  padding: 0.45rem 0.55rem;
  border-radius: 8px;
  background: color-mix(in oklab, var(--color-base-content) 5%, transparent);
  min-width: 0;
}

.dep-item .badge {
  flex-shrink: 0;
}

.dep-item-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.75rem;
  flex-shrink: 0;
}

.dep-item-desc {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.82rem;
  color: color-mix(in oklab, var(--color-base-content) 70%, transparent);
}

.dep-none {
  font-size: 0.82rem;
}
</style>
