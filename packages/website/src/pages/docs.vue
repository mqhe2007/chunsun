<script setup lang="ts">
import SiteLayout from "@/layouts/SiteLayout.vue";
import DocsView from "@/screens/docs/DocsView.vue";

definePage({
  meta: { title: "文档" },
});
</script>

<template>
  <SiteLayout>
    <DocsView />
  </SiteLayout>
</template>
