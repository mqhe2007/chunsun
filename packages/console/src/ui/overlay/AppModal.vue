<script setup lang="ts">
import { ref, watch } from "vue";

const open = defineModel<boolean>({ default: false });

withDefaults(
  defineProps<{
    title?: string;
    widthClass?: string;
  }>(),
  { widthClass: "max-w-lg" },
);

const dialogEl = ref<HTMLDialogElement | null>(null);

watch(open, value => {
  const el = dialogEl.value;
  if (!el) return;
  if (value) {
    if (!el.open) el.showModal();
  } else if (el.open) {
    el.close();
  }
});

function onClose() {
  open.value = false;
}
</script>

<template>
  <dialog ref="dialogEl" class="modal" @close="onClose">
    <div class="modal-box" :class="widthClass">
      <h3 v-if="title" class="text-lg font-bold mb-4">{{ title }}</h3>
      <slot />
      <div v-if="$slots.footer" class="modal-action">
        <slot name="footer" />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button type="submit">close</button>
    </form>
  </dialog>
</template>
