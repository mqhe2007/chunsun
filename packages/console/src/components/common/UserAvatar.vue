<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    qq?: string | null;
    size?: number | string;
    rounded?: boolean;
  }>(),
  { size: 32, rounded: true },
);

const sizePx = computed(() =>
  typeof props.size === "number" ? `${props.size}px` : props.size,
);

const qqSizeCode = computed(() => {
  const px = typeof props.size === "number" ? props.size : 32;
  if (px <= 40) return 1;
  if (px <= 100) return 3;
  return 4;
});

const avatarUrl = computed(() => {
  const qq = (props.qq ?? "").trim();
  if (!qq) return "";
  return `https://q.qlogo.cn/g?b=qq&nk=${encodeURIComponent(qq)}&s=${qqSizeCode.value}`;
});

const hasAvatar = computed(() => Boolean(avatarUrl.value));
</script>

<template>
  <div
    class="avatar"
    :class="{ 'avatar-placeholder': !hasAvatar }"
    role="img"
    aria-label="用户头像"
  >
    <div
      class="bg-base-200 text-base-content/40"
      :class="rounded ? 'rounded-full' : 'rounded-md'"
      :style="{ width: sizePx, height: sizePx }"
    >
      <img v-if="hasAvatar" :src="avatarUrl" alt="" loading="lazy" />
      <svg
        v-else
        viewBox="0 0 24 24"
        width="55%"
        height="55%"
        aria-hidden="true"
      >
        <circle cx="12" cy="8" r="4" fill="currentColor" />
        <path d="M4 20c0-4 3.6-6 8-6s8 2 8 6" fill="currentColor" />
      </svg>
    </div>
  </div>
</template>
