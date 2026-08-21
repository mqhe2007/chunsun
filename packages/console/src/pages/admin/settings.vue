<script setup lang="ts">
import SystemSettingsView from "@/screens/admin/SystemSettingsView.vue";

definePage({
  meta: { title: "平台设置", requiresAdmin: true },
});
</script>

<template>
  <SystemSettingsView />
</template>
