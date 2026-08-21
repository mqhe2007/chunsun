<script setup lang="ts">
import { ChevronLeft } from "@lucide/vue";

withDefaults(
  defineProps<{
    title: string;
    back?: { to: string; label: string };
    /** 撑满父级剩余高度，内部自行滚动 */
    fill?: boolean;
  }>(),
  { fill: false },
);
</script>

<template>
  <div
    class="flex min-w-0 flex-col gap-5"
    :class="fill ? 'h-full min-h-0 overflow-hidden' : ''"
  >
    <div class="flex shrink-0 flex-wrap items-center justify-between gap-3">
      <div class="flex min-w-0 items-center gap-1">
        <RouterLink
          v-if="back"
          :to="back.to"
          class="btn btn-ghost btn-square shrink-0"
          :aria-label="back.label"
          :title="back.label"
        >
          <ChevronLeft :size="20" aria-hidden="true" />
        </RouterLink>
        <div class="flex min-w-0 flex-wrap items-center gap-3">
          <h1 class="text-xl font-semibold tracking-tight">{{ title }}</h1>
          <slot name="title-extra" />
        </div>
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 flex-wrap items-center gap-2">
        <slot name="actions" />
      </div>
    </div>
    <div :class="fill ? 'flex min-h-0 flex-1 flex-col gap-5 overflow-hidden' : 'contents'">
      <slot />
    </div>
  </div>
</template>
