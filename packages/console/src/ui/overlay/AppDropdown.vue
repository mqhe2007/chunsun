<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";

export type DropdownItem = {
  label: string;
  disabled?: boolean;
  separator?: boolean;
  command?: () => void;
};

const open = defineModel<boolean>("open", { default: false });

defineProps<{
  items: DropdownItem[];
}>();

const root = ref<HTMLElement | null>(null);

function toggle() {
  open.value = !open.value;
}

function close() {
  open.value = false;
}

function onItemClick(item: DropdownItem) {
  if (item.disabled || item.separator) return;
  item.command?.();
  close();
}

function onClickOutside(event: MouseEvent) {
  if (!open.value) return;
  const el = root.value;
  if (el && !el.contains(event.target as Node)) {
    close();
  }
}

onMounted(() => document.addEventListener("click", onClickOutside));
onUnmounted(() => document.removeEventListener("click", onClickOutside));

defineExpose({ toggle, close });
</script>

<template>
  <div
    ref="root"
    class="dropdown"
    :class="{ 'dropdown-open': open }"
  >
    <div class="w-full">
      <slot name="trigger" :toggle="toggle" :open="open" />
    </div>
    <div
      class="dropdown-content rounded-box z-50 w-full min-w-52 border-0 bg-base-100 p-2 shadow-lg"
      role="menu"
      @click.stop
    >
      <slot name="header" />
      <ul class="menu w-full p-0">
        <template v-for="(item, index) in items" :key="index">
          <li v-if="item.separator" />
          <li v-else>
            <button
              type="button"
              role="menuitem"
              :disabled="item.disabled"
              :class="{ 'menu-disabled': item.disabled }"
              @click="onItemClick(item)"
            >
              {{ item.label }}
            </button>
          </li>
        </template>
      </ul>
      <slot name="footer" />
    </div>
  </div>
</template>
