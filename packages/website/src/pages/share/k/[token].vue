<script setup lang="ts">
import ShareKnowledgeView from "@/screens/share/ShareKnowledgeView.vue";
import SiteLayout from "@/layouts/SiteLayout.vue";

definePage({
  meta: { title: "公开文档" },
});
</script>

<template>
  <SiteLayout>
    <ShareKnowledgeView />
  </SiteLayout>
</template>
