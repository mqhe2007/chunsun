<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { BookOpen, Key, Settings, SlidersHorizontal, Users } from "@lucide/vue";
import ProjectMegaNav, { type MegaNavItem } from "@/components/projects/ProjectMegaNav.vue";

const props = defineProps<{
  projectId: string;
}>();

const route = useRoute();

const items = computed<MegaNavItem[]>(() => {
  const base = `/projects/${props.projectId}/settings`;
  return [
    { key: "", label: "知识设置", icon: BookOpen, to: base },
    { key: "general", label: "通用设置", icon: Settings, to: `${base}/general` },
    { key: "members", label: "成员管理", icon: Users, to: `${base}/members` },
    { key: "secret-key", label: "项目密钥", icon: Key, to: `${base}/secret-key` },
    { key: "env-vars", label: "环境变量", icon: SlidersHorizontal, to: `${base}/env-vars` },
  ];
});

const activeKey = computed(() => {
  const path = route.path;
  const hit = items.value.find(item => {
    if (!item.key) {
      const clean = path.replace(/\/$/, "");
      return clean.endsWith("/settings");
    }
    return path.includes(`/settings/${item.key}`);
  });
  return hit?.key ?? "";
});
</script>

<template>
  <ProjectMegaNav :items="items" :active-key="activeKey" label="项目设置菜单" />
</template>
