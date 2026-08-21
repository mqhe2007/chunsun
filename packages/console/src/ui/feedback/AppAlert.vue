<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    severity?: "info" | "success" | "warning" | "error";
  }>(),
  { severity: "info" },
);

const alertClass = computed(() => {
  const map = {
    info: "alert-info",
    success: "alert-success",
    warning: "alert-warning",
    error: "alert-error",
  } as const;
  return map[props.severity];
});
</script>

<template>
  <div class="alert text-sm" :class="alertClass" role="alert">
    <slot />
  </div>
</template>
