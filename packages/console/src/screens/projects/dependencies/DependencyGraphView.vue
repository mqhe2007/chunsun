<script setup lang="ts">
import {
  Download,
  Filter,
  Maximize2,
  RefreshCw,
  Search,
  ZoomIn,
  ZoomOut,
} from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  VueFlow,
  useVueFlow,
  type Edge,
  type Node,
} from "@vue-flow/core";
import { Background, BackgroundVariant } from "@vue-flow/background";
import { Controls } from "@vue-flow/controls";
import { MiniMap } from "@vue-flow/minimap";
import "@vue-flow/core/dist/style.css";
import "@vue-flow/core/dist/theme-default.css";
import dagre from "dagre";
import { toPng, toSvg } from "html-to-image";
import { AppPage, useToast } from "@/ui";
import { api } from "@/utils/api";
import {
  DEFECT_STATUS_LABEL,
  REQUIREMENT_STATUS_LABEL,
} from "@/utils/workflow";
import DependencyNode, { type NodeData } from "./DependencyNode.vue";

const route = useRoute();
const router = useRouter();
const toast = useToast();

// ── 数据类型 ──────────────────────────────────────────────
type RequirementRow = {
  id: string;
  description: string;
  status: string;
  origin?: string;
  createdAt: string;
  updatedAt: string;
};

type DefectRow = {
  id: string;
  description?: string | null;
  status: string;
  severity: string;
  updatedAt: string;
};

type DependencyEdge = {
  id: string;
  sourceType: "requirement" | "defect";
  sourceId: string;
  targetType: "requirement" | "defect";
  targetId: string;
};

type GraphNode = {
  id: string;
  kind: "requirement" | "defect";
  label: string;
  status: string;
  severity?: string;
};

// ── 状态 ──────────────────────────────────────────────────
const loading = ref(false);
const requirements = ref<RequirementRow[]>([]);
const defects = ref<DefectRow[]>([]);
const edges = ref<DependencyEdge[]>([]);

const searchQuery = ref("");
const statusFilter = ref<string>("all");
const showOnlyBlocked = ref(false);
const selectedNodeId = ref<string | null>(null);
const highlightChain = ref<Set<string>>(new Set());

const { fitView, zoomIn, zoomOut, project } = useVueFlow();

// ── 计算属性 ──────────────────────────────────────────────
const projectId = () => (route.params as Record<string, string>).id;

const allNodes = computed<GraphNode[]>(() => {
  const reqNodes: GraphNode[] = requirements.value.map(r => ({
    id: r.id,
    kind: "requirement",
    label: r.description || r.id,
    status: r.status,
  }));
  const defectNodes: GraphNode[] = defects.value.map(d => ({
    id: d.id,
    kind: "defect",
    label: d.description || d.id,
    status: d.status,
    severity: d.severity,
  }));
  return [...reqNodes, ...defectNodes];
});

/** 被阻塞的节点 ID 集合：有未完成的前置依赖 */
const blockedNodeIds = computed<Set<string>>(() => {
  const blocked = new Set<string>();
  const completedStatuses = new Set(["completed", "resolved", "closed"]);
  const nodeMap = new Map(allNodes.value.map(n => [n.id, n]));

  for (const edge of edges.value) {
    // source → target：source 阻塞 target
    const source = nodeMap.get(edge.sourceId);
    if (source && !completedStatuses.has(source.status)) {
      blocked.add(edge.targetId);
    }
  }
  return blocked;
});

/** 过滤后的节点 */
const filteredNodeIds = computed<Set<string>>(() => {
  let nodes = allNodes.value;

  if (statusFilter.value !== "all") {
    nodes = nodes.filter(n => n.status === statusFilter.value);
  }
  if (showOnlyBlocked.value) {
    nodes = nodes.filter(n => blockedNodeIds.value.has(n.id));
  }
  if (searchQuery.value.trim()) {
    const q = searchQuery.value.trim().toLowerCase();
    nodes = nodes.filter(
      n => n.id.toLowerCase().includes(q) || n.label.toLowerCase().includes(q),
    );
  }
  return new Set(nodes.map(n => n.id));
});

