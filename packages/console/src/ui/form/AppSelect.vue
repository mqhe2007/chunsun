<script setup lang="ts">
type OptionValue = string | number;

withDefaults(
  defineProps<{
    options: { label: string; value: OptionValue }[];
    id?: string;
    disabled?: boolean;
    /** 首项为空值，用于「全部」筛选 */
    clearable?: boolean;
    placeholder?: string;
  }>(),
  { placeholder: "全部", clearable: false },
);

/** 空字符串表示未选择（全部） */
const model = defineModel<OptionValue | "">({ default: "" });
</script>

<template>
  <select
    :id="id"
    v-model="model"
    class="select w-full"
    :disabled="disabled"
  >
    <option v-if="clearable" value="">{{ placeholder }}</option>
    <option v-for="opt in options" :key="String(opt.value)" :value="opt.value">
      {{ opt.label }}
    </option>
  </select>
</template>
