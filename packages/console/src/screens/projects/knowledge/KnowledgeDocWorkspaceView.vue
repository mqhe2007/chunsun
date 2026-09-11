<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter, onBeforeRouteLeave } from "vue-router";
import { AppField, AppPage, confirm, useToast } from "@/ui";
import MarkdownReader from "@/components/common/MarkdownReader.vue";
import MarkdownCodeMirror from "@/components/common/MarkdownCodeMirror.vue";
import KnowledgeSharePanel from "@/components/projects/knowledge/KnowledgeSharePanel.vue";
import { api } from "@/utils/api";
import { renderMarkdown } from "@/utils/markdown";

const CONSTITUTION_KEY = "constitution";
const MEMORY_KEY = "memory";
const CONSTITUTION_TITLE = "项目宪法";
const MEMORY_TITLE = "项目记忆";

type DocPayload = {
  key: string;
  title: string;
  content: string;
  system: boolean;
  loadStrategy?: string;
  updatedAt?: string;
};

const route = useRoute();
const router = useRouter();
const toast = useToast();

const loading = ref(true);
const saving = ref(false);
const shareOpen = ref(false);
const title = ref("");
const content = ref("");
const savedContent = ref("");
const loadStrategy = ref<"eager" | "lazy">("eager");
const system = ref(false);
const updatedAt = ref<string | null>(null);
const notFound = ref(false);

const projectId = computed(() => (route.params as Record<string, string>).id ?? "");
const docKey = computed(() => (route.params as Record<string, string>).docKey ?? "");
const isEdit = computed(() => route.query.mode === "edit");
const isConstitution = computed(() => docKey.value === CONSTITUTION_KEY);
const isMemory = computed(() => docKey.value === MEMORY_KEY);
const isSystem = computed(() => isConstitution.value || isMemory.value || system.value);
const shareable = computed(() => !isSystem.value);
const dirty = computed(() => content.value !== savedContent.value);
const previewHtml = computed(() => renderMarkdown(content.value));

const pageTitle = computed(() => {
  if (isConstitution.value) return CONSTITUTION_TITLE;
  if (isMemory.value) return MEMORY_TITLE;
  return title.value || "文档";
});

async function loadDoc() {
  loading.value = true;
  notFound.value = false;
  try {
    if (isConstitution.value) {
      const { data } = await api.get<{ success: boolean; data: DocPayload }>(
        `/projects/${projectId.value}/knowledge/constitution`,
      );
      if (!data.success) throw new Error("fail");
      applyDoc({
        key: CONSTITUTION_KEY,
        title: CONSTITUTION_TITLE,
        content: data.data.content ?? "",
        system: true,
        loadStrategy: "eager",
        updatedAt: data.data.updatedAt,
      });
    } else if (isMemory.value) {
      try {
        const { data } = await api.get<{
          success: boolean;
          data: { snapshot: string | null; updatedAt?: string };
        }>(`/projects/${projectId.value}/memory`);
        if (!data.success) throw new Error("fail");
        applyDoc({
          key: MEMORY_KEY,
          title: MEMORY_TITLE,
          content: data.data.snapshot ?? "",
          system: true,
          loadStrategy: "eager",
          updatedAt: data.data.updatedAt,
        });
      } catch {
        // 尚无记忆行时允许从空文档开始编辑（PUT 会 upsert）
        applyDoc({
          key: MEMORY_KEY,
          title: MEMORY_TITLE,
          content: "",
          system: true,
          loadStrategy: "eager",
        });
      }
    } else {
      const { data } = await api.get<{
        success: boolean;
        data: {
          id: string;
          title: string;
          content: string;
          loadStrategy?: string;
          updatedAt?: string;
        };
      }>(`/projects/${projectId.value}/knowledge/documents/${docKey.value}`);
      if (!data.success) throw new Error("fail");
      applyDoc({
        key: data.data.id,
        title: data.data.title,
        content: data.data.content ?? "",
        system: false,
        loadStrategy: (data.data.loadStrategy as "eager" | "lazy") || "eager",
        updatedAt: data.data.updatedAt,
      });
    }
  } catch {
    notFound.value = true;
    toast.error("文档不存在或无权访问");
  } finally {
    loading.value = false;
  }
}

function applyDoc(doc: DocPayload) {
  title.value = doc.title;
  content.value = doc.content;
  savedContent.value = doc.content;
  system.value = doc.system;
  loadStrategy.value = (doc.loadStrategy as "eager" | "lazy") || "eager";
  updatedAt.value = doc.updatedAt ?? null;
}

