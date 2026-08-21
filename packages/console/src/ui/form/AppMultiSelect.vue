<script setup lang="ts">
import { computed } from "vue";

type OptionValue = string | number;

const props = withDefaults(
  defineProps<{
    options: { label: string; value: OptionValue }[];
    placeholder?: string;
    disabled?: boolean;
  }>(),
  { placeholder: "全部" },
);

const model = defineModel<OptionValue[]>({ default: () => [] });

const summary = computed(() => {
  if (model.value.length === 0) return props.placeholder;
  if (model.value.length === 1) {
    const opt = props.options.find(o => o.value === model.value[0]);
    return opt?.label ?? String(model.value[0]);
  }
  return `${model.value.length} 项已选`;
});

function isChecked(value: OptionValue) {
  return model.value.includes(value);
}

function toggle(value: OptionValue, checked: boolean) {
  if (checked) {
    if (!model.value.includes(value)) {
      model.value = [...model.value, value];
    }
  } else {
    model.value = model.value.filter(v => v !== value);
  }
}

function clear() {
  model.value = [];
}
</script>

<template>
  <details class="dropdown w-44">
    <summary
      class="select w-full cursor-pointer list-none font-normal"
      :class="{ 'pointer-events-none opacity-50': disabled }"
    >
      <span class="truncate">{{ summary }}</span>
    </summary>
    <ul class="dropdown-content menu w-full border-0 bg-base-100 rounded-box z-20 p-2 shadow-lg max-h-64 overflow-y-auto">
      <li v-if="model.length > 0">
        <button type="button" class="text-sm" @click="clear">清除筛选</button>
      </li>
      <li v-for="opt in options" :key="String(opt.value)">
        <label class="fieldset-label cursor-pointer justify-start gap-2 py-1.5">
          <input
            type="checkbox"
            class="checkbox"
            :checked="isChecked(opt.value)"
            :disabled="disabled"
            @change="toggle(opt.value, ($event.target as HTMLInputElement).checked)"
          />
          {{ opt.label }}
        </label>
      </li>
    </ul>
  </details>
</template>
