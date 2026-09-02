<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { Bug, ChartNoAxesColumn, ClipboardList, GitBranch, Settings } from "@lucide/vue";
import ProjectMegaNav, { type MegaNavItem } from "@/components/projects/ProjectMegaNav.vue";

const props = defineProps<{
  projectId: string;
}>();

const route = useRoute();

const items = computed<MegaNavItem[]>(() => {
  const base = `/projects/${props.projectId}`;
  return [
    { key: "overview", label: "总览", icon: ChartNoAxesColumn, to: `${base}/overview` },
    { key: "requirements", label: "需求", icon: ClipboardList, to: `${base}/requirements` },
    { key: "defects", label: "缺陷", icon: Bug, to: `${base}/defects` },
    { key: "dependencies", label: "依赖图", icon: GitBranch, to: `${base}/dependencies` },
    { key: "settings", label: "设置", icon: Settings, to: `${base}/settings` },
  ];
});

const activeKey = computed(() => {
  const path = route.path;
  if (path.includes("/settings")) return "settings";
  if (path.includes("/requirements")) return "requirements";
  if (path.includes("/defects")) return "defects";
  if (path.includes("/dependencies")) return "dependencies";
  return "overview";
});
</script>

<template>
  <ProjectMegaNav :items="items" :active-key="activeKey" label="项目一级菜单" />
</template>
