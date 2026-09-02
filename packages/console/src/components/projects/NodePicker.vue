<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { AppModal } from "@/ui";
import { api } from "@/utils/api";

export type PickedNode = {
  kind: "requirement" | "defect";
  id: string;
  label: string;
};

type NodeRow = {
  id: string;
  description?: string | null;
  status?: string;
  severity?: string;
};

const props = withDefaults(
  defineProps<{
    projectId: string;
    placeholder?: string;
    /** 是否允许多选（默认多选）。单选时每次打开只保留最新选中项。 */
    multiple?: boolean;
  }>(),
  { placeholder: "选择上级依赖（可选）", multiple: true },
);

const model = defineModel<PickedNode[]>({ default: () => [] });

const open = ref(false);
const keyword = ref("");
const kindFilter = ref<"all" | "requirement" | "defect">("all");
const loading = ref(false);
const candidates = ref<NodeRow[]>([]);
const reqCache = ref<NodeRow[]>([]);
const defCache = ref<NodeRow[]>([]);

const kindTabs = [
  { value: "all", label: "全部" },
  { value: "requirement", label: "需求" },
  { value: "defect", label: "缺陷" },
] as const;

const selectedIds = computed(() => new Set(model.value.map(n => `${n.kind}:${n.id}`)));

function labelFor(kind: PickedNode["kind"], row: NodeRow): string {
  const desc = row.description?.trim();
  const short = desc ? (desc.length > 40 ? `${desc.slice(0, 40)}…` : desc) : row.id;
  return short || row.id;
}

function kindLabel(kind: PickedNode["kind"]): string {
  return kind === "requirement" ? "需求" : "缺陷";
}

async function fetchCandidates() {
  loading.value = true;
  try {
    const q = keyword.value.trim();
    const jobs: Promise<void>[] = [];

    if (kindFilter.value === "all" || kindFilter.value === "requirement") {
      jobs.push(
        (async () => {
          const query = q ? `?id=${encodeURIComponent(q)}` : "";
          const { data } = await api.get<{ success: boolean; data: NodeRow[] }>(
            `/projects/${props.projectId}/requirements${query}`,
          );
          if (data.success) reqCache.value = data.data;
        })(),
      );
    }
    if (kindFilter.value === "all" || kindFilter.value === "defect") {
      jobs.push(
        (async () => {
          const query = q ? `?q=${encodeURIComponent(q)}` : "";
          const { data } = await api.get<{ success: boolean; data: NodeRow[] }>(
            `/projects/${props.projectId}/defects${query}`,
          );
          if (data.success) defCache.value = data.data;
        })(),
      );
    }
    await Promise.all(jobs);
  } catch {
    // 忽略：搜索失败不阻断选择器
  } finally {
    loading.value = false;
  }
}

const mergedCandidates = computed(() => {
  const list: { kind: PickedNode["kind"]; row: NodeRow }[] = [];
  if (kindFilter.value !== "defect") {
    for (const r of reqCache.value) list.push({ kind: "requirement", row: r });
  }
  if (kindFilter.value !== "requirement") {
    for (const r of defCache.value) list.push({ kind: "defect", row: r });
  }
  return list;
});

function isSelected(kind: PickedNode["kind"], id: string): boolean {
  return selectedIds.value.has(`${kind}:${id}`);
}

function toggle(kind: PickedNode["kind"], row: NodeRow) {
  const key = `${kind}:${row.id}`;
  if (isSelected(kind, row.id)) {
    model.value = model.value.filter(n => `${n.kind}:${n.id}` !== key);
  } else {
    const node: PickedNode = { kind, id: row.id, label: labelFor(kind, row) };
    if (props.multiple) {
      model.value = [...model.value, node];
    } else {
      model.value = [node];
    }
  }
}

function removeNode(node: PickedNode) {
  model.value = model.value.filter(n => `${n.kind}:${n.id}` !== `${node.kind}:${node.id}`);
}

function openPicker() {
  keyword.value = "";
  open.value = true;
  void fetchCandidates();
}

watch(keyword, () => {
  void fetchCandidates();
});

watch(kindFilter, () => {
  void fetchCandidates();
});
</script>

