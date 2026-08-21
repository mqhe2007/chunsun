<script setup lang="ts">
import type { LucideIcon } from "@lucide/vue";

defineProps<{
  title: string;
  line: string;
  icon: LucideIcon;
}>();
</script>

<template>
  <article class="feature-capsule">
    <span class="feature-capsule-icon" aria-hidden="true">
      <component :is="icon" :size="18" />
    </span>
    <span class="feature-capsule-copy">
      <span class="feature-capsule-title">{{ title }}</span>
      <span class="feature-capsule-line">{{ line }}</span>
    </span>
  </article>
</template>

<style scoped>
.feature-capsule {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 0.7rem;
  max-inline-size: 22rem;
  padding: 0.65rem 1rem 0.65rem 0.7rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--chunsun-fog) 88%, white);
  border: 1px solid color-mix(in srgb, var(--chunsun-shoot) 16%, transparent);
  box-shadow: 0 1px 0 color-mix(in srgb, var(--chunsun-shoot) 8%, transparent);
}

.feature-capsule-icon {
  flex: none;
  display: inline-grid;
  place-items: center;
  inline-size: 2.1rem;
  block-size: 2.1rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--chunsun-shoot) 14%, transparent);
  color: var(--chunsun-shoot);
}

.feature-capsule-copy {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-inline-size: 0;
}

.feature-capsule-title {
  font-size: 0.92rem;
  font-weight: 700;
  letter-spacing: -0.01em;
  line-height: 1.25;
  color: var(--chunsun-ink);
  white-space: nowrap;
}

.feature-capsule-line {
  font-size: 0.78rem;
  line-height: 1.35;
  color: var(--chunsun-ink-muted);
  white-space: nowrap;
}
</style>