/** 关键路径：最长依赖链 */
const criticalPath = computed<string[]>(() => {
  const adj = new Map<string, string[]>();
  for (const edge of edges.value) {
    if (!adj.has(edge.sourceId)) adj.set(edge.sourceId, []);
    adj.get(edge.sourceId)!.push(edge.targetId);
  }

  const memo = new Map<string, string[]>();
  const visited = new Set<string>();

  function dfs(nodeId: string): string[] {
    if (memo.has(nodeId)) return memo.get(nodeId)!;
    if (visited.has(nodeId)) return [nodeId];
    visited.add(nodeId);

    const next = adj.get(nodeId) || [];
    let longest: string[] = [nodeId];
    for (const n of next) {
      const path = dfs(n);
      if (path.length + 1 > longest.length) {
        longest = [nodeId, ...path];
      }
    }
    memo.set(nodeId, longest);
    return longest;
  }

  let longest: string[] = [];
  for (const node of allNodes.value) {
    const path = dfs(node.id);
    if (path.length > longest.length) longest = path;
  }
  return longest.length > 1 ? longest : [];
});

// ── Vue Flow 节点/边 ─────────────────────────────────────
const flowNodes = computed<Node<NodeData>[]>(() => {
  return allNodes.value
    .filter(n => filteredNodeIds.value.has(n.id))
    .map(n => {
      const isInChain = highlightChain.value.size > 0 && highlightChain.value.has(n.id);
      const isDimmed = highlightChain.value.size > 0 && !highlightChain.value.has(n.id);
      return {
        id: n.id,
        type: "dependency",
        data: {
          id: n.id,
          kind: n.kind,
          label: n.label,
          status: n.status,
          isBlocked: blockedNodeIds.value.has(n.id),
          isHighlighted: isInChain || criticalPath.value.includes(n.id),
          isDimmed,
          severity: n.severity,
        },
        position: { x: 0, y: 0 },
      };
    });
});

const flowEdges = computed<Edge[]>(() => {
  const visibleIds = filteredNodeIds.value;
  return edges.value
    .filter(e => visibleIds.has(e.sourceId) && visibleIds.has(e.targetId))
    .map(e => {
      const isInChain =
        highlightChain.value.size > 0 &&
        highlightChain.value.has(e.sourceId) &&
        highlightChain.value.has(e.targetId);
      const isCritical =
        criticalPath.value.includes(e.sourceId) &&
        criticalPath.value.includes(e.targetId);
      return {
        id: e.id,
        source: e.sourceId,
        target: e.targetId,
        type: "smoothstep",
        animated: isInChain,
        style: {
          stroke: isInChain ? "#2563eb" : isCritical ? "#d97706" : "var(--color-base-300)",
          strokeWidth: isInChain || isCritical ? 2.5 : 1.5,
        },
        labelBgStyle: { fill: "var(--color-base-100)" },
      };
    });
});

// ── 布局计算（dagre）─────────────────────────────────────
function applyLayout(nodes: Node[], edges: Edge[]) {
  const g = new dagre.graphlib.Graph();
  g.setGraph({ rankdir: "TB", nodesep: 40, ranksep: 60, marginx: 20, marginy: 20 });
  g.setDefaultEdgeLabel(() => ({}));

  for (const node of nodes) {
    g.setNode(node.id, { width: 180, height: 80 });
  }
  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  return nodes.map(node => {
    const pos = g.node(node.id);
    return {
      ...node,
      position: { x: pos.x - 90, y: pos.y - 40 },
    };
  });
}

const positionedNodes = ref<Node[]>([]);

