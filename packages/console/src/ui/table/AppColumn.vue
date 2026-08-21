<script setup lang="ts">
import { inject, onMounted, useSlots, type Slot } from "vue";
import type { TableColumn } from "./AppTable.vue";

const props = defineProps<{
  header: string;
  width?: string;
}>();

const slots = useSlots();
const register = inject<(col: Omit<TableColumn, "id">) => void>("app-table-register");

onMounted(() => {
  register?.({
    header: props.header,
    width: props.width,
    body: slots.default as Slot<{ row: unknown }> | undefined,
  });
});
</script>

<template />
