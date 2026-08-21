<template>
  <ConsoleShell drawer-id="console-drawer" :include-system-admin="true">
    <template #nav="{ closeDrawer }">
      <ul class="menu menu-md w-full p-0">
        <li>
          <RouterLink
            to="/projects"
            :class="{ 'menu-active': route.path.startsWith('/projects') }"
            @click="closeDrawer"
          >
            <Folder class="h-4 w-4" aria-hidden="true" />
            项目管理
          </RouterLink>
        </li>
        <li>
          <RouterLink
            to="/notifications"
            :class="{ 'menu-active': route.path.startsWith('/notifications') }"
            @click="closeDrawer"
          >
            <Bell class="h-4 w-4" aria-hidden="true" />
            通知中心
            <span v-if="unreadCount > 0" class="badge badge-error">
              {{ unreadCount > 99 ? "99+" : unreadCount }}
            </span>
          </RouterLink>
        </li>
      </ul>
    </template>
    <slot />
  </ConsoleShell>
</template>

<script setup lang="ts">
import { Bell, Folder } from "@lucide/vue";
import { useRoute } from "vue-router";
import ConsoleShell from "@/layouts/ConsoleShell.vue";
import { useUnreadNotifications } from "@/composables/useUnreadNotifications";

const route = useRoute();
const { unreadCount } = useUnreadNotifications();
</script>
