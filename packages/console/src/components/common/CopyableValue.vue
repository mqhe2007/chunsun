<script setup lang="ts">
import CopyButton from "@/components/common/CopyButton.vue";
withDefaults(
  defineProps<{
    value: string;
    label?: string;
    stopPropagation?: boolean;
    /** 追加到 `<code>` 的 class */
    valueClass?: string;
  }>(),
  {
    label: "复制",
    stopPropagation: false,
    valueClass: "",
  },
);
</script>

<template>
  <span class="copyable-value">
    <code :class="valueClass">{{ value }}</code>
    <CopyButton
      :value="value"
      :label="label"
      :stop-propagation="stopPropagation"
    />
  </span>
</template>

<style scoped>
.copyable-value {
  display: inline-flex;
  align-items: center;
  gap: 0.1rem;
  max-width: 100%;
  min-width: 0;
}

.copyable-value code {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background: transparent;
  padding: 0;
}
</style>
