<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter, onBeforeRouteLeave } from "vue-router";
import { MessageSquarePlus } from "@lucide/vue";
import { AppAlert, AppField, AppModal, AppPage, confirm, useToast } from "@/ui";
import MarkdownReader from "@/components/common/MarkdownReader.vue";
import MarkdownCodeMirror from "@/components/common/MarkdownCodeMirror.vue";
import KnowledgeSharePanel from "@/components/projects/knowledge/KnowledgeSharePanel.vue";
import KnowledgeAnnotationPanel from "@/components/projects/knowledge/KnowledgeAnnotationPanel.vue";
import { useAnnotations } from "@/composables/useAnnotations";
import { useAnnotationHighlights } from "@/composables/useAnnotationHighlights";
import { normalizeAnchorText } from "@/utils/annotationAnchor";
import {
  describeVersionConflict,
  parseVersionConflict,
  type VersionConflict,
} from "@/utils/versionConflict";
import { isAbsentRowError } from "@/utils/absentRow";
import {
  selectionToolbarPlacement,
  type SelectionToolbarPlacement,
} from "@/utils/selectionToolbar";
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
  /**
   * 乐观锁版本号（后端 `revision` 列），**必填**。
   *
   * 曾经是可选的，并在 `applyDoc` 里 `?? 0` 兜底「兼容旧后端」。那个兜底把
   * 「响应里没读到这个字段」和「后端确实不下发这个字段」压成同一个值，于是
   * 三个 `loadDoc` 分支漏传 `revision` 时没有任何报错 —— 版本号静默变成 0。
   * 而后端对 0 的定义是「我认为这一行还不存在」，库里却是 1，于是每一次保存
   * 都精确地报「你的版本 0，当前版本 1」。
   *
   * 标成必填后，漏传是**编译期**错误：新增读取分支时不可能再默默丢掉它。
   * 没有真正下发版本号的旧实例也不再需要照顾 —— 缺字段就是读不到版本，
   * `revision` 保持 null、写入被拦，比带着伪造的 0 去撞 409 诚实得多。
   */
  revision: number;
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
/**
 * 当前持有的版本号。读到就存、写成就更新 —— 这是**唯一**的版本来源，
 * 不允许在写入时临时拼一个（见 save() 的注释）。
 *
 * null 表示「还没读到版本」（加载中/加载失败），此时禁止写入。
 * 与 0 的区别很关键：0 是「我认为这行还不存在」，是一次合法的创建意图；
 * null 是「我不知道」，写入应被拦住而不是被当成创建。
 */
