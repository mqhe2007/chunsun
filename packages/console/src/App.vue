<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { ConfirmHost, ToastHost } from "@/ui";
import ConsoleLayout from "@/layouts/ConsoleLayout.vue";
import AdminLayout from "@/layouts/AdminLayout.vue";

const route = useRoute();
const isAuth = computed(() => route.path.startsWith("/auth"));
const isSetup = computed(() => route.path === "/setup" || route.path.startsWith("/setup/"));
const isAdminArea = computed(() => route.path.startsWith("/admin"));
</script>

<template>
  <template v-if="isAuth || isSetup">
    <router-view />
  </template>
  <div v-else class="app-ui min-h-screen" data-theme="chunsun">
    <AdminLayout v-if="isAdminArea">
      <router-view />
    </AdminLayout>
    <ConsoleLayout v-else>
      <router-view />
    </ConsoleLayout>
  </div>
  <div class="app-ui" data-theme="chunsun">
    <ToastHost />
    <ConfirmHost />
  </div>
</template>
