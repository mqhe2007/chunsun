<script setup lang="ts">
import { Pencil, Trash2 } from "@lucide/vue";
import { onUnmounted, ref, watch } from "vue";
import CopyableValue from "@/components/common/CopyableValue.vue";
import UserAvatar from "@/components/common/UserAvatar.vue";
import { api } from "@/utils/api";
import { useToast } from "@/ui";
import {
  appendUniqueById,
  BOARD_COLUMNS,
  BOARD_PAGE_SIZE,
  BOARD_STATUSES,
  buildColumnQuery,
  parseListPageMeta,
  shouldLoadNextPage,
  type BoardStatus,
} from "./requirementBoard";

export type BoardRequirement = {
  id: string;
  description: string;
  status: string;
  origin?: string;
  ownerId?: string | null;
  owner?: {
    id: string;
    nickname: string | null;
    qq: string | null;
  } | null;
  updatedAt: string;
};

type ColumnState = {
  items: BoardRequirement[];
  page: number;
  total: number;
  loading: boolean;
  loadingMore: boolean;
};

const props = defineProps<{
  projectId: string;
  idFilter: string;
  ownerFilter: string;
  reloadToken: number;
  blockedIds?: Set<string>;
}>();

const emit = defineEmits<{
  openDetail: [row: BoardRequirement];
  edit: [row: BoardRequirement];
  delete: [row: BoardRequirement];
}>();

const toast = useToast();

function emptyColumns(): Record<BoardStatus, ColumnState> {
  return {
    pending: { items: [], page: 0, total: 0, loading: false, loadingMore: false },
    running: { items: [], page: 0, total: 0, loading: false, loadingMore: false },
    completed: { items: [], page: 0, total: 0, loading: false, loadingMore: false },
    abandoned: { items: [], page: 0, total: 0, loading: false, loadingMore: false },
  };
}

const columns = ref<Record<BoardStatus, ColumnState>>(emptyColumns());
const sentinels: Partial<Record<BoardStatus, HTMLElement | null>> = {};
const observers: Partial<Record<BoardStatus, IntersectionObserver>> = {};
const requestSeq: Record<BoardStatus, number> = {
  pending: 0,
  running: 0,
  completed: 0,
  abandoned: 0,
};

function ownerLabel(row: BoardRequirement): string {
  return row.owner?.nickname || "—";
}

function canMutate(row: BoardRequirement): boolean {
  return row.status === "pending";
}

function statusDotClass(status: BoardStatus) {
  if (status === "running") {
    return "status-info shadow-[0_0_0.55rem_color-mix(in_oklab,var(--color-info)_80%,transparent)]";
  }
  if (status === "completed") {
    return "status-success shadow-[0_0_0.55rem_color-mix(in_oklab,var(--color-success)_80%,transparent)]";
  }
  if (status === "abandoned") {
    return "status-warning shadow-[0_0_0.55rem_color-mix(in_oklab,var(--color-warning)_70%,transparent)]";
  }
  return "status-neutral shadow-[0_0_0.45rem_color-mix(in_oklab,var(--color-base-content)_40%,transparent)]";
}

function bindColumnRoot(status: BoardStatus, el: Element | null) {
  observers[status]?.disconnect();
  delete observers[status];
  if (!(el instanceof HTMLElement)) return;
  const io = new IntersectionObserver(
    entries => {
      if (!entries.some(entry => entry.isIntersecting)) return;
      const col = columns.value[status];
      void fetchColumn(status, col.page + 1, true);
    },
    { root: el, rootMargin: "120px 0px", threshold: 0 },
  );
  observers[status] = io;
  const sentinel = sentinels[status];
  if (sentinel) io.observe(sentinel);
}

function setSentinel(status: BoardStatus, el: Element | null) {
  const prev = sentinels[status];
  if (prev && observers[status]) observers[status]!.unobserve(prev);
  sentinels[status] = el instanceof HTMLElement ? el : null;
  if (sentinels[status] && observers[status]) observers[status]!.observe(sentinels[status]!);
}

async function fetchColumn(status: BoardStatus, page: number, append: boolean) {
  const col = columns.value[status];
  if (append) {
    if (col.loadingMore || col.loading) return;
    if (
      !shouldLoadNextPage({
        loaded: col.items.length,
        total: col.total,
        page: col.page,
        pageSize: BOARD_PAGE_SIZE,
      })
    ) {
      return;
    }
    col.loadingMore = true;
  } else {
    col.loading = true;
  }

  const seq = ++requestSeq[status];
  try {
    const qs = buildColumnQuery({
      status,
      id: props.idFilter,
      ownerId: props.ownerFilter,
      page,
      pageSize: BOARD_PAGE_SIZE,
    });
    const { data } = await api.get<{
      success: boolean;
      data: BoardRequirement[];
      meta?: { page: number; pageSize: number; total: number };
    }>(`/projects/${props.projectId}/requirements${qs}`);
    if (seq !== requestSeq[status]) return;
    if (!data.success) return;
    const meta = parseListPageMeta(data);
    const items = data.data ?? [];
    const next = columns.value[status];
    next.page = meta?.page ?? page;
    next.total = meta?.total ?? items.length;
    next.items = append ? appendUniqueById(next.items, items) : items;
  } catch {
    if (seq === requestSeq[status] && !append) {
      toast.error("获取失败", "加载看板列失败");
    }
  } finally {
    if (seq === requestSeq[status]) {
      columns.value[status].loading = false;
      columns.value[status].loadingMore = false;
    }
  }
}

