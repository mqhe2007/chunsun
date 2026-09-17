<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter, onBeforeRouteLeave } from "vue-router";
import { MessageSquarePlus } from "@lucide/vue";
import { AppField, AppModal, AppPage, confirm, useToast } from "@/ui";
import MarkdownReader from "@/components/common/MarkdownReader.vue";
import MarkdownCodeMirror from "@/components/common/MarkdownCodeMirror.vue";
import KnowledgeSharePanel from "@/components/projects/knowledge/KnowledgeSharePanel.vue";
import KnowledgeAnnotationPanel from "@/components/projects/knowledge/KnowledgeAnnotationPanel.vue";
import { useAnnotations } from "@/composables/useAnnotations";
import { useAnnotationHighlights } from "@/composables/useAnnotationHighlights";
import { normalizeAnchorText } from "@/utils/annotationAnchor";
import {
  selectionToolbarPlacement,
  type SelectionToolbarPlacement,
} from "@/utils/selectionToolbar";
import { useAuthStore } from "@/stores/auth";
import { api } from "@/utils/api";
import { collectDescendantKeys, visibleKnowledgeRows } from "@/utils/knowledgeTree";
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

type RelationRef = { id: string; title: string };
type ChildRef = { id: string; title: string; loadStrategy?: string };

type RelationPayload = {
  parentId?: string | null;
  breadcrumb?: RelationRef[];
  children?: ChildRef[];
};

type PickerItem = {
  key: string;
  title: string;
  system: boolean;
  parentId?: string | null;
  depth?: number;
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

// 主文档/分册关联（需求 AOzsC2VvzMHL）
const parentId = ref<string | null>(null);
const savedParentId = ref<string | null>(null);
const breadcrumb = ref<RelationRef[]>([]);
const children = ref<ChildRef[]>([]);
const parentOptions = ref<{ key: string; label: string }[]>([]);

const projectId = computed(() => (route.params as Record<string, string>).id ?? "");
const docKey = computed(() => (route.params as Record<string, string>).docKey ?? "");
const isEdit = computed(() => route.query.mode === "edit");
const isConstitution = computed(() => docKey.value === CONSTITUTION_KEY);
const isMemory = computed(() => docKey.value === MEMORY_KEY);
const isSystem = computed(() => isConstitution.value || isMemory.value || system.value);
const shareable = computed(() => !isSystem.value);
const dirty = computed(
  () => content.value !== savedContent.value || parentId.value !== savedParentId.value,
);
const previewHtml = computed(() => renderMarkdown(content.value));

// ---- 批注（需求 u-WPvdvYh4Fw：方案甲第三栏 + 内联高亮 + 选项 B 结案）----

const authStore = useAuthStore();
const {
  annotations: annList,
  loading: annLoading,
  openCount,
  load: loadAnnList,
  create: createAnn,
  updateBody: updateAnnBody,
  setStatus: setAnnStatus,
  remove: removeAnn,
} = useAnnotations(
  () => projectId.value,
  () => docKey.value,
);

const readArea = ref<HTMLElement | null>(null);
/** 桌面端第三栏折叠态；<lg 走 drawer（annDrawerOpen）。 */
const annPanelOpen = ref(true);
const annDrawerOpen = ref(false);
const annCompose = ref<{ anchorText: string; anchorPrefix: string; anchorSuffix: string } | null>(null);
const annComposeOpen = ref(false);
const composeBody = ref("");
const composeSaving = ref(false);
/** 正文选区旁的浮层工具条位置（视口坐标；null = 不显示）。 */
const selectionToolbar = ref<SelectionToolbarPlacement | null>(null);
const annActiveId = ref<string | null>(null);

const annHighlights = useAnnotationHighlights(
  () => readArea.value,
  annList,
  computed(() => !isEdit.value),
);

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
        } & RelationPayload;
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
      applyRelation(data.data);
      void loadParentOptions();
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

/** 关联关系（主文档/分册）落到本地状态；系统项恒无关联。 */
function applyRelation(payload: RelationPayload) {
  parentId.value = payload.parentId ?? null;
  savedParentId.value = parentId.value;
  breadcrumb.value = payload.breadcrumb ?? [];
  children.value = payload.children ?? [];
}

