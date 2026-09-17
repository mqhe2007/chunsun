<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Check, Eye, Pencil, RotateCcw, Trash2 } from "@lucide/vue";
import { AppField, AppModal } from "@/ui";
import type { KnowledgeAnnotation } from "@/composables/useAnnotations";

/**
 * 批注面板（需求 u-WPvdvYh4Fw，方案甲：右侧第三栏常驻）。
 *
 * 权限到按钮级：编辑 / 删除仅作者（后端 403 兜底）；结案 / 重新打开所有成员可用
 * （选项 B：AI 可结案，人可重开兜底）。面板本身不做权限判断以外的数据操作，
 * 全部动作 emit 给宿主（宿主持有 useAnnotations 状态，桌面栏与移动抽屉两个
 * 实例共享同一数据源）。
 *
 * 新建批注走「正文选区工具条 → 模态框」（宿主渲染），面板只列已存在的批注，
 * 列表按紧凑卡片排版：第一行是批注正文标题，第二行是状态、时间与操作。
 */

const props = defineProps<{
  annotations: KnowledgeAnnotation[];
  loading: boolean;
  currentUserId: string | null;
  /** 点击正文高亮时设置的批注 id，联动滚动到这里。 */
  activeId: string | null;
}>();

const emit = defineEmits<{
  edit: [id: string, body: string];
  resolve: [id: string, outcome: "addressed" | "dismissed", note: string];
  reopen: [id: string];
  remove: [id: string];
  "scroll-to-anchor": [id: string];
}>();

const editingId = ref<string | null>(null);
const editingBody = ref("");

const detailAnnotation = ref<KnowledgeAnnotation | null>(null);
const detailOpen = computed({
  get: () => detailAnnotation.value !== null,
  set: (open: boolean) => {
    if (!open) detailAnnotation.value = null;
  },
});

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

