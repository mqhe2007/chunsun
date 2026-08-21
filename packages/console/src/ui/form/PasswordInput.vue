<script setup lang="ts">
import { Eye, EyeOff } from "@lucide/vue";
import { ref } from "vue";

const model = defineModel<string>({ default: "" });

withDefaults(
  defineProps<{
    id?: string;
    placeholder?: string;
    invalid?: boolean;
    autocomplete?: string;
    toggleLabel?: boolean;
  }>(),
  { toggleLabel: true },
);

const show = ref(false);
</script>

<template>
  <div class="input w-full" :class="{ 'input-error': invalid }">
    <input
      :id="id"
      v-model="model"
      :type="show ? 'text' : 'password'"
      class="grow min-w-0"
      :placeholder="placeholder"
      :autocomplete="autocomplete"
    />
    <button
      v-if="toggleLabel"
      type="button"
      class="btn btn-ghost btn-square btn-xs"
      :aria-label="show ? '隐藏密码' : '显示密码'"
      :aria-pressed="show"
      :title="show ? '隐藏密码' : '显示密码'"
      @click="show = !show"
    >
      <EyeOff v-if="show" :size="16" aria-hidden="true" />
      <Eye v-else :size="16" aria-hidden="true" />
    </button>
  </div>
</template>
