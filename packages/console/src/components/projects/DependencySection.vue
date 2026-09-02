<script setup lang="ts">
import { Link2, Plus, Trash2, X } from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
import { AppModal, AppSelect, useToast } from "@/ui";
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

const toast = useToast();

const summary = ref<DependencySummary | null>(null);
const loading = ref(false);

// 添加依赖弹窗
const showAdd = ref(false);
const addSaving = ref(false);
const addForm = ref({
  direction: "blocking" as "blocking" | "blockedBy",
  targetType: "requirement" as "requirement" | "defect",
  targetId: "",
});

// 目标节点搜索
const searchResults = ref<{ id: string; description: string }[]>([]);
const searchLoading = ref(false);

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

async function searchTargets(query: string) {
  if (!query.trim()) {
    searchResults.value = [];
    return;
  }
  searchLoading.value = true;
  try {
    const q = query.trim();
    const url =
      addForm.value.targetType === "requirement"
        ? `/projects/${props.projectId}/requirements?id=${encodeURIComponent(q)}`
        : `/projects/${props.projectId}/defects?q=${encodeURIComponent(q)}`;
    const { data } = await api.get<{ success: boolean; data: any[] }>(url);
    if (data.success) {
      searchResults.value = (data.data ?? []).map(r => ({
        id: r.id,
        description: r.description ?? "",
      }));
    }
  } catch {
    searchResults.value = [];
  } finally {
    searchLoading.value = false;
  }
}

function openAdd() {
  addForm.value = {
    direction: "blocking",
    targetType: "requirement",
    targetId: "",
  };
  searchResults.value = [];
  showAdd.value = true;
}

async function confirmAdd() {
  const targetId = addForm.value.targetId.trim();
  if (!targetId) {
    toast.warn("请选择", "请选择目标节点");
    return;
  }
  addSaving.value = true;
  try {
    const body =
      addForm.value.direction === "blocking"
        ? {
            sourceType: props.nodeType,
            sourceId: props.nodeId,
            targetType: addForm.value.targetType,
            targetId,
          }
        : {
            sourceType: addForm.value.targetType,
            sourceId: targetId,
            targetType: props.nodeType,
            targetId: props.nodeId,
          };
    const { data } = await api.post<{ success: boolean; error?: string }>(
      `/projects/${props.projectId}/dependencies`,
      body,
    );
    if (!data.success) {
      toast.error("添加失败", data.error ?? "未知错误");
      return;
    }
    toast.success("已添加依赖");
    showAdd.value = false;
    await fetchSummary();
  } catch (err: unknown) {
    const msg =
      (err as { response?: { data?: { error?: string } } })?.response?.data
        ?.error ?? "请求异常";
    toast.error("添加失败", msg);
  } finally {
    addSaving.value = false;
  }
}