watch(
  [flowNodes, flowEdges],
  () => {
    positionedNodes.value = applyLayout(
      JSON.parse(JSON.stringify(flowNodes.value)),
      flowEdges.value,
    );
  },
  { immediate: true, deep: true },
);

// ── 数据获取 ──────────────────────────────────────────────
async function fetchAll() {
  loading.value = true;
  try {
    const [reqRes, defectRes, depRes] = await Promise.all([
      api.get<{ success: boolean; data: RequirementRow[] }>(
        `/projects/${projectId()}/requirements`,
      ),
      api.get<{ success: boolean; data: DefectRow[] }>(`/projects/${projectId()}/defects`),
      api.get<{ success: boolean; data: DependencyEdge[] }>(
        `/projects/${projectId()}/dependencies`,
      ),
    ]);

    requirements.value = reqRes.data.success ? reqRes.data.data : [];
    defects.value = defectRes.data.success ? defectRes.data.data : [];
    edges.value = depRes.data.success ? depRes.data.data : [];
  } catch {
    toast.error("加载失败", "获取依赖图数据失败");
  } finally {
    loading.value = false;
  }
}

// ── 交互 ──────────────────────────────────────────────────
function onNodeClick(event: { node: Node }) {
  const nodeId = (event.node as { id: string }).id;
  const node = allNodes.value.find(n => n.id === nodeId);
  if (!node) return;

  selectedNodeId.value = nodeId;

  // 如果是被阻塞节点，高亮阻塞链路
  if (blockedNodeIds.value.has(nodeId)) {
    highlightChain.value = computeBlockChain(nodeId);
  } else {
    highlightChain.value = new Set();
  }
}

function onPaneClick() {
  selectedNodeId.value = null;
  highlightChain.value = new Set();
}

/** 计算从节点追溯到最上游未完成前置的完整链路 */
function computeBlockChain(nodeId: string): Set<string> {
  const chain = new Set<string>([nodeId]);
  const completedStatuses = new Set(["completed", "resolved", "closed"]);
  const nodeMap = new Map(allNodes.value.map(n => [n.id, n]));

  // 反向 BFS：从被阻塞节点向上追溯所有未完成前置
  const queue = [nodeId];
  while (queue.length > 0) {
    const current = queue.shift()!;
    for (const edge of edges.value) {
      if (edge.targetId === current) {
        const source = nodeMap.get(edge.sourceId);
        if (source && !completedStatuses.has(source.status)) {
          if (!chain.has(edge.sourceId)) {
            chain.add(edge.sourceId);
            queue.push(edge.sourceId);
          }
        }
      }
    }
  }
  return chain;
}

function goToDetail() {
  if (!selectedNodeId.value) return;
  const node = allNodes.value.find(n => n.id === selectedNodeId.value);
  if (!node) return;
  if (node.kind === "requirement") {
    router.push(`/projects/${projectId()}/requirements/${node.id}`);
  } else {
    router.push(`/projects/${projectId()}/defects`);
  }
}

// ── 导出 ──────────────────────────────────────────────────
async function exportPNG() {
  const vueFlowEl = document.querySelector(".vue-flow") as HTMLElement;
  if (!vueFlowEl) {
    toast.error("导出失败", "未找到图形容器");
    return;
  }

  try {
    const bgColor =
      getComputedStyle(document.documentElement).getPropertyValue("--color-base-100") ||
      "#ffffff";
    const dataUrl = await toPng(vueFlowEl, {
      backgroundColor: bgColor.trim(),
      pixelRatio: 2,
      cacheBust: true,
    });
    const link = document.createElement("a");
    link.download = `dependency-graph-${projectId()}.png`;
    link.href = dataUrl;
    link.click();
    toast.success("已导出", "PNG 图片已下载");
  } catch {
    toast.error("导出失败", "导出过程出错");
  }
}

