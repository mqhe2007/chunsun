<script setup lang="ts">
import { ChevronDown, ChevronRight, Eye, Pencil, Trash2 } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  AppColumn,
  AppField,
  AppModal,
  AppPage,
  AppTable,
  confirm,
  useToast,
} from "@/ui";
import { api } from "@/utils/api";
import { visibleKnowledgeRows } from "@/utils/knowledgeTree";

const CONSTITUTION_KEY = "constitution";
const CONSTITUTION_TITLE = "项目宪法";
const MEMORY_KEY = "memory";
const MEMORY_TITLE = "项目记忆";

type ContextItem = {
  key: string;
  title: string;
  content: string;
  system: boolean;
  loadStrategy?: string;
  parentId?: string | null;
  depth?: number;
};

type ContextsPayload = {
  contexts: ContextItem[];
};

type ContextRow = {
  key: string;
  title: string;
  content: string;
  system: boolean;
  loadStrategy?: string;
  parentId?: string | null;
  depth: number;
  childCount: number;
  preview: string;
};

const route = useRoute();
const router = useRouter();
const toast = useToast();

const loading = ref(false);
const saving = ref(false);
const contexts = ref<ContextItem[]>([]);
/** 折叠的主文档 key（默认全部展开） */
const collapsedKeys = ref<Set<string>>(new Set());

const dialogOpen = ref(false);
const formTitle = ref("");
const formLoadStrategy = ref<"eager" | "lazy">("eager");
const formParentId = ref("");

const projectId = () => (route.params as Record<string, string>).id;

const rows = computed<ContextRow[]>(() =>
  visibleKnowledgeRows(contexts.value, collapsedKeys.value).map(({ item, depth, childCount }) => ({
    ...item,
    depth,
    childCount,
    preview: item.content.trim()
      ? `${item.content.trim().slice(0, 80)}${item.content.trim().length > 80 ? "…" : ""}`
      : "（空）",
  })),
);

/** 「所属主文档」候选：自定义文档（系统项恒为根，不作为父），带层级缩进。 */
const parentOptions = computed(() =>
  visibleKnowledgeRows(contexts.value.filter(c => !c.system)).map(({ item, depth }) => ({
    key: item.key,
    label: `${"　".repeat(depth)}${item.title}`,
  })),
);

function isCollapsed(key: string) {
  return collapsedKeys.value.has(key);
}

function toggleCollapse(key: string) {
  const next = new Set(collapsedKeys.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  collapsedKeys.value = next;
}

function applyPayload(data: ContextsPayload) {
  const list = data.contexts ?? [];
  const constitution = list.find(c => c.key === CONSTITUTION_KEY);
  const memory = list.find(c => c.key === MEMORY_KEY);
  const customs = list.filter(
    c => c.key !== CONSTITUTION_KEY && c.key !== MEMORY_KEY,
  );
  contexts.value = [
    ...(constitution ? [constitution] : []),
    ...(memory ? [memory] : []),
    ...customs,
  ];
}

async function fetchContexts() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: ContextsPayload }>(
      `/projects/${projectId()}/knowledge/documents`,
    );
    if (data.success) {
      applyPayload(data.data);
    } else {
      toast.error("获取失败", "无法加载知识");
    }
  } catch {
    toast.error("获取失败", "加载知识失败");
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  formTitle.value = "";
  formLoadStrategy.value = "eager";
  formParentId.value = "";
  dialogOpen.value = true;
}

function openDoc(row: ContextRow, mode?: "edit") {
  const q = mode === "edit" ? { mode: "edit" } : {};
  router.push({
    path: `/projects/${projectId()}/knowledge/docs/${row.key}`,
    query: q,
  });
}

async function saveCreate() {
  const title = formTitle.value.trim();
  if (!title) {
    toast.warn("请填写标题");
    return;
  }

  saving.value = true;
  try {
    const body: Record<string, unknown> = {
      title,
      content: "",
      loadStrategy: formLoadStrategy.value,
    };
    if (formParentId.value) body.parentId = formParentId.value;
    const res = await api.post<{ success: boolean; data: { id: string } }>(
      `/projects/${projectId()}/knowledge/documents`,
      body,
    );
    if (!res.data.success) throw new Error("create failed");
    dialogOpen.value = false;
    toast.success("已创建");
    const id = res.data.data?.id;
    if (id) {
      await router.push({
        path: `/projects/${projectId()}/knowledge/docs/${id}`,
        query: { mode: "edit" },
      });
    } else {
      await fetchContexts();
    }
  } catch {
    toast.error("保存失败", "请稍后重试");
  } finally {
    saving.value = false;
  }
}

