<script setup lang="ts">
import { ChevronLeft } from "@lucide/vue";

withDefaults(
  defineProps<{
    label?: string;
  }>(),
  {
    label: "返回",
  },
);

defineEmits<{
  click: [event: MouseEvent];
}>();
</script>

<template>
  <button type="button" class="btn btn-ghost -ml-2 gap-1" @click="$emit('click', $event)">
    <ChevronLeft :size="16" aria-hidden="true" />
    <span>{{ label }}</span>
  </button>
</template>
