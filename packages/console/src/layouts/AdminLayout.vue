<template>
  <ConsoleShell drawer-id="admin-drawer" :include-system-admin="false">
    <template #nav="{ closeDrawer }">
      <ul class="menu menu-md w-full p-0">
        <li>
          <RouterLink to="/projects" @click="closeDrawer">
            <ArrowLeft class="h-4 w-4" aria-hidden="true" />
            返回控制台
          </RouterLink>
        </li>
        <li></li>
        <li v-for="item in adminNavItems" :key="item.url">
          <RouterLink
            :to="item.url"
            :class="{ 'menu-active': item.match(route.path) }"
            @click="closeDrawer"
          >
            <component :is="item.icon" class="h-4 w-4" aria-hidden="true" />
            {{ item.label }}
          </RouterLink>
        </li>
      </ul>
    </template>
    <slot />
  </ConsoleShell>
</template>

<script setup lang="ts">
import { ArrowLeft, Settings, Ticket, Users } from "@lucide/vue";
import { useRoute } from "vue-router";
import ConsoleShell from "@/layouts/ConsoleShell.vue";

const route = useRoute();

const adminNavItems = [
  { label: "用户管理", icon: Users, url: "/admin/users", match: (p: string) => p.startsWith("/admin/users") },
  { label: "平台设置", icon: Settings, url: "/admin/settings", match: (p: string) => p.startsWith("/admin/settings") },
  { label: "邀请码", icon: Ticket, url: "/admin/invitations", match: (p: string) => p.startsWith("/admin/invitations") },
];
</script>