<template>
  <div class="node-picker">
    <!-- 触发器：已选 chips + 添加按钮 -->
    <div class="picker-trigger">
      <div v-if="model.length > 0" class="picker-chips">
        <span
          v-for="node in model"
          :key="`${node.kind}:${node.id}`"
          class="chip"
        >
          <span class="chip-kind" :class="node.kind">{{ kindLabel(node.kind) }}</span>
          <span class="chip-label" :title="node.id">{{ node.label }}</span>
          <button
            type="button"
            class="chip-remove"
            aria-label="移除"
            @click="removeNode(node)"
          >
            ×
          </button>
        </span>
      </div>
      <button type="button" class="btn btn-sm btn-ghost" @click="openPicker">
        {{ model.length > 0 ? "添加依赖" : placeholder }}
      </button>
    </div>

    <!-- 弹出式选择器 -->
    <AppModal v-model="open" :title="'选择上级依赖'" width-class="max-w-xl">
      <div class="picker-body">
        <div class="picker-toolbar">
          <div class="join">
            <button
              v-for="t in kindTabs"
              :key="t.value"
              type="button"
              class="btn btn-sm join-item"
              :class="kindFilter === t.value ? 'btn-active' : 'btn-ghost'"
              @click="kindFilter = t.value"
            >
              {{ t.label }}
            </button>
          </div>
          <input
            v-model="keyword"
            type="text"
            class="input input-sm flex-1"
            placeholder="按 ID 或描述搜索"
          />
        </div>

        <div class="picker-list">
          <span v-if="loading" class="loading loading-spinner loading-sm" />
          <template v-else-if="mergedCandidates.length > 0">
            <label
              v-for="{ kind, row } in mergedCandidates"
              :key="`${kind}:${row.id}`"
              class="picker-item"
            >
              <input
                type="checkbox"
                class="checkbox checkbox-sm"
                :checked="isSelected(kind, row.id)"
                @change="toggle(kind, row)"
              />
              <span class="item-kind" :class="kind">{{ kindLabel(kind) }}</span>
              <span class="item-id">{{ row.id }}</span>
              <span class="item-desc" :title="row.description ?? ''">
                {{ row.description || "—" }}
              </span>
            </label>
          </template>
          <div v-else class="picker-empty">无匹配节点</div>
        </div>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="open = false">取消</button>
        <button type="button" class="btn btn-primary" @click="open = false">
          确定（已选 {{ model.length }}）
        </button>
      </template>
    </AppModal>
  </div>
</template>

<style scoped>
.node-picker {
  width: 100%;
}

.picker-trigger {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  align-items: center;
}

.picker-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.15rem 0.5rem;
  border-radius: var(--radius-field, 0.5rem);
  background: var(--color-base-200, #f0f0f0);
  font-size: 0.8rem;
  max-width: 100%;
}

.chip-kind {
  font-size: 0.7rem;
  padding: 0 0.35rem;
  border-radius: 0.25rem;
  font-weight: 600;
}

.chip-kind.requirement {
  background: #e0f2fe;
  color: #0369a1;
}

.chip-kind.defect {
  background: #fef3c7;
  color: #b45309;
}

.chip-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 12rem;
}

.chip-remove {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 1rem;
  line-height: 1;
  color: var(--color-base-content, #333);
  opacity: 0.5;
}

.chip-remove:hover {
  opacity: 1;
}

.picker-body {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.picker-toolbar {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.picker-list {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  max-height: 20rem;
  overflow-y: auto;
}

.picker-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.6rem;
  border-radius: var(--radius-field, 0.5rem);
  cursor: pointer;
}

.picker-item:hover {
  background: var(--color-base-200, #f0f0f0);
}

.item-kind {
  font-size: 0.7rem;
  padding: 0 0.35rem;
  border-radius: 0.25rem;
  font-weight: 600;
  flex-shrink: 0;
}

.item-kind.requirement {
  background: #e0f2fe;
  color: #0369a1;
}

.item-kind.defect {
  background: #fef3c7;
  color: #b45309;
}

.item-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 0.75rem;
  color: var(--color-base-content, #333);
  opacity: 0.7;
  flex-shrink: 0;
}

.item-desc {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.85rem;
}

.picker-empty {
  padding: 1rem;
  text-align: center;
  color: var(--color-base-content, #333);
  opacity: 0.5;
}
</style>
