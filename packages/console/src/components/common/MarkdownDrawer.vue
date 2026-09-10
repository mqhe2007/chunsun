<script setup lang="ts">
import { computed } from "vue";
import { AppDrawer } from "@/ui";
import { renderMarkdown } from "@/utils/markdown";

const props = withDefaults(
  defineProps<{
    /** 抽屉标题 */
    title?: string;
    /** markdown 原文；空内容显示占位 */
    content?: string | null;
  }>(),
  { title: "内容查看", content: "" },
);

const open = defineModel<boolean>({ default: false });

const rendered = computed(() => renderMarkdown(props.content ?? ""));
</script>

<template>
  <AppDrawer
    v-model="open"
    :title="title"
    width-class="w-full max-w-[50vw]!"
  >
    <div
      v-if="rendered"
      class="markdown-body"
      v-html="rendered"
    />
    <p v-else class="text-sm text-base-content/60">
      内容为空。
    </p>
  </AppDrawer>
</template>
