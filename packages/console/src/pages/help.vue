<script setup lang="ts">
/**
 * 使用帮助已迁移至营销侧「文档」中心（/docs）。
 * 保留本路由为重定向，避免旧链接/书签死链。
 * 优先由 router/index.ts 守卫层重定向（不渲染本组件），此处为组件级兜底。
 */
import { onMounted } from "vue";

onMounted(() => {
  window.location.replace("/docs");
});
</script>

<template>
  <div />
</template>
