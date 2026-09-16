<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { AppField } from "@/ui";
import type { KnowledgeAnnotation } from "@/composables/useAnnotations";

/**
 * 批注面板（需求 u-WPvdvYh4Fw，方案甲：右侧第三栏常驻）。
 *
 * 权限到按钮级：编辑 / 删除仅作者（后端 403 兜底）；结案 / 重新打开所有成员可用
 * （选项 B：AI 可结案，人可重开兜底）。面板本身不做权限判断以外的数据操作，
 * 全部动作 emit 给宿主（宿主持有 useAnnotations 状态，桌面栏与移动抽屉两个
 * 实例共享同一数据源）。
 */

const props = defineProps<{
  annotations: KnowledgeAnnotation[];
  loading: boolean;
  currentUserId: string | null;
  /** 选区带入的锚点（null = 整篇批注）。 */
  compose: { anchorText: string; anchorPrefix: string; anchorSuffix: string } | null;
  composeOpen: boolean;
  /** 点击正文高亮时设置的批注 id，联动滚动到这里。 */
  activeId: string | null;
}>();

const emit = defineEmits<{
  "update:composeOpen": [value: boolean];
  "submit-compose": [body: string];
  edit: [id: string, body: string];
  resolve: [id: string, outcome: "addressed" | "dismissed", note: string];
  reopen: [id: string];
  remove: [id: string];
}>();

const composeBody = ref("");
const composeSaving = ref(false);

const editingId = ref<string | null>(null);
const editingBody = ref("");

const resolveId = ref<string | null>(null);
const resolveOutcome = ref<"addressed" | "dismissed">("addressed");
const resolveNote = ref("");

const openList = () => props.annotations.filter(a => a.status === "open" || a.status === "stale");
const resolvedList = () => props.annotations.filter(a => a.status === "resolved");

function isAuthor(ann: KnowledgeAnnotation): boolean {
  return Boolean(props.currentUserId) && ann.createdBy === props.currentUserId;
}

function timeLabel(iso: string): string {
  return new Date(iso).toLocaleString();
}

const outcomeLabel: Record<string, string> = {
  addressed: "已按批注处理",
  dismissed: "判断无需修改",
};

watch(
  () => props.composeOpen,
  open => {
    if (open) {
      composeBody.value = "";
      void nextTick(() => (document.getElementById("ann-compose-body") as HTMLTextAreaElement | null)?.focus());
    }
  },
);

// 点击正文高亮 → 滚动到对应项并闪一下
const itemRefs = new Map<string, HTMLElement>();
function setItemRef(id: string, el: unknown) {
  if (el) itemRefs.set(id, el as HTMLElement);
  else itemRefs.delete(id);
}
watch(
  () => props.activeId,
  async id => {
    if (!id) return;
    await nextTick();
    const el = itemRefs.get(id);
    if (!el) return;
    el.scrollIntoView({ block: "nearest", behavior: "smooth" });
    el.classList.remove("ann-flash");
    void el.offsetWidth; // 重启动画
    el.classList.add("ann-flash");
  },
);

async function submitCompose() {
  if (!composeBody.value.trim() || composeSaving.value) return;
  composeSaving.value = true;
  emit("submit-compose", composeBody.value);
  // 宿主会关闭 composeOpen；这里不重置 saving（由 v-if 卸载表单）
  composeSaving.value = false;
}

function startEdit(ann: KnowledgeAnnotation) {
  editingId.value = ann.id;
  editingBody.value = ann.body;
}
function submitEdit() {
  if (editingId.value && editingBody.value.trim()) {
    emit("edit", editingId.value, editingBody.value);
  }
  editingId.value = null;
  editingBody.value = "";
}

function startResolve(id: string) {
  resolveId.value = id;
  resolveOutcome.value = "addressed";
  resolveNote.value = "";
}

