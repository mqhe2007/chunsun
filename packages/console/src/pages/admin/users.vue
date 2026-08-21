<script setup lang="ts">
import UserManagementView from "@/screens/admin/UserManagementView.vue";

definePage({
  meta: { title: "用户管理", requiresAdmin: true },
});
</script>

<template>
  <UserManagementView />
</template>
