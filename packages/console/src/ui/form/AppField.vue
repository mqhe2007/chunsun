<script setup lang="ts">
defineProps<{
  label: string;
  hint?: string;
  error?: string;
  htmlFor?: string;
}>();
</script>

<template>
  <fieldset class="fieldset w-full gap-3 text-sm">
    <label v-if="label" class="fieldset-legend mb-0! py-0" :for="htmlFor">{{ label }}</label>
    <slot />
    <p v-if="error" class="label text-error whitespace-normal">{{ error }}</p>
    <p v-else-if="hint" class="label whitespace-normal">{{ hint }}</p>
  </fieldset>
</template>
