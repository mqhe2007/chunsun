<script setup lang="ts">
/**
 * 春笋品牌标：使用 logo-mark.svg（根目录 logo.svg 的矢量笋形）。
 * 直接以 <img> 渲染，保留原图色彩与透明背景。
 */
import { computed } from "vue";
import logoMarkUrl from "./assets/logo-mark.svg";

const props = withDefaults(
  defineProps<{
    size?: string | number;
    label?: string;
  }>(),
  {
    size: "1.5rem",
    label: "春笋",
  },
);

const sizeCss = computed(() => {
  if (typeof props.size === "number") return `${props.size}px`;
  const raw = props.size.trim();
  return /^\d+(\.\d+)?$/.test(raw) ? `${raw}px` : raw;
});
</script>

<template>
  <img
    class="brand-mark"
    :src="logoMarkUrl"
    :alt="label"
    :style="{ height: sizeCss }"
  />
</template>

<style scoped>
.brand-mark {
  display: block;
  flex-shrink: 0;
  width: auto;
  object-fit: contain;
}
</style>