function reloadAll() {
  for (const status of BOARD_STATUSES) {
    columns.value[status] = {
      items: [],
      page: 0,
      total: 0,
      loading: true,
      loadingMore: false,
    };
    void fetchColumn(status, 1, false);
  }
}

watch(
  () => [props.projectId, props.reloadToken] as const,
  () => {
    if (props.projectId) reloadAll();
  },
  { immediate: true },
);

onUnmounted(() => {
  for (const status of BOARD_STATUSES) {
    observers[status]?.disconnect();
    delete observers[status];
  }
});
</script>

<template>
  <div class="req-board" role="list">
    <section
      v-for="col in BOARD_COLUMNS"
      :key="col.status"
      class="req-board-col"
      :data-status="col.status"
      role="listitem"
    >
      <header class="req-board-head">
        <h2>
          <span class="status status-sm" :class="statusDotClass(col.status)" aria-hidden="true" />
          {{ col.label }}
        </h2>
        <span class="badge badge-ghost badge-sm">{{ columns[col.status].total }}</span>
      </header>
      <div
        class="req-board-cards"
        :aria-busy="columns[col.status].loading"
        :ref="el => bindColumnRoot(col.status, el as Element | null)"
      >
        <div v-if="columns[col.status].loading && columns[col.status].items.length === 0" class="req-board-empty">
          <span class="loading loading-spinner loading-sm" />
        </div>
        <template v-else-if="columns[col.status].items.length === 0">
          <p class="req-board-empty">暂无需求</p>
        </template>
        <button
          v-for="row in columns[col.status].items"
          :key="row.id"
          type="button"
          class="card card-sm bg-base-100 req-card"
          draggable="false"
          :aria-grabbed="false"
          @click="emit('openDetail', row)"
          @dragstart.prevent
        >
          <div class="card-body gap-2 p-3">
            <div class="flex items-start justify-between gap-2">
              <CopyableValue
                :value="row.id"
                label="复制 ID"
                value-class="req-id"
                stop-propagation
              />
              <span v-if="row.origin === 'defect'" class="badge badge-warning badge-sm shrink-0">
                来自缺陷
              </span>
              <span
                v-if="props.blockedIds?.has(row.id)"
                class="badge badge-error badge-sm shrink-0"
                title="被其他节点阻塞"
              >
                被阻塞
              </span>
            </div>
            <p class="req-card-desc" :title="row.description">{{ row.description }}</p>
            <div class="flex items-center gap-2 text-xs text-base-content/60">
              <span v-if="row.owner" class="inline-flex min-w-0 items-center gap-1">
                <UserAvatar :qq="row.owner.qq" :size="18" />
                <span class="truncate">{{ ownerLabel(row) }}</span>
              </span>
              <span v-else>—</span>
              <span class="ms-auto shrink-0">
                {{ new Date(row.updatedAt).toLocaleDateString() }}
              </span>
            </div>
            <div v-if="canMutate(row)" class="flex justify-end gap-1" @click.stop>
              <button
                type="button"
                class="btn btn-ghost btn-square btn-xs"
                aria-label="编辑"
                title="编辑"
                @click="emit('edit', row)"
              >
                <Pencil :size="14" aria-hidden="true" />
              </button>
              <button
                type="button"
                class="btn btn-ghost btn-square btn-xs btn-error"
                aria-label="删除"
                title="删除"
                @click="emit('delete', row)"
              >
                <Trash2 :size="14" aria-hidden="true" />
              </button>
            </div>
          </div>
        </button>
        <div
          :ref="el => setSentinel(col.status, (el as HTMLElement | null))"
          class="req-board-sentinel"
          :data-status="col.status"
        />
        <div v-if="columns[col.status].loadingMore" class="req-board-empty">
          <span class="loading loading-spinner loading-xs" />
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.req-board {
  display: grid;
  grid-template-columns: repeat(4, minmax(16rem, 1fr));
  flex: 1 1 auto;
  gap: 0.75rem;
  min-block-size: 0;
  overflow-x: auto;
  overflow-y: hidden;
}

.req-board-col {
  display: flex;
  min-inline-size: 16rem;
  min-block-size: 0;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid color-mix(in oklab, var(--color-base-content) 10%, transparent);
  border-radius: 0.75rem;
  background: color-mix(in oklab, var(--color-base-200) 70%, var(--color-base-100));
}

.req-board-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  padding: 0.7rem 0.85rem 0.45rem;
}

.req-board-head h2 {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  margin: 0;
  font-size: 0.9rem;
  font-weight: 650;
}

.req-board-cards {
  display: flex;
  min-block-size: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: 0.55rem;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 0.35rem 0.65rem 0.75rem;
}

.req-card {
  inline-size: 100%;
  cursor: pointer;
  text-align: start;
  box-shadow: none;
}

.req-card:hover {
  background: var(--color-base-100);
  outline: 1px solid color-mix(in oklab, var(--color-base-content) 12%, transparent);
}

.req-card-desc {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  line-clamp: 2;
  overflow: clip;
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.45;
}

.req-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.75rem;
}

.req-board-empty {
  display: grid;
  min-block-size: 6rem;
  place-items: center;
  margin: 0;
  color: color-mix(in oklab, var(--color-base-content) 50%, transparent);
  font-size: 0.85rem;
}

.req-board-sentinel {
  block-size: 1px;
}
</style>
