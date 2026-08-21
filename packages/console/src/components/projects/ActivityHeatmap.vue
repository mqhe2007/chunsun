<script setup lang="ts">
import { computed } from "vue";

type Entry = { date: string; count: number };

const props = defineProps<{
  entries: Entry[];
  max: number;
}>();

/** 周一为周首（0=Mon ... 6=Sun） */
function weekdayMonFirst(isoDate: string): number {
  const d = new Date(`${isoDate}T00:00:00Z`);
  return (d.getUTCDay() + 6) % 7;
}

function monthOf(isoDate: string): number {
  return new Date(`${isoDate}T00:00:00Z`).getUTCMonth();
}

/** 非零日按四分位数分级：0 → L0；>0 按 P25/P50/P75 划分 L1-L4 */
const thresholds = computed(() => {
  const counts = props.entries
    .map(e => e.count)
    .filter(c => c > 0)
    .sort((a, b) => a - b);
  if (counts.length === 0) return { q1: 0, q2: 0, q3: 0 };
  const at = (p: number) => {
    const idx = Math.min(counts.length - 1, Math.floor(p * counts.length));
    return counts[idx]!;
  };
  return { q1: at(0.25), q2: at(0.5), q3: at(0.75) };
});

function colorClass(count: number): string {
  if (count <= 0) return "hm-l0";
  const { q1, q2, q3 } = thresholds.value;
  if (count <= q1) return "hm-l1";
  if (count <= q2) return "hm-l2";
  if (count <= q3) return "hm-l3";
  return "hm-l4";
}

/** 每个 cell 在 CSS Grid 中的行列位置 */
const cells = computed(() => {
  const result: { entry: Entry; row: number; col: number }[] = [];
  let weekIndex = 0;
  let currentWeekDay = 0;

  for (const e of props.entries) {
    const wd = weekdayMonFirst(e.date);
    if (currentWeekDay > 0 && wd === 0) {
      weekIndex += 1;
    }
    result.push({ entry: e, row: wd + 1, col: weekIndex + 1 });
    currentWeekDay = wd;
  }
  return result;
});

const colCount = computed(() => {
  if (cells.value.length === 0) return 0;
  return Math.max(...cells.value.map(c => c.col));
});

/** 顶部月份标签：取每个月第一次出现的列 */
const monthLabels = computed(() => {
  const labels: { col: number; label: string }[] = [];
  let lastMonth = -1;
  for (const c of cells.value) {
    const m = monthOf(c.entry.date);
    if (m !== lastMonth) {
      labels.push({ col: c.col, label: `${m + 1}` });
      lastMonth = m;
    }
  }
  return labels;
});

function cellTitle(entry: Entry): string {
  return `${entry.date} · ${entry.count} 条活动`;
}
</script>

<template>
  <div class="hm-root">
    <div class="hm-months" :style="{ gridTemplateColumns: `repeat(${colCount}, 1fr)` }">
      <span
        v-for="m in monthLabels"
        :key="m.col"
        class="hm-month-label"
        :style="{ gridColumn: m.col }"
      >
        {{ m.label }}
      </span>
    </div>

    <div
      class="hm-grid"
      :style="{
        gridTemplateColumns: `repeat(${colCount}, 1fr)`,
        gridTemplateRows: 'repeat(7, 1fr)',
      }"
    >
      <div
        v-for="c in cells"
        :key="`${c.entry.date}`"
        class="hm-cell"
        :class="colorClass(c.entry.count)"
        :style="{ gridRow: c.row, gridColumn: c.col }"
        :title="cellTitle(c.entry)"
      />
    </div>

    <div class="hm-legend">
      <span class="hm-legend-label">少</span>
      <div class="hm-legend-cell hm-l0" />
      <div class="hm-legend-cell hm-l1" />
      <div class="hm-legend-cell hm-l2" />
      <div class="hm-legend-cell hm-l3" />
      <div class="hm-legend-cell hm-l4" />
      <span class="hm-legend-label">多</span>
    </div>
  </div>
</template>

<style scoped>
.hm-root {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  width: 100%;
}

.hm-months {
  display: grid;
  gap: 2px;
  height: 0.9rem;
}

.hm-month-label {
  font-size: 0.62rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: clip;
}

.hm-grid {
  display: grid;
  gap: 2px;
  width: 100%;
  aspect-ratio: 53 / 7;
  min-height: 120px;
}

.hm-cell {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  border-radius: 2px;
}

.hm-l0 {
  background: var(--color-base-200);
}

.hm-l1 {
  background: color-mix(in oklch, var(--color-primary) 28%, var(--color-base-100));
}

.hm-l2 {
  background: color-mix(in oklch, var(--color-primary) 48%, var(--color-base-100));
}

.hm-l3 {
  background: color-mix(in oklch, var(--color-primary) 68%, var(--color-base-100));
}

.hm-l4 {
  background: color-mix(in oklch, var(--color-primary) 90%, var(--color-base-100));
}

.hm-legend {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.25rem;
  margin-top: 0.25rem;
}

.hm-legend-label {
  font-size: 0.62rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.hm-legend-cell {
  width: 10px;
  height: 10px;
  border-radius: 2px;
}
</style>
