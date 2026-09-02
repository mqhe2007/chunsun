<script setup lang="ts">
import DependencyGraphView from "@/screens/projects/dependencies/DependencyGraphView.vue";

definePage({
  meta: { title: "依赖图" },
});
</script>

<template>
  <DependencyGraphView />
</template>
