<script setup lang="ts">
import { siteToastStore } from "@/composables/useSiteToast";
</script>

<template>
  <div class="site-toast" aria-live="polite">
    <div v-for="item in siteToastStore.items" :key="item.id" class="site-toast-item">
      {{ item.title }}
    </div>
  </div>
</template>

<style scoped>
.site-toast {
  position: fixed;
  top: 5.5rem;
  left: 50%;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  transform: translateX(-50%);
  pointer-events: none;
}

.site-toast-item {
  min-width: 12rem;
  padding: 0.65rem 1rem;
  border-radius: 0.5rem;
  background: var(--chunsun-ink);
  color: var(--chunsun-fog);
  font-size: 0.875rem;
  font-weight: 500;
  box-shadow: 0 8px 24px color-mix(in srgb, var(--chunsun-ink) 18%, transparent);
  white-space: nowrap;
}
</style>