async function removeDependency(target: DependencyNode) {
  // target 在 blocking 列表里，说明当前节点阻塞 target（source=当前）
  // target 在 blockedBy 列表里，说明 target 阻塞当前节点（source=target）
  const isBlocking = blocking.value.some(b => b.id === target.id && b.kind === target.kind);
  const sourceType = isBlocking ? props.nodeType : target.kind;
  const sourceId = isBlocking ? props.nodeId : target.id;
  const targetType = isBlocking ? target.kind : props.nodeType;
  const targetId = isBlocking ? target.id : props.nodeId;

  try {
    const { data } = await api.delete<{ success: boolean; error?: string }>(
      `/projects/${props.projectId}/dependencies/${sourceType}/${sourceId}/${targetType}/${targetId}`,
    );
    if (!data.success) {
      toast.error("移除失败", data.error ?? "未知错误");
      return;
    }
    toast.success("已移除依赖");
    await fetchSummary();
  } catch (err: unknown) {
    const msg =
      (err as { response?: { data?: { error?: string } } })?.response?.data
        ?.error ?? "请求异常";
    toast.error("移除失败", msg);
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
      <button type="button" class="btn btn-ghost btn-sm" @click="openAdd">
        <Plus :size="14" aria-hidden="true" />
        添加依赖
      </button>
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
            <div class="dep-item-main">
              <span class="badge badge-sm" :class="n.kind === 'defect' ? 'badge-warning' : 'badge-info'">
                {{ kindLabel(n.kind) }}
              </span>
              <span class="dep-item-id">{{ n.id }}</span>
              <span class="dep-item-desc">{{ nodeLabel(n) }}</span>
            </div>
            <button
              type="button"
              class="btn btn-ghost btn-xs"
              title="移除依赖"
              @click="removeDependency(n)"
            >
              <X :size="14" aria-hidden="true" />
            </button>
          </li>
        </ul>
        <span v-else class="dep-none text-base-content/50">—</span>
      </div>

      <div class="dep-col">
        <h3 class="dep-col-title">被谁阻塞（Blocked By）</h3>
        <ul v-if="blockedBy.length" class="dep-list">
          <li v-for="n in blockedBy" :key="`bb-${n.kind}-${n.id}`" class="dep-item">
            <div class="dep-item-main">
              <span class="badge badge-sm" :class="n.kind === 'defect' ? 'badge-warning' : 'badge-info'">
                {{ kindLabel(n.kind) }}
              </span>
              <span class="dep-item-id">{{ n.id }}</span>
              <span class="dep-item-desc">{{ nodeLabel(n) }}</span>
            </div>
            <button
              type="button"
              class="btn btn-ghost btn-xs"
              title="移除依赖"
              @click="removeDependency(n)"
            >
              <X :size="14" aria-hidden="true" />
            </button>
          </li>
        </ul>
        <span v-else class="dep-none text-base-content/50">—</span>
      </div>
    </div>

    <AppModal v-model="showAdd" title="添加依赖关系">
      <div class="form-grid">
        <div class="field">
          <label class="label">关系方向</label>
          <AppSelect
            v-model="addForm.direction"
            :options="[
              { label: '阻塞了谁（Blocking）', value: 'blocking' },
              { label: '被谁阻塞（Blocked By）', value: 'blockedBy' },
            ]"
          />
        </div>
        <div class="field">
          <label class="label">目标类型</label>
          <AppSelect
            v-model="addForm.targetType"
            :options="[
              { label: '需求', value: 'requirement' },
              { label: '缺陷', value: 'defect' },
            ]"
            @update:model-value="searchResults = []"
          />
        </div>
        <div class="field">
          <label class="label">目标节点（按 ID / 描述搜索）</label>
          <input
            v-model="addForm.targetId"
            type="text"
            class="input w-full"
            placeholder="输入 ID 或描述"
            @input="searchTargets(addForm.targetId)"
          />
          <div v-if="searchLoading" class="dep-loading-inline">
            <span class="loading loading-spinner loading-xs" />
          </div>
          <ul v-else-if="searchResults.length" class="search-results">
            <li
              v-for="r in searchResults"
              :key="r.id"
              class="search-result-item"
              @click="addForm.targetId = r.id"
            >
              <span class="search-result-id">{{ r.id }}</span>
              <span class="search-result-desc">{{ r.description }}</span>
            </li>
          </ul>
        </div>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="showAdd = false">取消</button>
        <button
          type="button"
          class="btn btn-primary"
          :disabled="addSaving || !addForm.targetId.trim()"
          @click="confirmAdd"
        >
          <span v-if="addSaving" class="loading loading-spinner loading-sm" />
          添加
        </button>
      </template>
    </AppModal>
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
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.45rem 0.55rem;
  border-radius: 8px;
  background: color-mix(in oklab, var(--color-base-content) 5%, transparent);
}

.dep-item-main {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  min-width: 0;
}

.dep-item-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.75rem;
  flex-shrink: 0;
}

.dep-item-desc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.82rem;
  color: color-mix(in oklab, var(--color-base-content) 70%, transparent);
}

.dep-none {
  font-size: 0.82rem;
}

.form-grid {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.label {
  font-size: 0.8rem;
  font-weight: 600;
  color: color-mix(in oklab, var(--color-base-content) 70%, transparent);
}

.dep-loading-inline {
  padding: 0.35rem 0;
}

.search-results {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 200px;
  overflow-y: auto;
  border: 1px solid color-mix(in oklab, var(--color-base-content) 15%, transparent);
  border-radius: 8px;
}

.search-result-item {
  display: flex;
  gap: 0.5rem;
  padding: 0.45rem 0.55rem;
  cursor: pointer;
  font-size: 0.82rem;
  align-items: center;
}

.search-result-item:hover {
  background: color-mix(in oklab, var(--color-base-content) 8%, transparent);
}

.search-result-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.75rem;
  flex-shrink: 0;
}

.search-result-desc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: color-mix(in oklab, var(--color-base-content) 70%, transparent);
}
</style>