async function exportSVG() {
  const vueFlowEl = document.querySelector(".vue-flow") as HTMLElement;
  if (!vueFlowEl) {
    toast.error("导出失败", "未找到图形容器");
    return;
  }

  try {
    const bgColor =
      getComputedStyle(document.documentElement).getPropertyValue("--color-base-100") ||
      "#ffffff";
    const dataUrl = await toSvg(vueFlowEl, {
      backgroundColor: bgColor.trim(),
      cacheBust: true,
    });
    const link = document.createElement("a");
    link.download = `dependency-graph-${projectId()}.svg`;
    link.href = dataUrl;
    link.click();
    toast.success("已导出", "SVG 图片已下载");
  } catch {
    toast.error("导出失败", "导出过程出错");
  }
}

// ── 筛选选项 ──────────────────────────────────────────────
const statusOptions = computed(() => {
  const reqStatuses = Object.entries(REQUIREMENT_STATUS_LABEL).map(([value, label]) => ({
    value,
    label: `需求: ${label}`,
  }));
  const defectStatuses = Object.entries(DEFECT_STATUS_LABEL).map(([value, label]) => ({
    value,
    label: `缺陷: ${label}`,
  }));
  return [{ value: "all", label: "全部状态" }, ...reqStatuses, ...defectStatuses];
});

const stats = computed(() => ({
  total: allNodes.value.length,
  requirements: requirements.value.length,
  defects: defects.value.length,
  blocked: blockedNodeIds.value.size,
  edges: edges.value.length,
}));

// ── 生命周期 ──────────────────────────────────────────────
onMounted(() => {
  void fetchAll();
});

watch(
  () => (route.params as Record<string, string>).id,
  () => {
    void fetchAll();
  },
);
</script>

