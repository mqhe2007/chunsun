<script setup lang="ts">
import { Bell, Lock, User } from "@lucide/vue";
import { computed } from "vue";
import { useRoute } from "vue-router";
import ProjectMegaNav, { type MegaNavItem } from "@/components/projects/ProjectMegaNav.vue";

const route = useRoute();

const sectionOptions: MegaNavItem[] = [
  { key: "profile", label: "基本资料", icon: User, to: "/settings/profile" },
  { key: "password", label: "登录密码", icon: Lock, to: "/settings/password" },
  { key: "notifications", label: "消息通知", icon: Bell, to: "/settings/notifications" },
];

const activeKey = computed(() => {
  const path = route.path;
  if (path.includes("/settings/password")) return "password";
  if (path.includes("/settings/notifications")) return "notifications";
  return "profile";
});
</script>

<template>
  <ProjectMegaNav
    class="mb-4"
    :items="sectionOptions"
    :active-key="activeKey"
    label="账户设置菜单"
  />
</template>