async function save() {
  if (!isSystem.value && !title.value.trim()) {
    toast.warn("请填写标题");
    return;
  }
  saving.value = true;
  try {
    if (isConstitution.value) {
      const res = await api.put<{ success: boolean }>(
        `/projects/${projectId.value}/knowledge/constitution`,
        { content: content.value },
      );
      if (!res.data.success) throw new Error("fail");
    } else if (isMemory.value) {
      const res = await api.put<{ success: boolean }>(
        `/projects/${projectId.value}/memory`,
        { snapshot: content.value },
      );
      if (!res.data.success) throw new Error("fail");
    } else {
      const res = await api.put<{ success: boolean }>(
        `/projects/${projectId.value}/knowledge/documents/${docKey.value}`,
        {
          title: title.value.trim(),
          content: content.value,
          loadStrategy: loadStrategy.value,
        },
      );
      if (!res.data.success) throw new Error("fail");
    }
    savedContent.value = content.value;
    toast.success("已保存");
  } catch {
    toast.error("保存失败");
  } finally {
    saving.value = false;
  }
}

function goRead() {
  router.replace({
    path: `/projects/${projectId.value}/knowledge/docs/${docKey.value}`,
    query: {},
  });
}

function goEdit() {
  router.replace({
    path: `/projects/${projectId.value}/knowledge/docs/${docKey.value}`,
    query: { mode: "edit" },
  });
}

onBeforeRouteLeave(async () => {
  if (!dirty.value || !isEdit.value) return true;
  return confirm({
    title: "未保存的更改",
    message: "离开将丢失未保存内容，确定离开？",
    confirmLabel: "离开",
    danger: true,
  });
});

watch([projectId, docKey], () => {
  void loadDoc();
});

onMounted(loadDoc);
</script>

<template>
  <AppPage
    :title="pageTitle"
    :back="{ to: `/projects/${projectId}/knowledge`, label: '返回知识库' }"
    fill
  >
    <template #title-extra>
      <span v-if="isSystem" class="badge badge-ghost">固定</span>
      <span
        v-if="!isSystem"
        class="badge"
        :class="loadStrategy === 'lazy' ? 'badge-warning' : 'badge-success'"
      >
        {{ loadStrategy === "lazy" ? "按需" : "启动" }}
      </span>
      <span v-if="updatedAt" class="text-xs text-base-content/50">
        更新 {{ new Date(updatedAt).toLocaleString() }}
      </span>
      <span v-if="dirty && isEdit" class="badge badge-outline badge-warning">未保存</span>
    </template>
    <template #actions>
      <template v-if="!loading && !notFound">
        <button
          v-if="shareable"
          type="button"
          class="btn btn-ghost"
          @click="shareOpen = true"
        >
          分享
        </button>
        <button
          v-if="isEdit"
          type="button"
          class="btn btn-ghost"
          @click="goRead"
        >
          阅读
        </button>
        <button
          v-else
          type="button"
          class="btn btn-ghost"
          @click="goEdit"
        >
          编辑
        </button>
        <button
          v-if="isEdit"
          type="button"
          class="btn btn-primary"
          :disabled="saving || !dirty"
          @click="save"
        >
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          保存
        </button>
      </template>
    </template>

    <div v-if="loading" class="flex justify-center py-16">
      <span class="loading loading-spinner loading-lg" />
    </div>
    <div v-else-if="notFound" class="py-16 text-center text-base-content/60">
      文档不存在
    </div>
    <div v-else-if="isEdit" class="workspace-edit flex min-h-0 flex-1 flex-col gap-3">
      <div v-if="!isSystem" class="grid gap-3 md:grid-cols-2">
        <AppField label="标题" html-for="doc-title">
          <input
            id="doc-title"
            v-model="title"
            type="text"
            class="input input-bordered w-full"
            maxlength="200"
          />
        </AppField>
        <AppField label="加载策略" html-for="doc-strategy">
          <select id="doc-strategy" v-model="loadStrategy" class="select select-bordered w-full">
            <option value="eager">启动时加载</option>
            <option value="lazy">按需加载</option>
          </select>
        </AppField>
      </div>
      <p v-else class="text-sm text-base-content/60">
        {{ isConstitution ? "项目宪法为系统固定项，标题不可更改、不可删除、不可分享。" : "项目记忆为系统固定项，标题不可更改、不可删除、不可分享。" }}
      </p>
      <div class="edit-split grid min-h-[28rem] flex-1 gap-3 lg:grid-cols-2">
        <MarkdownCodeMirror v-model="content" class="min-h-72" @save="save" />
        <div class="preview-pane overflow-auto rounded-box border border-base-300 bg-base-100 p-4">
          <p class="mb-2 text-xs font-medium text-base-content/50">预览</p>
          <div class="markdown-body" v-html="previewHtml" />
        </div>
      </div>
    </div>
    <div v-else class="workspace-read">
      <MarkdownReader :content="content" />
    </div>

    <KnowledgeSharePanel
      v-model:open="shareOpen"
      :project-id="projectId"
      :doc-id="docKey"
      :shareable="shareable"
    />
  </AppPage>
</template>

<style scoped>
.workspace-edit {
  min-height: calc(100vh - 12rem);
}
</style>