<template>
  <AppPage title="依赖图" :fill="true">
    <template #actions>
      <button
        type="button"
        class="btn btn-ghost btn-sm"
        :disabled="loading"
        @click="fetchAll"
      >
        <RefreshCw :size="14" :class="{ 'animate-spin': loading }" />
        刷新
      </button>
      <div class="dropdown dropdown-end">
        <button type="button" class="btn btn-ghost btn-sm" tabindex="0">
          <Download :size="14" />
          导出
        </button>
        <ul class="dropdown-content menu bg-base-100 rounded-box z-50 w-36 p-1 shadow">
          <li><button type="button" @click="exportPNG">PNG 图片</button></li>
          <li><button type="button" @click="exportSVG">SVG 矢量</button></li>
        </ul>
      </div>
    </template>

    <!-- 工具栏 -->
    <div class="mb-3 flex flex-wrap items-center gap-2">
      <div class="relative">
        <Search :size="14" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-base-content/40" />
        <input
          v-model="searchQuery"
          type="text"
          class="input input-sm w-48 pl-8"
          placeholder="搜索 ID / 标题"
        />
      </div>

      <select v-model="statusFilter" class="select select-sm w-44">
        <option v-for="opt in statusOptions" :key="opt.value" :value="opt.value">
          {{ opt.label }}
        </option>
      </select>

      <label class="label cursor-pointer gap-2">
        <input v-model="showOnlyBlocked" type="checkbox" class="checkbox checkbox-sm" />
        <span class="label-text text-sm">只看被阻塞</span>
      </label>

      <div class="ml-auto flex items-center gap-3 text-xs text-base-content/60">
        <span>共 {{ stats.total }} 节点</span>
        <span>{{ stats.requirements }} 需求</span>
        <span>{{ stats.defects }} 缺陷</span>
        <span class="text-error">{{ stats.blocked }} 被阻塞</span>
        <span>{{ stats.edges }} 依赖边</span>
      </div>
    </div>

    <!-- 图例 -->
    <div class="mb-3 flex flex-wrap items-center gap-4 text-xs text-base-content/60">
      <div class="flex items-center gap-1.5">
        <span class="inline-block h-2.5 w-2.5 rounded-full" style="background: #a3ab96" />
        待处理
      </div>
      <div class="flex items-center gap-1.5">
        <span class="inline-block h-2.5 w-2.5 rounded-full" style="background: #2563eb" />
        进行中
      </div>
      <div class="flex items-center gap-1.5">
        <span class="inline-block h-2.5 w-2.5 rounded-full" style="background: #15803d" />
        已完成
      </div>
      <div class="flex items-center gap-1.5">
        <span class="inline-block h-2.5 w-2.5 rounded-full border-2 border-error" style="background: transparent" />
        被阻塞
      </div>
      <div class="flex items-center gap-1.5">
        <span class="inline-block h-0.5 w-6" style="background: #d97706" />
        关键路径
      </div>
    </div>

    <!-- 选中节点信息条 -->
    <div
      v-if="selectedNodeId"
      class="mb-3 flex items-center gap-3 rounded-lg bg-base-200 px-3 py-2 text-sm"
    >
      <span class="font-mono text-xs">{{ selectedNodeId }}</span>
      <button type="button" class="btn btn-ghost btn-xs" @click="goToDetail">
        查看详情
      </button>
      <button type="button" class="btn btn-ghost btn-xs" @click="selectedNodeId = null; highlightChain = new Set()">
        取消选中
      </button>
      <span v-if="highlightChain.size > 1" class="ml-auto text-xs text-info">
        阻塞链路：{{ highlightChain.size }} 个节点
      </span>
    </div>

    <!-- 图形容器 -->
    <div class="dep-graph-container relative min-h-0 flex-1 overflow-hidden rounded-xl border border-base-300">
      <div v-if="loading" class="absolute inset-0 z-10 flex items-center justify-center bg-base-100/80">
        <span class="loading loading-spinner loading-md" />
      </div>

      <div v-else-if="allNodes.length === 0" class="absolute inset-0 flex flex-col items-center justify-center gap-2 text-base-content/50">
        <Filter :size="32" />
        <p class="text-sm">暂无需求和缺陷</p>
      </div>

      <VueFlow
        v-else
        :nodes="positionedNodes"
        :edges="flowEdges"
        :node-types="{ dependency: DependencyNode }"
        :fit-view-on-init="true"
        :min-zoom="0.2"
        :max-zoom="2"
        :default-edge-options="{ type: 'smoothstep' }"
        class="dep-graph"
        @node-click="onNodeClick"
        @pane-click="onPaneClick"
      >
        <Background :gap="20" :size="1" :variant="BackgroundVariant.Dots" />
        <Controls :show-fit-view="false" :show-interactive="false" position="bottom-right">
          <button type="button" class="dep-control-btn" @click="() => zoomIn()" title="放大">
            <ZoomIn :size="16" />
          </button>
          <button type="button" class="dep-control-btn" @click="() => zoomOut()" title="缩小">
            <ZoomOut :size="16" />
          </button>
          <button type="button" class="dep-control-btn" @click="fitView()" title="适应视图">
            <Maximize2 :size="16" />
          </button>
        </Controls>
        <MiniMap
          :node-color="n => ((n as { data?: NodeData }).data)?.isBlocked ? '#dc2626' : '#2563eb'"
          :mask-color="'rgba(0,0,0,0.1)'"
          pannable
          zoomable
        />
      </VueFlow>
    </div>
  </AppPage>
</template>

<style scoped>
.dep-graph-container {
  background: var(--color-base-100);
}

.dep-graph {
  height: 100%;
  width: 100%;
}

.dep-control-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: var(--color-base-100);
  color: var(--color-base-content);
  cursor: pointer;
  border-bottom: 1px solid var(--color-base-300);
}

.dep-control-btn:hover {
  background: var(--color-base-200);
}

:deep(.vue-flow__minimap) {
  background: var(--color-base-100);
  border: 1px solid var(--color-base-300);
  border-radius: 8px;
  overflow: hidden;
}

:deep(.vue-flow__controls) {
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--color-base-300);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

:deep(.vue-flow__edge-path) {
  stroke-width: 1.5;
}
</style>
