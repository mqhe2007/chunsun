<script setup lang="ts">
import { Eye, Pencil, Trash2 } from "@lucide/vue";
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
  preview: string;
};

const route = useRoute();
const router = useRouter();
const toast = useToast();

const loading = ref(false);
const saving = ref(false);
const contexts = ref<ContextItem[]>([]);

const dialogOpen = ref(false);
const formTitle = ref("");
const formContent = ref("");
const formLoadStrategy = ref<"eager" | "lazy">("eager");

const projectId = () => (route.params as Record<string, string>).id;

const rows = computed<ContextRow[]>(() =>
  contexts.value.map(c => ({
    ...c,
    preview: c.content.trim()
      ? `${c.content.trim().slice(0, 80)}${c.content.trim().length > 80 ? "…" : ""}`
      : "（空）",
  })),
);

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
  formContent.value = "";
  formLoadStrategy.value = "eager";
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
    const res = await api.post<{ success: boolean; data: { id: string } }>(
      `/projects/${projectId()}/knowledge/documents`,
      { title, content: formContent.value, loadStrategy: formLoadStrategy.value },
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

  const ok = await confirm({
    title: "删除知识文档",
    message: `确定删除「${row.title}」？删除后 Agent 将不再加载该文档。`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    const res = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/knowledge/documents/${row.key}`,
    );
    if (!res.data.success) throw new Error("delete failed");
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
          <div class="title-cell">
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
        <AppField label="标题" html-for="ctx-title">
          <input
            id="ctx-title"
            v-model="formTitle"
            type="text"
            class="input w-full"
            maxlength="200"
            placeholder="例如：编码规范、命名约定"
          />
        </AppField>
        <AppField label="正文（可稍后在工作台完善）" html-for="ctx-content">
          <textarea
            id="ctx-content"
            v-model="formContent"
            rows="8"
            class="textarea w-full mono"
            placeholder="Markdown 正文…"
          />
        </AppField>
        <AppField label="加载策略" html-for="ctx-strategy">
          <select
            id="ctx-strategy"
            v-model="formLoadStrategy"
            class="select w-full"
          >
            <option value="eager">启动时加载（默认，适合核心规则）</option>
            <option value="lazy">按需加载（适合参考资料、长文档）</option>
          </select>
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="dialogOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="saving" @click="saveCreate">
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          创建并编辑
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.82rem;
  line-height: 1.45;
}

.title-cell {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  flex-wrap: wrap;
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