/** 所属主文档候选：排除自身与自身全部后代（防成环），带层级缩进。 */
async function loadParentOptions() {
  try {
    const { data } = await api.get<{
      success: boolean;
      data: { contexts: PickerItem[] };
    }>(`/projects/${projectId.value}/knowledge/documents`);
    if (!data.success) return;
    const items = (data.data.contexts ?? []).filter(c => !c.system);
    const excluded = collectDescendantKeys(items, docKey.value);
    parentOptions.value = visibleKnowledgeRows(items)
      .filter(({ item }) => item.key !== docKey.value && !excluded.has(item.key))
      .map(({ item, depth }) => ({
        key: item.key,
        label: `${"　".repeat(depth)}${item.title}`,
      }));
  } catch {
    // 候选加载失败不阻塞阅读/编辑，下拉退化为仅「无」
    parentOptions.value = [];
  }
}

function openRelated(id: string) {
  router.push({ path: `/projects/${projectId.value}/knowledge/docs/${id}` });
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
          parentId: parentId.value,
        },
      );
      if (!res.data.success) throw new Error("fail");
      // 关联可能变化：回读面包屑/子文档并同步已保存的父
      const { data } = await api.get<{
        success: boolean;
        data: RelationPayload;
      }>(`/projects/${projectId.value}/knowledge/documents/${docKey.value}`);
      if (data.success) applyRelation(data.data);
      void loadParentOptions();
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

// ---- 批注：加载 / 选区创建 / 高亮联动 / 动作处理 ----

async function loadAnnotations() {
  // 宪法 / 记忆 / 自定义文档都走同一端点；docRef 形态由后端 parse_doc_ref 承接
  try {
    await loadAnnList();
  } catch {
    toast.warn("批注加载失败");
  }
}

function toggleAnnotationPanel() {
  if (window.matchMedia("(min-width: 1024px)").matches) {
    annPanelOpen.value = !annPanelOpen.value;
  } else {
    annDrawerOpen.value = true;
  }
}

/** 点击正文：命中内联高亮 → 右栏对应项滚动 + 闪烁。 */
function onReadClick(e: MouseEvent) {
  const id = annHighlights.hitTest(e.clientX, e.clientY);
  if (id) annActiveId.value = id;
}

/** 选区松开 → 记下锚点并在选区旁浮出工具条（工具条只负责“要不要批注”）。 */
function onReadPointerup(e: PointerEvent) {
  if (isEdit.value || e.button !== 0) return;
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return;
  const range = sel.getRangeAt(0);
  const body = (e.currentTarget as HTMLElement).querySelector(".markdown-body");
  if (!body || !body.contains(range.commonAncestorContainer)) return;
  const anchorText = normalizeAnchorText(sel.toString());
  if (!anchorText) return;
  annCompose.value = {
    anchorText,
    anchorPrefix: sliceContext(range.startContainer, range.startOffset, -1),
    anchorSuffix: sliceContext(range.endContainer, range.endOffset, 1),
  };
  selectionToolbar.value = selectionToolbarPlacement(range.getBoundingClientRect(), {
    width: window.innerWidth,
  });
}

function hideSelectionToolbar() {
  selectionToolbar.value = null;
}

/** 工具条「批注」→ 模态框写正文；锚点已在选区松开时记下。 */
function openCompose() {
  selectionToolbar.value = null;
  if (!annCompose.value) return;
  annComposeOpen.value = true;
}

/** 点到工具条以外（含点正文、点面板）就收起工具条。 */
function onDocumentPointerDown(e: MouseEvent) {
  if (!selectionToolbar.value) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest("[data-ann-toolbar]")) return;
  hideSelectionToolbar();
}

function onDocumentKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") hideSelectionToolbar();
}

// 打开模态框时清空并聚焦；关闭时丢掉锚点，避免串到下一条批注。
watch(annComposeOpen, open => {
  composeBody.value = "";
  if (!open) {
    annCompose.value = null;
    return;
  }
  void nextTick(() =>
    (document.getElementById("ann-compose-body") as HTMLTextAreaElement | null)?.focus(),
  );
});

/** 取锚点前后文片段（同节点内最多 20 字符，仅供人眼定位，不做匹配保证）。 */
function sliceContext(node: Node, offset: number, dir: -1 | 1): string {
  if (node.nodeType !== Node.TEXT_NODE) return "";
  const t = node.textContent ?? "";
  return dir < 0 ? t.slice(Math.max(0, offset - 20), offset) : t.slice(offset, offset + 20);
}

async function submitCompose() {
  const body = composeBody.value.trim();
  if (!body || composeSaving.value) return;
  composeSaving.value = true;
  const payload = { body, ...(annCompose.value ?? {}) };
  try {
    const created = await createAnn(payload);
    if (!created) {
      // 保留输入，便于直接重试
      toast.error("添加失败");
      return;
    }
    toast.success("已添加批注");
    annActiveId.value = created.id;
    annComposeOpen.value = false;
  } catch {
    toast.error("添加失败");
  } finally {
    composeSaving.value = false;
  }
}

