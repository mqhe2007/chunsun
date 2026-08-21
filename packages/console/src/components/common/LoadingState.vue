<template>
  <div class="loading-container">
    <span
      class="loading loading-spinner"
      :style="{ width: spinnerSize, height: spinnerSize }"
      aria-hidden="true"
    />
    <span class="loading-text">{{ text }}</span>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  text?: string
  spinnerSize?: string
}>(), {
  text: '加载中...',
  spinnerSize: '2rem'
})
</script>

<style scoped>
.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 60px;
  color: color-mix(in oklab, var(--color-base-content) 60%, transparent);
}

.loading-text {
  font-size: 13px;
}
</style>
