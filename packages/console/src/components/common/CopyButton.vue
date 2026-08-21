<script setup lang="ts">
import { Copy } from "@lucide/vue";
import { useToast } from "@/ui";

const props = withDefaults(
  defineProps<{
    value: string;
    /** 无障碍 / tooltip 文案 */
    label?: string;
    /** 在可点击行内时阻止冒泡 */
    stopPropagation?: boolean;
  }>(),
  {
    label: "复制",
    stopPropagation: false,
  },
);

const toast = useToast();

async function onClick(event: MouseEvent) {
  if (props.stopPropagation) event.stopPropagation();
  if (!props.value) return;
  try {
    await navigator.clipboard.writeText(props.value);
    toast.add({
      severity: "success",
      summary: "已复制",
      detail: props.value,
      life: 1500,
    });
  } catch {
    toast.add({
      severity: "warn",
      summary: "复制失败",
      life: 2000,
    });
  }
}
</script>

<template>
  <button
    type="button"
    class="btn btn-ghost btn-sm copy-btn"
    :aria-label="label"
    :title="label"
    @click="onClick"
  >
    <Copy :size="14" />
  </button>
</template>

<style scoped>
.copy-btn {
  flex-shrink: 0;
}
</style>
