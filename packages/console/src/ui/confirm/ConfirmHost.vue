<script setup lang="ts">
import { acceptConfirm, rejectConfirm, useConfirmState } from "./useConfirm";

const state = useConfirmState();
</script>

<template>
  <dialog class="modal" :class="{ 'modal-open': !!state.pending }">
    <div class="modal-box max-w-md">
      <h3 class="text-lg font-bold">{{ state.pending?.title }}</h3>
      <p class="py-4">{{ state.pending?.message }}</p>
      <div class="modal-action">
        <button type="button" class="btn btn-ghost" @click="rejectConfirm">
          {{ state.pending?.cancelLabel ?? "取消" }}
        </button>
        <button
          type="button"
          class="btn"
          :class="state.pending?.danger ? 'btn-error' : 'btn-primary'"
          @click="acceptConfirm"
        >
          {{ state.pending?.confirmLabel ?? "确定" }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button type="button" @click="rejectConfirm">close</button>
    </form>
  </dialog>
</template>
