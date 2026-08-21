<script setup lang="ts">
import { nextTick, onMounted, ref, watch } from "vue";
import { computeAutoHeight } from "./autoHeightTextarea";

const model = defineModel<string>({ default: "" });

const props = withDefaults(
  defineProps<{
    placeholder?: string;
    rows?: number;
    minHeightPx?: number;
    maxHeightPx?: number;
    disabled?: boolean;
  }>(),
  {
    rows: 3,
    minHeightPx: 72,
  },
);

const el = ref<HTMLTextAreaElement | null>(null);

function syncHeight() {
  const node = el.value;
  if (!node) return;
  node.style.height = "auto";
  node.style.height = computeAutoHeight(node.scrollHeight, props.minHeightPx, props.maxHeightPx);
}

watch(model, () => {
  void nextTick(syncHeight);
});

onMounted(() => {
  syncHeight();
});
</script>

<template>
  <textarea
    ref="el"
    v-model="model"
    class="textarea w-full field-sizing-content resize-none overflow-hidden"
    :rows="rows"
    :placeholder="placeholder"
    :disabled="disabled"
    :style="{ minHeight: `${minHeightPx}px`, maxHeight: maxHeightPx ? `${maxHeightPx}px` : undefined }"
    @input="syncHeight"
  />
</template>
