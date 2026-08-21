<script setup lang="ts">
import { toastStore } from "./useToast";

const alertClass: Record<string, string> = {
  success: "alert-success",
  error: "alert-error",
  warn: "alert-warning",
  info: "alert-info",
};
</script>

<template>
  <div class="toast toast-top toast-center z-[9999]">
    <div
      v-for="item in toastStore.items"
      :key="item.id"
      role="alert"
      class="alert alert-horizontal shadow-lg"
      :class="alertClass[item.severity] ?? 'alert-info'"
    >
      <span class="whitespace-nowrap">{{ item.title }}</span>
    </div>
  </div>
</template>