async function onEditAnnotation(id: string, body: string) {
  try {
    if (await updateAnnBody(id, body)) toast.success("已更新");
    else toast.error("更新失败");
  } catch {
    toast.error("更新失败");
  }
}

async function onResolveAnnotation(
  id: string,
  outcome: "addressed" | "dismissed",
  note: string,
) {
  try {
    if (await setAnnStatus(id, "resolved", { outcome, resolvedNote: note || undefined })) {
      toast.success("已结案");
    } else {
      toast.error("结案失败");
    }
  } catch {
    toast.error("结案失败");
  }
}

async function onReopenAnnotation(id: string) {
  try {
    if (await setAnnStatus(id, "open")) toast.success("已重新打开");
    else toast.error("重新打开失败");
  } catch {
    toast.error("重新打开失败");
  }
}

async function onRemoveAnnotation(id: string) {
  const yes = await confirm({
    title: "删除批注",
    message: "批注删除后不可恢复，确定删除？",
    confirmLabel: "删除",
    danger: true,
  });
  if (!yes) return;
  try {
    if (await removeAnn(id)) toast.success("已删除");
    else toast.error("删除失败");
  } catch {
    toast.error("删除失败");
  }
}

async function onScrollToAnchor(id: string) {
  // 移动端抽屉会盖住正文；先关闭再定位，否则用户看不到滚动结果。
  if (annDrawerOpen.value) {
    annDrawerOpen.value = false;
    await nextTick();
  }
  annHighlights.scrollToAnchor(id);
}

// 内容（重新）渲染后重算内联高亮；编辑态不高亮
watch([content, isEdit, loading, notFound], () => {
  if (!isEdit.value && !loading.value && !notFound.value) annHighlights.recompute();
});

watch([projectId, docKey], () => {
  annActiveId.value = null;
  hideSelectionToolbar();
  annComposeOpen.value = false;
  annCompose.value = null;
  annDrawerOpen.value = false;
  parentId.value = null;
  savedParentId.value = null;
  breadcrumb.value = [];
  children.value = [];
  parentOptions.value = [];
  void loadDoc();
  void loadAnnotations();
});

onMounted(() => {
  document.addEventListener("pointerdown", onDocumentPointerDown, true);
  document.addEventListener("scroll", hideSelectionToolbar, true);
  window.addEventListener("resize", hideSelectionToolbar);
  document.addEventListener("keydown", onDocumentKeydown);
  void loadDoc();
  void loadAnnotations();
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown, true);
  document.removeEventListener("scroll", hideSelectionToolbar, true);
  window.removeEventListener("resize", hideSelectionToolbar);
  document.removeEventListener("keydown", onDocumentKeydown);
});
</script>

