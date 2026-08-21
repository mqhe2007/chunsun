<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
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

type ContextItem = {
  key: string;
  title: string;
  content: string;
  system: boolean;
};

type ContextsPayload = {
  contexts: ContextItem[];
};

type ContextRow = {
  key: string;
  title: string;
  content: string;
  system: boolean;
  preview: string;
};

const route = useRoute();
const toast = useToast();

const loading = ref(false);
const saving = ref(false);
const contexts = ref<ContextItem[]>([]);

const dialogOpen = ref(false);
/** null = 新建自定义；constitution / id = 编辑 */
const editingKey = ref<string | null>(null);
const formTitle = ref("");
const formContent = ref("");

const projectId = () => (route.params as Record<string, string>).id;

const isEditingConstitution = computed(
  () => editingKey.value === CONSTITUTION_KEY,
);

const dialogHeader = computed(() => {
  if (editingKey.value === null) return "添加文档";
  if (isEditingConstitution.value) return "编辑项目宪法";
  return "编辑文档";
});

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
  const customs = list.filter(c => c.key !== CONSTITUTION_KEY);
  contexts.value = constitution ? [constitution, ...customs] : customs;
}

async function fetchContexts() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: ContextsPayload }>(
      `/projects/${projectId()}/contexts`,
    );
    if (data.success) {
      applyPayload(data.data);
    } else {
      toast.error("获取失败", "无法加载上下文配置");
    }
  } catch {
    toast.error("获取失败", "加载上下文配置失败");
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  editingKey.value = null;
  formTitle.value = "";
  formContent.value = "";
  dialogOpen.value = true;
}

function openEdit(row: ContextRow) {
  editingKey.value = row.key;
  formTitle.value = row.system ? CONSTITUTION_TITLE : row.title;
  formContent.value = row.content;
  dialogOpen.value = true;
}

async function saveDoc() {
  const title = isEditingConstitution.value
    ? CONSTITUTION_TITLE
    : formTitle.value.trim();

  if (!isEditingConstitution.value && !title) {
    toast.warn("请填写标题");
    return;
  }

  saving.value = true;
  try {
    if (editingKey.value === null) {
      const res = await api.post<{ success: boolean }>(
        `/projects/${projectId()}/contexts`,
        { title, content: formContent.value },
      );
      if (!res.data.success) throw new Error("create failed");
    } else if (isEditingConstitution.value) {
      const res = await api.put<{ success: boolean }>(
        `/projects/${projectId()}/contexts/constitution`,
        { content: formContent.value },
      );
      if (!res.data.success) throw new Error("constitution update failed");
    } else {
      const res = await api.put<{ success: boolean }>(
        `/projects/${projectId()}/contexts/${editingKey.value}`,
        { title, content: formContent.value },
      );
      if (!res.data.success) throw new Error("update failed");
    }

    dialogOpen.value = false;
    toast.success("已保存");
    await fetchContexts();
  } catch {
    toast.error("保存失败", "请稍后重试");
  } finally {
    saving.value = false;
  }
}

async function confirmDelete(row: ContextRow) {
  if (row.system || row.key === CONSTITUTION_KEY) return;

  const ok = await confirm({
    title: "删除上下文文档",
    message: `确定删除「${row.title}」？删除后 Agent 将不再加载该文档。`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    const res = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/contexts/${row.key}`,
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
  <AppPage title="上下文配置">
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
      empty="暂无上下文文档"
      striped
    >
      <AppColumn header="标题">
        <template #default="{ row }">
          <div class="title-cell">
            <span>{{ (row as ContextRow).title }}</span>
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
      <AppColumn header="操作" width="7rem">
        <template #default="{ row }">
          <div class="row-actions">
            <button
              type="button"
              class="btn btn-ghost btn-sm btn-square"
              aria-label="编辑"
              @click="openEdit(row as ContextRow)"
            >
              ✎
            </button>
            <button
              v-if="!(row as ContextRow).system"
              type="button"
              class="btn btn-ghost btn-sm btn-square btn-error"
              aria-label="删除"
              @click="confirmDelete(row as ContextRow)"
            >
              ✕
            </button>
          </div>
        </template>
      </AppColumn>
    </AppTable>

    <AppModal v-model="dialogOpen" :title="dialogHeader">
      <div class="dialog-form">
        <AppField label="标题" html-for="ctx-title">
          <input
            id="ctx-title"
            v-model="formTitle"
            type="text"
            class="input w-full"
            maxlength="200"
            :disabled="isEditingConstitution"
            :placeholder="
              isEditingConstitution
                ? CONSTITUTION_TITLE
                : '例如：编码规范、命名约定'
            "
          />
        </AppField>
        <p v-if="isEditingConstitution" class="hint text-base-content/60">
          项目宪法为系统固定项，标题不可更改、不可删除。
        </p>
        <AppField label="正文" html-for="ctx-content">
          <textarea
            id="ctx-content"
            v-model="formContent"
            rows="14"
            class="textarea w-full mono"
            :placeholder="
              isEditingConstitution
                ? '# 项目宪法\n\n## 核心原则\n- …\n\n## 技术约束\n- …'
                : 'Markdown 正文…'
            "
          />
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="dialogOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="saving" @click="saveDoc">
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          保存
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>

.hint {
  margin: 0;
  font-size: 0.85rem;
  line-height: 1.45;
}

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
