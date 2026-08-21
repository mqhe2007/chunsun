<script setup lang="ts">
import { computed, provide, ref, watch, type Slot } from "vue";

export type TableColumn = {
  id: number;
  header: string;
  width?: string;
  body?: Slot<{ row: unknown }>;
};

const props = withDefaults(
  defineProps<{
    rows: unknown[];
    loading?: boolean;
    empty?: string;
    striped?: boolean;
    rowHover?: boolean;
    /** 客户端分页：对 rows 切片 */
    paginator?: boolean;
    rowsPerPage?: number;
    /** 服务端分页：rows 为当前页，total 为总条数 */
    total?: number;
    pageSizeOptions?: number[];
  }>(),
  {
    loading: false,
    empty: "暂无数据",
    striped: false,
    rowHover: false,
    paginator: false,
    rowsPerPage: 20,
    pageSizeOptions: () => [10, 20, 50],
  },
);

const page = defineModel<number>("page", { default: 1 });
const pageSize = defineModel<number>("pageSize", { default: 20 });

defineEmits<{ rowClick: [row: unknown] }>();

const columns = ref<TableColumn[]>([]);
let columnSeq = 0;

provide("app-table-register", (col: Omit<TableColumn, "id">) => {
  const id = ++columnSeq;
  columns.value.push({ id, ...col });
});

const isServerPaginator = computed(() => props.total !== undefined);

const displayRows = computed(() => {
  if (isServerPaginator.value || !props.paginator) return props.rows;
  const start = (page.value - 1) * pageSize.value;
  return props.rows.slice(start, start + pageSize.value);
});

const totalPages = computed(() => {
  if (isServerPaginator.value) {
    return Math.max(1, Math.ceil((props.total ?? 0) / pageSize.value));
  }
  if (props.paginator) {
    return Math.max(1, Math.ceil(props.rows.length / pageSize.value));
  }
  return 1;
});

const showPaginator = computed(() => {
  if (props.loading) return false;
  if (isServerPaginator.value) return (props.total ?? 0) > pageSize.value;
  return props.paginator && props.rows.length > pageSize.value;
});

watch(pageSize, () => {
  if (page.value > totalPages.value) {
    page.value = totalPages.value;
  }
});

function rowClass(index: number) {
  return props.striped && index % 2 === 1 ? "bg-base-200/60" : "";
}

function goPrev() {
  page.value = Math.max(1, page.value - 1);
}

function goNext() {
  page.value = Math.min(totalPages.value, page.value + 1);
}
</script>

<template>
  <div class="w-full min-w-0 rounded-box bg-base-100">
    <div v-if="loading" class="flex justify-center py-16">
      <span class="loading loading-spinner loading-lg text-primary" />
    </div>
    <table v-else class="table table-fixed w-full">
      <thead>
        <tr>
          <th
            v-for="col in columns"
            :key="col.id"
            class="min-w-0 overflow-hidden"
            :style="col.width ? { width: col.width } : { width: '100%' }"
          >
            {{ col.header }}
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="displayRows.length === 0">
          <td :colspan="Math.max(columns.length, 1)" class="text-center text-base-content/60 py-10">
            {{ empty }}
          </td>
        </tr>
        <tr
          v-for="(row, index) in displayRows"
          :key="index"
          :class="[rowClass(index), rowHover ? 'hover cursor-pointer' : '']"
          @click="rowHover ? $emit('rowClick', row) : undefined"
        >
          <td
            v-for="col in columns"
            :key="col.id"
            class="min-w-0 overflow-hidden"
            :style="col.width ? { width: col.width } : { width: '100%' }"
          >
            <component
              :is="{ render: () => (col.body ? col.body({ row }) : null) }"
              v-if="col.body"
            />
          </td>
        </tr>
      </tbody>
    </table>
  </div>

  <div v-if="showPaginator" class="mt-3 flex flex-wrap items-center justify-between gap-2">
    <label v-if="isServerPaginator" class="flex items-center gap-2 text-sm text-base-content/70">
      每页
      <select v-model.number="pageSize" class="select">
        <option v-for="n in pageSizeOptions" :key="n" :value="n">{{ n }}</option>
      </select>
      条
    </label>
    <div class="join">
      <button type="button" class="join-item btn" :disabled="page <= 1" @click="goPrev">
        上一页
      </button>
      <button type="button" class="join-item btn pointer-events-none">
        {{ page }} / {{ totalPages }}
      </button>
      <button
        type="button"
        class="join-item btn"
        :disabled="page >= totalPages"
        @click="goNext"
      >
        下一页
      </button>
    </div>
  </div>

  <slot />
</template>