<template>
  <AppPage
    :title="pageTitle"
    :back="{ to: `/projects/${projectId}/knowledge`, label: '返回知识库' }"
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
          v-if="!isEdit"
          type="button"
          class="btn btn-ghost"
          @click="toggleAnnotationPanel"
        >
          批注
          <span class="badge badge-sm" :class="openCount > 0 ? 'badge-primary' : 'badge-ghost'">
            {{ openCount }}
          </span>
        </button>
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
        <AppField label="所属主文档" html-for="doc-parent">
          <select id="doc-parent" v-model="parentId" class="select select-bordered w-full">
            <option :value="null">无（根文档）</option>
            <option v-for="opt in parentOptions" :key="opt.key" :value="opt.key">
              {{ opt.label }}
            </option>
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
    <!-- 方案甲三栏：TOC（MarkdownReader 内）｜正文｜批注栏；<lg 批注退化为抽屉 -->
    <div v-else ref="readArea" class="workspace-read flex min-h-0 flex-1 flex-col">
      <div class="drawer drawer-end min-h-0 flex-1">
        <input id="ann-drawer-toggle" v-model="annDrawerOpen" type="checkbox" class="drawer-toggle" />
        <div
          class="drawer-content flex min-h-0 min-w-0 flex-col gap-4 lg:flex-row lg:items-start"
          @click="onReadClick"
          @pointerup="onReadPointerup"
        >
          <div class="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
            <!-- 关联关系（主文档/分册）：阅读态直观呈现归属与分册清单 -->
            <div
              v-if="!isSystem && (breadcrumb.length || children.length)"
              class="relation-strip rounded-box border border-base-300 bg-base-100 px-3 py-2 text-sm shadow-sm"
            >
              <div v-if="breadcrumb.length" class="relation-line">
                <span class="relation-label">所属主文档：</span>
                <template v-for="(b, i) in breadcrumb" :key="b.id">
                  <span v-if="i > 0" class="relation-sep">/</span>
                  <button type="button" class="link link-hover" @click="openRelated(b.id)">
                    {{ b.title }}
                  </button>
                </template>
              </div>
              <div v-if="children.length" class="relation-line">
                <span class="relation-label">分册（{{ children.length }}）：</span>
                <button
                  v-for="c in children"
                  :key="c.id"
                  type="button"
                  class="btn btn-ghost btn-xs"
                  @click="openRelated(c.id)"
                >
                  {{ c.title }}
                </button>
              </div>
            </div>
            <MarkdownReader :content="content" class="min-w-0 flex-1" />
          </div>
          <aside
            v-if="annPanelOpen"
            class="hidden w-64 shrink-0 lg:sticky lg:top-4 lg:block xl:w-72"
          >
            <div
              class="rounded-box border border-base-300 bg-base-100 p-3 shadow-sm lg:max-h-[calc(100vh-6rem)] lg:overflow-y-auto"
            >
              <KnowledgeAnnotationPanel
                :annotations="annList"
                :loading="annLoading"
                :current-user-id="authStore.userId"
                :active-id="annActiveId"
                @edit="onEditAnnotation"
                @resolve="onResolveAnnotation"
                @reopen="onReopenAnnotation"
                @remove="onRemoveAnnotation"
                @scroll-to-anchor="onScrollToAnchor"
              />
            </div>
          </aside>
        </div>
        <div class="drawer-side z-40 lg:hidden">
          <label for="ann-drawer-toggle" class="drawer-overlay" aria-label="关闭批注" />
          <aside class="h-full w-full max-w-md overflow-y-auto bg-base-100 p-3">
            <KnowledgeAnnotationPanel
              :annotations="annList"
              :loading="annLoading"
              :current-user-id="authStore.userId"
              :active-id="annActiveId"
              @edit="onEditAnnotation"
              @resolve="onResolveAnnotation"
              @reopen="onReopenAnnotation"
              @remove="onRemoveAnnotation"
              @scroll-to-anchor="onScrollToAnchor"
            />
          </aside>
        </div>
      </div>

      <!-- 选区工具条：teleport 到 body，用视口坐标 fixed 定位，避免被滚动容器裁剪 -->
      <Teleport to="body">
        <div
          v-if="selectionToolbar"
          data-ann-toolbar
          class="fixed z-[70] whitespace-nowrap"
          :style="{
            left: `${selectionToolbar.x}px`,
            top: `${selectionToolbar.y}px`,
            transform: `translate(-50%, ${selectionToolbar.below ? '0' : '-100%'})`,
          }"
        >
          <div class="flex items-center rounded-box border border-base-300 bg-base-100 p-1 shadow-lg">
            <button type="button" class="btn btn-ghost btn-xs gap-1" @click="openCompose">
              <MessageSquarePlus :size="14" aria-hidden="true" />
              批注
            </button>
          </div>
        </div>
      </Teleport>

      <!-- 新建批注：先框选正文（工具条）→ 这里写正文 -->
      <AppModal v-model="annComposeOpen" title="添加批注">
        <div class="flex flex-col gap-3">
          <blockquote
            v-if="annCompose?.anchorText"
            class="max-h-24 overflow-y-auto border-l-2 border-primary/60 pl-2 text-xs leading-relaxed text-base-content/70"
          >
            {{ annCompose.anchorText }}
          </blockquote>
          <p v-else class="text-xs text-base-content/50">未选中文本，将作为整篇批注。</p>
          <textarea
            id="ann-compose-body"
            v-model="composeBody"
            class="textarea textarea-bordered w-full text-sm"
            rows="4"
            placeholder="写下你的批注…"
            @keydown.ctrl.enter="submitCompose"
            @keydown.meta.enter="submitCompose"
          />
        </div>
        <template #footer>
          <button type="button" class="btn btn-ghost" @click="annComposeOpen = false">取消</button>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="!composeBody.trim() || composeSaving"
            @click="submitCompose"
          >
            <span v-if="composeSaving" class="loading loading-spinner loading-xs" />
            提交批注
          </button>
        </template>
      </AppModal>
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

.relation-strip {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.relation-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.3rem;
}

.relation-label {
  font-size: 0.8rem;
  color: color-mix(in oklab, var(--color-base-content) 55%, transparent);
}

.relation-sep {
  color: color-mix(in oklab, var(--color-base-content) 40%, transparent);
}
</style>