async function confirmDelete(row: ContextRow) {
  if (row.system || row.key === CONSTITUTION_KEY) return;

  const cascade = row.childCount > 0;
  const ok = await confirm({
    title: "删除知识文档",
    message: cascade
      ? `「${row.title}」下有 ${row.childCount} 篇分册，将连同全部分册一起删除，且不可恢复。`
      : `确定删除「${row.title}」？删除后 Agent 将不再加载该文档。`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    const suffix = cascade ? "?withChildren=true" : "";
    const res = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/knowledge/documents/${row.key}${suffix}`,
    );
    if (!res.data.success) throw new Error("delete failed");
    const next = new Set(collapsedKeys.value);
    next.delete(row.key);
    collapsedKeys.value = next;
    toast.success("已删除");
    await fetchContexts();
  } catch {
    toast.error("删除失败");
  }
}

onMounted(fetchContexts);
</script>

<template>
  <AppPage title="知识库">
    <template #actions>
      <button
        type="button"
        class="btn btn-ghost"
        :disabled="loading"
        @click="fetchContexts"
      >
        <span v-if="loading" class="loading loading-spinner loading-xs" />
        刷新
      </button>
      <button type="button" class="btn btn-primary" @click="openCreate">添加文档</button>
    </template>

    <AppTable
      :rows="rows"
      :loading="loading"
      empty="暂无知识文档"
      striped
    >
      <AppColumn header="标题">
        <template #default="{ row }">
          <div
            class="title-cell"
            :style="{ paddingLeft: `${(row as ContextRow).depth * 1.25}rem` }"
          >
            <button
              v-if="(row as ContextRow).childCount > 0"
              type="button"
              class="btn btn-ghost btn-xs btn-square"
              :aria-label="isCollapsed((row as ContextRow).key) ? '展开分册' : '收起分册'"
              :title="isCollapsed((row as ContextRow).key) ? '展开分册' : '收起分册'"
              @click="toggleCollapse((row as ContextRow).key)"
            >
              <ChevronRight v-if="isCollapsed((row as ContextRow).key)" :size="14" aria-hidden="true" />
              <ChevronDown v-else :size="14" aria-hidden="true" />
            </button>
            <span v-else class="tree-spacer" aria-hidden="true" />
            <button
              type="button"
              class="link link-hover text-left"
              @click="openDoc(row as ContextRow)"
            >
              {{ (row as ContextRow).title }}
            </button>
            <span v-if="(row as ContextRow).system" class="badge badge-ghost">
              固定
            </span>
            <span
              v-if="(row as ContextRow).childCount > 0"
              class="badge badge-ghost badge-sm"
            >
              {{ (row as ContextRow).childCount }} 分册
            </span>
          </div>
        </template>
      </AppColumn>
      <AppColumn header="内容预览">
        <template #default="{ row }">
          <span class="preview-cell">{{ (row as ContextRow).preview }}</span>
        </template>
      </AppColumn>
      <AppColumn header="加载策略" width="6rem">
        <template #default="{ row }">
          <span
            :class="[
              'badge',
              (row as ContextRow).loadStrategy === 'lazy' ? 'badge-warning' : 'badge-success',
            ]"
          >
            {{ (row as ContextRow).loadStrategy === 'lazy' ? '按需' : '启动' }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="操作" width="9rem">
        <template #default="{ row }">
          <div class="row-actions">
            <button
              type="button"
              class="btn btn-ghost btn-sm btn-square"
              aria-label="阅读"
              title="阅读"
              @click="openDoc(row as ContextRow)"
            >
              <Eye :size="16" aria-hidden="true" />
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-sm btn-square"
              aria-label="编辑"
              title="编辑"
              @click="openDoc(row as ContextRow, 'edit')"
            >
              <Pencil :size="16" aria-hidden="true" />
            </button>
            <button
              v-if="!(row as ContextRow).system"
              type="button"
              class="btn btn-ghost btn-sm btn-square btn-error"
              aria-label="删除"
              @click="confirmDelete(row as ContextRow)"
            >
              <Trash2 :size="16" aria-hidden="true" />
            </button>
          </div>
        </template>
      </AppColumn>
    </AppTable>

    <AppModal v-model="dialogOpen" title="添加文档">
      <div class="dialog-form">
        <p class="text-sm text-base-content/60">
          先填写标题与加载策略，创建后进入文档工作台用分屏编辑器写正文。
        </p>
        <AppField label="标题" html-for="ctx-title">
          <input
            id="ctx-title"
            v-model="formTitle"
            type="text"
            class="input input-bordered w-full"
            maxlength="200"
            placeholder="例如：编码规范、命名约定"
            @keydown.enter.prevent="saveCreate"
          />
        </AppField>
        <AppField label="加载策略" html-for="ctx-strategy">
          <select
            id="ctx-strategy"
            v-model="formLoadStrategy"
            class="select select-bordered w-full"
          >
            <option value="eager">启动时加载（默认，适合核心规则）</option>
            <option value="lazy">按需加载（适合参考资料、长文档）</option>
          </select>
        </AppField>
        <AppField label="所属主文档" html-for="ctx-parent">
          <select
            id="ctx-parent"
            v-model="formParentId"
            class="select select-bordered w-full"
          >
            <option value="">无（根文档）</option>
            <option v-for="opt in parentOptions" :key="opt.key" :value="opt.key">
              {{ opt.label }}
            </option>
          </select>
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="dialogOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="saving" @click="saveCreate">
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          创建并开始编辑
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.title-cell {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
}

.tree-spacer {
  display: inline-block;
  width: 1.5rem;
  flex: none;
}

.preview-cell {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  font-size: 0.875rem;
}

.row-actions {
  display: inline-flex;
  flex-wrap: nowrap;
  gap: 0.15rem;
}

.dialog-form {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}
</style>