const revision = ref<number | null>(null);
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
  // 先置空再读：加载中途（或加载失败）不允许残留上一份版本号。
  // 残留的版本号比没有更危险 —— 它会通过判空检查，然后带着过期版本发出去，
  // 要么误报冲突，要么在对方已改的前提下一路写成。
  revision.value = null;
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
        revision: data.data.revision,
        updatedAt: data.data.updatedAt,
      });
    } else if (isMemory.value) {
      try {
        const { data } = await api.get<{
          success: boolean;
          data: {
            snapshot: string | null;
            /**
             * `GET /memory` 恒下发版本号：缺行时后端给 0（与「首次写入传 0」的
             * 建行语义对齐）。故这里**不标可选** —— 标可选就等于承认「读到了行
             * 却没有版本号」，那种状态没有合法处理方式，只该在类型层面不成立。
             */
            revision: number;
            updatedAt?: string;
          };
        }>(`/projects/${projectId.value}/memory`);
        if (!data.success) throw new Error("fail");
        applyDoc({
          key: MEMORY_KEY,
          title: MEMORY_TITLE,
          content: data.data.snapshot ?? "",
          system: true,
          loadStrategy: "eager",
          revision: data.data.revision,
          updatedAt: data.data.updatedAt,
        });
      } catch (err) {
        // 只有「后端明确说这行不存在」才降级为空文档：此时 revision = 0 是准确的
        // 陈述（这行确实不存在），PUT 的 upsert 正是我们要的建行路径。
        if (!isAbsentRowError(err)) throw err;
        applyDoc({
          key: MEMORY_KEY,
          title: MEMORY_TITLE,
          content: "",
          system: true,
          loadStrategy: "eager",
          // 显式 0 = 「我认为项目记忆还不存在」，与后端 `expected_revision = 0`
          // 的建行分支对齐。**不能省略**：`DocPayload.revision` 是必填的，而且
          // 语义上这里确实知道版本号 —— 就是「不存在」。
          revision: 0,
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
          revision?: number;
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
        revision: data.data.revision,
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
  // 兜底成 null 而不是 0：0 是「我认为这行还不存在」这个**合法的写意图**，
  // 用它兜底会把「没读到版本」伪装成一次创建；null 是「我不知道」，save() 会
  // 据此拦住写入并提示重新加载。同为兜底，方向相反。
  // 类型上 `revision` 必填，但运行时仍可能拿到 undefined（旧实例响应缺字段），
  // 故保留这一层，让 `revision` 的不变量始终是「有效数字或 null」。
  revision.value = doc.revision ?? null;
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

/** 未解决的乐观锁冲突；非 null 时编辑器顶部常驻提示条（toast 会消失，它不会）。 */
const conflict = ref<VersionConflict | null>(null);

/** 冲突发生时刻的本地草稿；重新加载后供对照合并，不被远端版本冲掉。 */
const conflictDraft = ref<string | null>(null);

/** 冲突提示里的版本对比，供提示条与 toast 共用。 */
const conflictText = computed(() =>
  conflict.value ? describeVersionConflict(conflict.value) : "",
);

function clearConflict() {
  conflict.value = null;
}

async function save() {
  if (!isSystem.value && !title.value.trim()) {
    toast.warn("请填写标题");
    return;
  }
  // 没读到版本就不发写请求：宁可让用户点一次重新加载，也不要发一次注定 400 的请求，
  // 更不要「猜」一个版本号 —— 猜中会覆盖别人的改动，猜不中会拿到一个假冲突。
  if (revision.value === null) {
    toast.warn("尚未取得版本号", "请先重新加载文档再保存");
    return;
  }
  const expected = revision.value;
  saving.value = true;
  try {
    if (isConstitution.value) {
      const res = await api.put<{ success: boolean; data?: DocPayload }>(
        `/projects/${projectId.value}/knowledge/constitution`,
        { content: content.value, revision: expected },
      );
      if (!res.data.success) throw new Error("fail");
      if (res.data.data?.revision != null) revision.value = res.data.data.revision;
    } else if (isMemory.value) {
      const res = await api.put<{ success: boolean; data?: DocPayload }>(
        `/projects/${projectId.value}/memory`,
        { snapshot: content.value, revision: expected },
      );
      if (!res.data.success) throw new Error("fail");
      if (res.data.data?.revision != null) revision.value = res.data.data.revision;
    } else {
      const res = await api.put<{ success: boolean; data?: DocPayload }>(
        `/projects/${projectId.value}/knowledge/documents/${docKey.value}`,
        {
          title: title.value.trim(),
          content: content.value,
          loadStrategy: loadStrategy.value,
          parentId: parentId.value,
          revision: expected,
        },
      );
      if (!res.data.success) throw new Error("fail");
      if (res.data.data?.revision != null) revision.value = res.data.data.revision;
      // 关联可能变化：回读面包屑/子文档并同步已保存的父
      const { data } = await api.get<{
        success: boolean;
        data: RelationPayload & { revision?: number };
      }>(`/projects/${projectId.value}/knowledge/documents/${docKey.value}`);
      if (data.success) {
        applyRelation(data.data);
        // 回读顺手带回的版本，用于消掉「保存后 revision 可能又变了」的窗口
        if (data.data.revision != null) revision.value = data.data.revision;
      }
      void loadParentOptions();
    }
    savedContent.value = content.value;
    clearConflictState();
    toast.success("已保存");
  } catch (err) {
    const info = parseVersionConflict(err);
    if (info) {
      // 冲突不是「保存失败」：请求本身没错，是这一行被抢在了前面。
      // 硬重试（拿旧版本再发一次）必然再失败，所以这里不给重试按钮，
      // 只给两件真能解决的事：看最新正文、带着草稿重新加载。
      conflict.value = info;
      toast.warn("写入被拒绝", describeVersionConflict(info));
    } else {
      toast.error("保存失败");
    }
  } finally {
    saving.value = false;
  }
}

/**
 * 冲突后重新加载：**把最新正文取到眼前，同时把草稿收进提示条**。
 *
 * 为什么不直接 `loadDoc()` 了事：当前正文会被对方的版本替换掉，
 * 而用户在编辑器里写的那一段没有任何副本 —— 他只能凭记忆重打。
 * 所以先抓一份草稿再加载，加载完让他自己对照合并。
 *
 * 加载后的状态是很明确的：
 * - `content` = 对方的版本，`savedContent` 也是它（`loadDoc` 里 `applyDoc` 设的），
 *   所以「未保存」标记如实反映「当前编辑器内容 == 已保存版本」；
 * - `revision` = 刚读到的版本，正是下一次写入该用的那一个。
 *   这一点是**版本号在 applyDoc 里重置**换来的 —— 若沿用冲突时的旧版本，
 *   合并后的写入会再撞一次 409，看起来像「重载没用」。
 */
async function reloadAfterConflict() {
  if (!conflict.value) return;
  const draft = content.value;
  try {
    await loadDoc();
  } catch {
    // loadDoc 内部已处理失败态（notFound + toast）
    return;
  }
  conflictDraft.value = draft;
}

/** 放弃合并：把自己的草稿放回编辑器（此时版本号已是最新的，可以直接存）。 */
function restoreConflictDraft() {
  if (conflictDraft.value === null) return;
  content.value = conflictDraft.value;
  // 必须同时挪 `savedContent` 的基准：`loadDoc` 刚把它设成对方的正文，
  // 而我们要的是「我的草稿 vs **已存的**对方正文」这个比较。
  // 不改的话 `dirty` 恒为 true，保存按钮一直亮着而用户什么都没改。
  savedContent.value = conflict.value?.currentContent ?? savedContent.value;
  clearConflictState();
}

function clearConflictState() {
  clearConflict();
  conflictDraft.value = null;
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
    if (await setAnnStatus(id, "resolved", { outcome, resolvedNote: note })) {
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
      <!--
        冲突提示条：常驻在编辑区顶部，不用 toast。
        toast 3 秒后消失，而这条信息要在「去读最新正文 → 回来合并 → 再保存」
        整个过程中一直在场 —— 那过程短则几十秒，长则几分钟。
      -->
      <AppAlert v-if="conflict" severity="warning">
        <div class="flex flex-col gap-2">
          <p class="font-medium">{{ conflictText }}</p>
          <p class="text-xs opacity-80">
            你这一版没有保存，对方的版本已经生效。请对照合并后重新保存 ——
            服务端不做自动合并，直接重试会用同一个旧版本再失败一次。
          </p>
          <div class="flex flex-wrap items-center gap-2">
            <button type="button" class="btn btn-xs btn-primary" @click="reloadAfterConflict">
              加载最新版本（我的草稿会保留）
            </button>
            <button
              v-if="conflictDraft !== null"
              type="button"
              class="btn btn-xs"
              @click="restoreConflictDraft"
            >
              改用我的草稿
            </button>
            <button v-if="conflictDraft !== null" type="button" class="btn btn-xs btn-ghost" @click="clearConflictState">
              知道了
            </button>
          </div>
          <p v-if="conflictDraft !== null" class="text-xs opacity-70">
            你的草稿已暂存（{{ conflictDraft.length }} 字符）；当前编辑器里是对方的最新正文。
          </p>
        </div>
      </AppAlert>

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
