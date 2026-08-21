<script setup lang="ts">
import { ref, watch } from "vue";

const open = defineModel<boolean>({ default: false });

withDefaults(
  defineProps<{
    title?: string;
    /** Tailwind max-width / width classes for the panel */
    widthClass?: string;
  }>(),
  { widthClass: "w-full max-w-md" },
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
  <dialog ref="dialogEl" class="modal modal-end" @close="onClose">
    <div
      class="modal-box flex h-full max-h-none flex-col rounded-none p-0"
      :class="widthClass"
    >
      <h3
        v-if="title"
        class="border-base-200 shrink-0 border-b px-5 py-4 text-lg font-bold"
      >
        {{ title }}
      </h3>
      <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        <slot />
      </div>
      <div
        v-if="$slots.footer"
        class="border-base-200 modal-action mt-0 shrink-0 justify-end border-t px-5 py-3"
      >
        <slot name="footer" />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button type="submit">close</button>
    </form>
  </dialog>
</template>