/** 紧凑列表里的短时间：同年只显示 月/日 时:分。 */
function shortTime(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const sameYear = d.getFullYear() === now.getFullYear();
  return d.toLocaleString(undefined, {
    ...(sameYear ? {} : { year: "numeric" }),
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

const outcomeLabel: Record<string, string> = {
  addressed: "已按批注处理",
  dismissed: "判断无需修改",
};

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

function confirmResolve(id: string) {
  emit("resolve", id, resolveOutcome.value, resolveNote.value.trim());
  resolveId.value = null;
}

function anchorQuote(ann: KnowledgeAnnotation): string {
  return (ann.anchorText ?? "").trim();
}

function openDetails(ann: KnowledgeAnnotation) {
  detailAnnotation.value = ann;
}

const statusLabel: Record<KnowledgeAnnotation["status"], string> = {
  open: "待处理",
  stale: "待处理",
  resolved: "已处理",
};

function statusClass(status: KnowledgeAnnotation["status"]): string {
  if (status === "resolved") return "badge-success";
  return "badge-primary";
}

function locateDetails() {
  const id = detailAnnotation.value?.id;
  if (!id) return;
  detailAnnotation.value = null;
  emit("scroll-to-anchor", id);
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3">
    <p class="text-[11px] leading-4 text-base-content/50">
      批注仅作用于本文档；跨需求的一次性方向请写入需求工作记忆，不要留在批注里。
    </p>

    <div v-if="loading" class="flex justify-center py-6">
      <span class="loading loading-spinner" />
    </div>

    <template v-else>
      <!-- 未处理：open + stale；stale 的锚点失效说明只在详情中展示。 -->
      <div class="flex flex-col gap-1.5">
        <h4 class="text-sm font-medium">
          未处理
          <span class="badge badge-primary badge-sm ml-1">{{ openList().length }}</span>
        </h4>
        <p v-if="openList().length === 0" class="text-xs text-base-content/50">
          暂无未处理批注。选中正文后点击浮出的「批注」即可添加。
        </p>
        <div
          v-for="ann in openList()"
          :key="ann.id"
          :ref="el => setItemRef(ann.id, el)"
          class="card bg-base-200/40 scroll-mt-4"
        >
          <div class="card-body gap-1.5 p-2.5">
            <div class="min-w-0">
              <button
                type="button"
                class="block w-full truncate text-left text-[13px] font-medium leading-snug disabled:cursor-default disabled:opacity-100"
                :class="anchorQuote(ann) ? 'cursor-pointer transition-colors hover:text-primary' : ''"
                :disabled="!anchorQuote(ann)"
                :title="ann.body"
                @click.stop="emit('scroll-to-anchor', ann.id)"
              >
                {{ ann.body }}
              </button>
            </div>
            <div class="flex min-w-0 items-center gap-1.5">
              <span class="badge badge-xs shrink-0" :class="statusClass(ann.status)">
                {{ statusLabel[ann.status] }}
              </span>
              <span
                class="min-w-0 flex-1 truncate text-[11px] text-base-content/50"
                :title="timeLabel(ann.createdAt)"
              >
                {{ shortTime(ann.createdAt) }}
              </span>
              <div class="flex shrink-0 items-center gap-0.5">
                <button
                  type="button"
                  class="btn btn-ghost btn-square btn-xs"
                  aria-label="查看批注详情"
                  title="查看详情"
                  @click.stop="openDetails(ann)"
                >
                  <Eye :size="13" aria-hidden="true" />
                </button>
                <template v-if="isAuthor(ann)">
                  <button
                    type="button"
                    class="btn btn-ghost btn-square btn-xs"
                    aria-label="编辑批注"
                    title="编辑"
                    @click.stop="startEdit(ann)"
                  >
                    <Pencil :size="13" aria-hidden="true" />
                  </button>
                  <button
                    type="button"
                    class="btn btn-ghost btn-square btn-xs text-error"
                    aria-label="删除批注"
                    title="删除"
                    @click.stop="emit('remove', ann.id)"
                  >
                    <Trash2 :size="13" aria-hidden="true" />
                  </button>
                </template>
                <button
                  type="button"
                  class="btn btn-ghost btn-square btn-xs"
                  aria-label="结案"
                  title="结案"
                  @click.stop="startResolve(ann.id)"
                >
                  <Check :size="13" aria-hidden="true" />
                </button>
              </div>
            </div>

            <!-- 结案表单（选项 B：须给依据，人可对照正文核查） -->
            <div v-if="resolveId === ann.id" class="mt-1 flex flex-col gap-1.5 border-t border-base-300 pt-2">
              <div class="flex flex-wrap gap-2">
                <label class="label cursor-pointer gap-1 p-0 text-xs">
                  <input
                    v-model="resolveOutcome"
                    type="radio"
                    name="ann-outcome"
                    value="addressed"
                    class="radio radio-primary radio-xs"
                  />
                  已按批注处理
                </label>
                <label class="label cursor-pointer gap-1 p-0 text-xs">
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
                <button type="button" class="btn btn-primary btn-xs" @click="confirmResolve(ann.id)">
                  确认结案
                </button>
              </div>
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
        <div class="collapse-content flex flex-col gap-1.5">
          <div
            v-for="ann in resolvedList()"
            :key="ann.id"
            :ref="el => setItemRef(ann.id, el)"
            class="card bg-base-100"
          >
            <div class="card-body gap-1.5 p-2.5">
              <div class="min-w-0">
                <button
                  type="button"
                  class="block w-full truncate text-left text-[13px] font-medium leading-snug text-base-content/70 disabled:cursor-default disabled:opacity-100"
                  :class="anchorQuote(ann) ? 'cursor-pointer transition-colors hover:text-primary' : ''"
                  :disabled="!anchorQuote(ann)"
                  :title="ann.body"
                  @click.stop="emit('scroll-to-anchor', ann.id)"
                >
                  {{ ann.body }}
                </button>
              </div>
              <div class="flex min-w-0 items-center gap-1.5">
                <span class="badge badge-xs shrink-0" :class="statusClass(ann.status)">
                  {{ statusLabel[ann.status] }}
                </span>
                <span
                  class="min-w-0 flex-1 truncate text-[11px] text-base-content/50"
                  :title="timeLabel(ann.createdAt)"
                >
                  {{ shortTime(ann.createdAt) }}
                </span>
                <button
                  type="button"
                  class="btn btn-ghost btn-square btn-xs shrink-0"
                  aria-label="查看批注详情"
                  title="查看详情"
                  @click.stop="openDetails(ann)"
                >
                  <Eye :size="13" aria-hidden="true" />
                </button>
                <button
                  type="button"
                  class="btn btn-ghost btn-square btn-xs shrink-0"
                  aria-label="重新打开"
                  title="重新打开"
                  @click.stop="emit('reopen', ann.id)"
                >
                  <RotateCcw :size="13" aria-hidden="true" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </details>
    </template>
  </div>

  <AppModal
    v-model="detailOpen"
    :title="detailAnnotation ? '批注详情' : undefined"
    width-class="max-w-2xl"
  >
    <div v-if="detailAnnotation" class="flex flex-col gap-4 text-sm">
      <div class="flex flex-wrap items-center gap-2">
        <span class="badge" :class="statusClass(detailAnnotation.status)">
          {{ statusLabel[detailAnnotation.status] }}
        </span>
        <span
          v-if="detailAnnotation.status === 'resolved' && detailAnnotation.outcome"
          class="badge badge-ghost"
        >
          {{ outcomeLabel[detailAnnotation.outcome] ?? "已处理" }}
        </span>
        <span class="text-xs text-base-content/50">
          创建于 {{ timeLabel(detailAnnotation.createdAt) }}
        </span>
      </div>

      <div
        v-if="detailAnnotation.status === 'stale'"
        class="alert alert-warning py-2 text-xs"
      >
        文档内容更新后，系统已无法定位这条批注引用的原文。批注仍保留为待处理，可查看内容后编辑、结案或删除。
      </div>

      <div>
        <div class="mb-1 text-xs font-medium text-base-content/50">批注内容</div>
        <p class="whitespace-pre-wrap break-words leading-relaxed">{{ detailAnnotation.body }}</p>
      </div>

      <div v-if="anchorQuote(detailAnnotation)">
        <div class="mb-1 text-xs font-medium text-base-content/50">引用原文</div>
        <blockquote class="max-h-48 overflow-y-auto whitespace-pre-wrap break-words border-l-2 border-primary/60 pl-3 leading-relaxed text-base-content/70">
          {{ anchorQuote(detailAnnotation) }}
        </blockquote>
      </div>
      <p v-else class="text-xs text-base-content/50">针对整篇文档的批注</p>

      <div v-if="detailAnnotation.status === 'resolved' && detailAnnotation.resolvedNote">
        <div class="mb-1 text-xs font-medium text-base-content/50">处理依据</div>
        <p class="whitespace-pre-wrap break-words rounded-box bg-base-200/60 px-3 py-2 leading-relaxed">
          {{ detailAnnotation.resolvedNote }}
        </p>
      </div>
      <div v-if="detailAnnotation.resolvedAt" class="text-xs text-base-content/50">
        处理于 {{ timeLabel(detailAnnotation.resolvedAt) }}
      </div>
    </div>
    <template #footer>
      <button
        v-if="detailAnnotation?.anchorText?.trim()"
        type="button"
        class="btn btn-ghost"
        @click="locateDetails"
      >
        定位原文
      </button>
      <button type="button" class="btn btn-primary" @click="detailAnnotation = null">关闭</button>
    </template>
  </AppModal>
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