function anchorQuote(ann: KnowledgeAnnotation): string {
  return (ann.anchorText ?? "").trim();
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3">
    <p class="text-[11px] leading-4 text-base-content/50">
      批注仅作用于本文档；跨需求的一次性方向请写入需求工作记忆，不要留在批注里。
    </p>

    <!-- 新建（选中文本 → 内联批注；无选区 → 整篇批注） -->
    <div v-if="composeOpen" class="card bg-base-200/60 card-compact">
      <div class="card-body gap-2">
        <blockquote
          v-if="compose?.anchorText"
          class="border-l-2 border-primary/60 pl-2 text-xs text-base-content/70 line-clamp-2"
        >
          {{ compose.anchorText }}
        </blockquote>
        <p v-else class="text-xs text-base-content/50">未选中文本，将作为整篇批注。</p>
        <textarea
          id="ann-compose-body"
          v-model="composeBody"
          class="textarea textarea-bordered w-full text-sm"
          rows="3"
          placeholder="写下你的批注…"
        />
        <div class="flex justify-end gap-2">
          <button type="button" class="btn btn-ghost btn-xs" @click="emit('update:composeOpen', false)">
            取消
          </button>
          <button
            type="button"
            class="btn btn-primary btn-xs"
            :disabled="!composeBody.trim() || composeSaving"
            @click="submitCompose"
          >
            提交批注
          </button>
        </div>
      </div>
    </div>

    <div v-if="loading" class="flex justify-center py-6">
      <span class="loading loading-spinner" />
    </div>

    <template v-else>
      <!-- 未处理：open + stale（stale 带「原文已变更」标记，永不静默删除） -->
      <div class="flex flex-col gap-2">
        <h4 class="text-sm font-medium">
          未处理
          <span class="badge badge-primary badge-sm ml-1">{{ openList().length }}</span>
        </h4>
        <p v-if="openList().length === 0" class="text-xs text-base-content/50">
          暂无未处理批注。选中正文即可添加内联批注，也可对整篇发表批注。
        </p>
        <div
          v-for="ann in openList()"
          :key="ann.id"
          :ref="el => setItemRef(ann.id, el)"
          class="card bg-base-200/40 card-compact scroll-mt-4"
        >
          <div class="card-body gap-1.5">
            <blockquote
              v-if="anchorQuote(ann)"
              class="border-l-2 border-base-content/20 pl-2 text-xs text-base-content/60 line-clamp-2"
            >
              {{ anchorQuote(ann) }}
            </blockquote>
            <p v-else class="text-xs text-base-content/50">整篇批注</p>

            <p class="text-sm whitespace-pre-wrap">{{ ann.body }}</p>

            <div class="flex flex-wrap items-center gap-1.5">
              <span v-if="ann.status === 'stale'" class="badge badge-warning badge-sm">原文已变更</span>
              <span class="text-[11px] text-base-content/50">
                {{ ann.createdBy }} · {{ timeLabel(ann.createdAt) }}
              </span>
            </div>

            <!-- 结案表单（选项 B：须给依据，人可对照正文核查） -->
            <div v-if="resolveId === ann.id" class="mt-1 flex flex-col gap-1.5 border-t border-base-300 pt-2">
              <div class="flex gap-2">
                <label class="label cursor-pointer gap-1 text-xs">
                  <input
                    v-model="resolveOutcome"
                    type="radio"
                    name="ann-outcome"
                    value="addressed"
                    class="radio radio-primary radio-xs"
                  />
                  已按批注处理
                </label>
                <label class="label cursor-pointer gap-1 text-xs">
                  <input
                    v-model="resolveOutcome"
                    type="radio"
                    name="ann-outcome"
                    value="dismissed"
                    class="radio radio-primary radio-xs"
                  />
                  判断无需修改
                </label>
              </div>
              <AppField label="处理依据" html-for="ann-resolve-note">
                <textarea
                  id="ann-resolve-note"
                  v-model="resolveNote"
                  class="textarea textarea-bordered w-full text-xs"
                  rows="2"
                  placeholder="改了哪里 / 为什么不改（建议填写）"
                />
              </AppField>
              <div class="flex justify-end gap-2">
                <button type="button" class="btn btn-ghost btn-xs" @click="resolveId = null">取消</button>
                <button
                  type="button"
                  class="btn btn-primary btn-xs"
                  @click="emit('resolve', ann.id, resolveOutcome, resolveNote.trim()); resolveId = null"
                >
                  确认结案
                </button>
              </div>
            </div>

            <div v-else class="mt-0.5 flex justify-end gap-1">
              <template v-if="isAuthor(ann)">
                <button type="button" class="btn btn-ghost btn-xs" @click="startEdit(ann)">编辑</button>
                <button type="button" class="btn btn-ghost btn-xs text-error" @click="emit('remove', ann.id)">
                  删除
                </button>
              </template>
              <button type="button" class="btn btn-ghost btn-xs" @click="startResolve(ann.id)">结案</button>
            </div>

            <!-- 编辑态 -->
            <div v-if="editingId === ann.id" class="mt-1 flex flex-col gap-1.5 border-t border-base-300 pt-2">
              <textarea
                v-model="editingBody"
                class="textarea textarea-bordered w-full text-sm"
                rows="3"
              />
              <div class="flex justify-end gap-2">
                <button type="button" class="btn btn-ghost btn-xs" @click="editingId = null">取消</button>
                <button
                  type="button"
                  class="btn btn-primary btn-xs"
                  :disabled="!editingBody.trim()"
                  @click="submitEdit"
                >
                  保存
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 已处理：默认折叠但可见（选项 B 可核查兜底），可重新打开 -->
      <details v-if="resolvedList().length > 0" class="collapse collapse-arrow bg-base-200/40">
        <summary class="collapse-title text-sm">
          已处理
          <span class="badge badge-ghost badge-sm ml-1">{{ resolvedList().length }}</span>
        </summary>
        <div class="collapse-content flex flex-col gap-2">
          <div v-for="ann in resolvedList()" :key="ann.id" :ref="el => setItemRef(ann.id, el)" class="card bg-base-100 card-compact">
            <div class="card-body gap-1.5">
              <blockquote
                v-if="anchorQuote(ann)"
                class="border-l-2 border-base-content/20 pl-2 text-xs text-base-content/60 line-clamp-2"
              >
                {{ anchorQuote(ann) }}
              </blockquote>
              <p class="text-sm whitespace-pre-wrap text-base-content/70">{{ ann.body }}</p>
              <div class="flex flex-wrap items-center gap-1.5">
                <span
                  class="badge badge-sm"
                  :class="ann.outcome === 'dismissed' ? 'badge-ghost' : 'badge-success'"
                >
                  {{ outcomeLabel[ann.outcome ?? ""] ?? "已处理" }}
                </span>
                <span class="text-[11px] text-base-content/50">
                  {{ ann.createdBy }} · {{ timeLabel(ann.createdAt) }}
                </span>
              </div>
              <p v-if="ann.resolvedNote" class="rounded-box bg-base-200/60 px-2 py-1 text-xs text-base-content/70">
                依据：{{ ann.resolvedNote }}
              </p>
              <div class="flex justify-end">
                <button type="button" class="btn btn-ghost btn-xs" @click="emit('reopen', ann.id)">
                  重新打开
                </button>
              </div>
            </div>
          </div>
        </div>
      </details>
    </template>
  </div>
</template>

<style scoped>
.ann-flash {
  animation: ann-flash 1.2s ease;
}
@keyframes ann-flash {
  0% {
    background-color: color-mix(in oklab, var(--color-primary) 25%, transparent);
  }
  100% {
    background-color: transparent;
  }
}
</style>
