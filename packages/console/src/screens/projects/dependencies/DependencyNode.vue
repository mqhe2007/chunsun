<script setup lang="ts">
import { computed } from "vue";
import { Handle, Position, type NodeProps } from "@vue-flow/core";
import { AlertTriangle, Bug, ClipboardList } from "@lucide/vue";

export type NodeData = {
  id: string;
  kind: "requirement" | "defect";
  label: string;
  status: string;
  isBlocked: boolean;
  isHighlighted: boolean;
  isDimmed: boolean;
  severity?: string;
};

const props = defineProps<NodeProps<NodeData>>();

const statusColor = computed(() => {
  const s = props.data.status;
  if (props.data.kind === "requirement") {
    if (s === "completed") return "#15803d";
    if (s === "running") return "#2563eb";
    if (s === "abandoned") return "#6b7280";
    return "#a3ab96";
  }
  if (s === "resolved" || s === "closed") return "#15803d";
  if (s === "processing") return "#2563eb";
  return "#d97706";
});

const nodeClass = computed(() => {
  const classes = ["dep-node"];
  if (props.data.isBlocked) classes.push("dep-node--blocked");
  if (props.data.isHighlighted) classes.push("dep-node--highlighted");
  if (props.data.isDimmed) classes.push("dep-node--dimmed");
  if (props.selected) classes.push("dep-node--selected");
  return classes.join(" ");
});
</script>

<template>
  <div
    :class="nodeClass"
    :title="`${data.kind === 'requirement' ? '需求' : '缺陷'}: ${data.id}\n状态: ${data.status}\n${data.isBlocked ? '⚠ 被阻塞' : ''}`"
  >
    <Handle type="target" :position="Position.Top" class="dep-handle" />
    <div class="dep-node-inner">
      <div class="dep-node-header">
        <span class="dep-node-kind">
          <Bug v-if="data.kind === 'defect'" :size="12" />
          <ClipboardList v-else :size="12" />
        </span>
        <span class="dep-node-id">{{ data.id }}</span>
        <AlertTriangle v-if="data.isBlocked" :size="12" class="dep-node-blocked-icon" />
      </div>
      <div class="dep-node-label" :title="data.label">{{ data.label }}</div>
      <div class="dep-node-footer">
        <span class="dep-node-status-dot" :style="{ background: statusColor }" />
        <span class="dep-node-status">{{ data.status }}</span>
      </div>
    </div>
    <Handle type="source" :position="Position.Bottom" class="dep-handle" />
  </div>
</template>

<style scoped>
.dep-node {
  min-width: 160px;
  max-width: 200px;
  border-radius: 10px;
  background: var(--color-base-100);
  border: 2px solid var(--color-base-300);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  transition: box-shadow 0.15s, border-color 0.15s, opacity 0.15s;
  font-size: 0.8rem;
}

.dep-node--blocked {
  border-color: #dc2626;
  box-shadow: 0 0 0 2px rgba(220, 38, 38, 0.15);
}

.dep-node--highlighted {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.2);
}

.dep-node--selected {
  border-color: #2563eb;
  box-shadow: 0 0 0 3px rgba(37, 99, 235, 0.25);
}

.dep-node--dimmed {
  opacity: 0.3;
}

.dep-node-inner {
  padding: 8px 10px;
}

.dep-node-header {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-bottom: 4px;
}

.dep-node-kind {
  display: inline-flex;
  color: var(--color-base-content);
  opacity: 0.6;
}

.dep-node-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.7rem;
  color: var(--color-base-content);
  opacity: 0.5;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dep-node-blocked-icon {
  color: #dc2626;
  flex-shrink: 0;
}

.dep-node-label {
  font-weight: 500;
  color: var(--color-base-content);
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  line-height: 1.3;
  min-height: 2.08em;
}

.dep-node-footer {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-top: 6px;
}

.dep-node-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dep-node-status {
  font-size: 0.7rem;
  text-transform: capitalize;
  color: var(--color-base-content);
  opacity: 0.6;
}

.dep-handle {
  width: 8px;
  height: 8px;
  background: var(--color-base-300);
  border: 2px solid var(--color-base-100);
}

:deep(.vue-flow__handle) {
  width: 8px;
  height: 8px;
}
</style>
