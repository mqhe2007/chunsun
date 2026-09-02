<script setup lang="ts">
import { ExternalLink } from "@lucide/vue";
import { onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  AppColumn,
  AppField,
  AppModal,
  AppMultiSelect,
  AppPage,
  AppSelect,
  AppTable,
  confirm,
  useToast,
} from "@/ui";
import { api } from "@/utils/api";
import {
  DEFECT_SEVERITY_LABEL,
  DEFECT_STATUS_LABEL,
} from "@/utils/workflow";
import CopyableValue from "@/components/common/CopyableValue.vue";
import UserAvatar from "@/components/common/UserAvatar.vue";
import DependencySection from "@/components/projects/DependencySection.vue";
import NodePicker, { type PickedNode } from "@/components/projects/NodePicker.vue";

type DefectRow = {
  id: string;
  description?: string | null;
  status: string;
  severity: string;
  requirementId?: string | null;
  createdBy?: string | null;
  creator?: {
    id: string;
    nickname: string | null;
    qq: string | null;
    email: string | null;
  } | null;
  updatedAt: string;
};

const route = useRoute();
const router = useRouter();
const toast = useToast();

const loading = ref(false);
const saving = ref(false);
const defects = ref<DefectRow[]>([]);
const blockedDefectIds = ref<Set<string>>(new Set());

const statusFilter = ref<string[]>([]);
const severityFilter = ref("");
const keyword = ref("");

const showForm = ref(false);
const showDetail = ref(false);
const selected = ref<DefectRow | null>(null);
const editing = ref<DefectRow | null>(null);
const form = ref({
  description: "",
  status: "open",
  severity: "minor",
  blockedBy: [] as PickedNode[],
});

const projectId = () => (route.params as Record<string, string>).id;

const statusOptions = Object.entries(DEFECT_STATUS_LABEL).map(
  ([value, label]) => ({ label, value }),
);
const severityOptions = Object.entries(DEFECT_SEVERITY_LABEL).map(
  ([value, label]) => ({ label, value }),
);

function statusBadgeClass(status: string) {
  if (status === "resolved" || status === "closed") return "badge-success";
  if (status === "processing") return "badge-info";
  return "badge-warning";
}

function severityBadgeClass(sev: string) {
  if (sev === "critical") return "badge-error";
  if (sev === "major") return "badge-warning";
  if (sev === "trivial") return "badge-ghost";
  return "badge-info";
}

function creatorLabel(row: DefectRow): string {
  return row.creator?.nickname || row.creator?.email || "—";
}

function buildQuery() {
  const params = new URLSearchParams();
  if (statusFilter.value.length > 0) params.set("status", statusFilter.value.join(","));
  if (severityFilter.value) params.set("severity", severityFilter.value);
  const q = keyword.value.trim();
  if (q) params.set("q", q);
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

async function fetchDefects() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: DefectRow[] }>(
      `/projects/${projectId()}/defects${buildQuery()}`,
    );
    if (data.success) {
      defects.value = data.data;
    }
  } catch {
    toast.error("获取失败", "加载缺陷列表失败");
  } finally {
    loading.value = false;
  }
}

let filterTimer: ReturnType<typeof setTimeout> | null = null;
watch(
  [statusFilter, severityFilter, keyword],
  () => {
    if (filterTimer) clearTimeout(filterTimer);
    filterTimer = setTimeout(fetchDefects, 250);
  },
);

type DependencyEdge = {
  sourceType: "requirement" | "defect";
  sourceId: string;
  targetType: "requirement" | "defect";
  targetId: string;
};

async function fetchBlockedIds() {
  try {
    const { data } = await api.get<{
      success: boolean;
      data: DependencyEdge[];
    }>(`/projects/${projectId()}/dependencies`);
    if (data.success) {
      const set = new Set<string>();
      for (const e of data.data) {
        if (e.targetType === "defect") set.add(e.targetId);
      }
      blockedDefectIds.value = set;
    }
  } catch {
    // 忽略：依赖端点失败不阻断列表展示
  }
}

function openCreate() {
  editing.value = null;
  form.value = {
    description: "",
    status: "open",
    severity: "minor",
    blockedBy: [],
  };
  showForm.value = true;
}

function openDetail(row: DefectRow) {
  selected.value = row;
  showDetail.value = true;
}

function openEdit(row: DefectRow) {
  editing.value = row;
  form.value = {
    description: row.description ?? "",
    status: row.status,
    severity: row.severity,
    blockedBy: [],
  };
  showForm.value = true;
}

async function saveDefect() {
  if (!form.value.description.trim()) {
    toast.warn("请填写描述");
    return;
  }

  saving.value = true;
  const payload = {
    description: form.value.description.trim() || undefined,
    status: form.value.status,
    severity: form.value.severity,
    blockedBy: form.value.blockedBy.map(n => ({ kind: n.kind, id: n.id })),
  };

  try {
    if (editing.value) {
      await api.patch(
        `/projects/${projectId()}/defects/${editing.value.id}`,
        payload,
      );
      toast.success("已更新");
    } else {
      await api.post(`/projects/${projectId()}/defects`, payload);
      toast.success("已创建");
    }
    showForm.value = false;
    await fetchDefects();
  } catch (err: unknown) {
    const msg =
      (err as { response?: { data?: { error?: string } } })?.response?.data
        ?.error ?? "保存失败";
    toast.error("保存失败", msg);
  } finally {
    saving.value = false;
  }
}

async function confirmDelete(row: DefectRow) {
  const ok = await confirm({
    title: "删除缺陷",
    message: `确定删除缺陷「${row.id}」？`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    await api.delete(`/projects/${projectId()}/defects/${row.id}`);
    toast.success("已删除");
    await fetchDefects();
  } catch {
    toast.error("删除失败");
  }
}

async function confirmConvert(row: DefectRow) {
  const ok = await confirm({
    title: "转需求",
    message: `将缺陷「${row.id}」转为需求后，缺陷本身会删除。确定继续？`,
    confirmLabel: "转为需求",
  });
  if (!ok) return;
  try {
    const { data } = await api.post<{
      success: boolean;
      data: { id: string };
    }>(
      `/projects/${projectId()}/defects/${row.id}/convert-to-requirement`,
    );
    if (!data.success) throw new Error("convert failed");
    toast.success("已转为需求", data.data.id);
    await router.push(
      `/projects/${projectId()}/requirements/${data.data.id}`,
    );
  } catch {
    toast.error("转需求失败");
  }
}

onMounted(async () => {
  await Promise.all([fetchDefects(), fetchBlockedIds()]);
});
</script>

<template>
  <AppPage title="缺陷">
    <template #actions>
      <button
        type="button"
        class="btn btn-ghost"
        :disabled="loading"
        @click="fetchDefects"
      >
        <span v-if="loading" class="loading loading-spinner loading-xs" />
        刷新
      </button>
      <button type="button" class="btn btn-primary" @click="openCreate">新建缺陷</button>
    </template>

    <div class="flex flex-wrap items-center gap-2 mb-4">
      <input
        v-model="keyword"
        type="text"
        class="input w-44"
        placeholder="ID / 描述"
      />
      <AppMultiSelect
        v-model="statusFilter"
        :options="statusOptions"
        placeholder="全部状态"
      />
      <AppSelect
        v-model="severityFilter"
        class="w-44!"
        :options="severityOptions"
        clearable
        placeholder="全部严重级别"
      />
      <span class="text-sm text-base-content/60 sm:ml-auto">共 {{ defects.length }} 条</span>
    </div>

    <AppTable
      :rows="defects"
      :loading="loading"
      empty="暂无缺陷"
      striped
      paginator
      :rows-per-page="20"
    >
      <AppColumn header="ID" width="11rem">
        <template #default="{ row }">
          <CopyableValue
            :value="(row as DefectRow).id"
            label="复制 ID"
            value-class="defect-id"
          />
        </template>
      </AppColumn>
      <AppColumn header="描述">
        <template #default="{ row }">
          <span class="desc-cell">{{ (row as DefectRow).description || "—" }}</span>
        </template>
      </AppColumn>
      <AppColumn header="严重级别" width="100px">
        <template #default="{ row }">
          <span class="badge" :class="severityBadgeClass((row as DefectRow).severity)">
            {{ DEFECT_SEVERITY_LABEL[(row as DefectRow).severity] ?? (row as DefectRow).severity }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="状态" width="140px">
        <template #default="{ row }">
          <div class="status-cell">
            <span class="badge" :class="statusBadgeClass((row as DefectRow).status)">
              {{ DEFECT_STATUS_LABEL[(row as DefectRow).status] ?? (row as DefectRow).status }}
            </span>
            <span
              v-if="blockedDefectIds.has((row as DefectRow).id)"
              class="badge badge-error"
              title="被其他节点阻塞"
            >
              被阻塞
            </span>
          </div>
        </template>
      </AppColumn>
      <AppColumn header="更新" width="110px">
        <template #default="{ row }">
          {{ new Date((row as DefectRow).updatedAt).toLocaleDateString() }}
        </template>
      </AppColumn>
      <AppColumn header="操作" width="17rem">
        <template #default="{ row }">
          <div class="row-actions">
            <button type="button" class="btn btn-ghost btn-sm" @click="openDetail(row as DefectRow)">
              详情
            </button>
            <button type="button" class="btn btn-ghost btn-sm" @click="openEdit(row as DefectRow)">
              编辑
            </button>
            <button
              v-if="(row as DefectRow).status === 'open'"
              type="button"
              class="btn btn-ghost btn-sm"
              @click="confirmConvert(row as DefectRow)"
            >
              转需求
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-sm btn-error"
              @click="confirmDelete(row as DefectRow)"
            >
              删除
            </button>
          </div>
        </template>
      </AppColumn>
    </AppTable>

    <AppModal v-model="showForm" :title="editing ? '编辑缺陷' : '新建缺陷'">
      <div class="form-grid">
        <AppField label="描述 / 复现步骤" class="full">
          <textarea
            v-model="form.description"
            rows="4"
            class="textarea w-full"
          />
        </AppField>
        <AppField label="严重级别">
          <AppSelect v-model="form.severity" :options="severityOptions" />
        </AppField>
        <AppField label="状态">
          <AppSelect v-model="form.status" :options="statusOptions" />
        </AppField>
        <AppField label="上级依赖（被谁阻塞）" class="full">
          <NodePicker
            v-model="form.blockedBy"
            :project-id="projectId()"
            placeholder="选择上游需求/缺陷（可选）"
          />
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="showForm = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="saving" @click="saveDefect">
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          {{ editing ? "保存" : "创建" }}
        </button>
      </template>
    </AppModal>

    <AppModal v-model="showDetail" title="缺陷详情">
      <div v-if="selected" class="detail-grid">
        <div class="full">
          <span class="detail-label">ID</span>
          <CopyableValue
            :value="selected.id"
            label="复制 ID"
            value-class="defect-id"
          />
        </div>
        <div class="full">
          <span class="detail-label">描述 / 复现步骤</span>
          <p v-if="selected.description" class="detail-desc">{{ selected.description }}</p>
          <span v-else class="text-base-content/60">—</span>
        </div>
        <div>
          <span class="detail-label">严重级别</span>
          <span class="badge" :class="severityBadgeClass(selected.severity)">
            {{ DEFECT_SEVERITY_LABEL[selected.severity] ?? selected.severity }}
          </span>
        </div>
        <div>
          <span class="detail-label">状态</span>
          <span class="badge" :class="statusBadgeClass(selected.status)">
            {{ DEFECT_STATUS_LABEL[selected.status] ?? selected.status }}
          </span>
        </div>
        <div class="full">
          <span class="detail-label">创建人</span>
          <div v-if="selected.creator" class="creator-cell">
            <UserAvatar :qq="selected.creator.qq" :size="24" />
            <span>{{ creatorLabel(selected) }}</span>
          </div>
          <span v-else class="text-base-content/60">—</span>
        </div>
        <div class="full">
          <span class="detail-label">修复需求</span>
          <router-link
            v-if="selected.requirementId"
            class="detail-link"
            :to="`/projects/${projectId()}/requirements/${selected.requirementId}`"
          >
            {{ selected.requirementId }}
            <ExternalLink :size="14" aria-hidden="true" />
          </router-link>
          <p v-else class="text-base-content/60 empty-feats">
            尚未派生修复需求。在会话中执行 <code>/chunsun-fix {{ selected.id }}</code> 派生唯一修复需求并进入自主交付。
          </p>
        </div>
      </div>
      <DependencySection
        v-if="selected"
        class="mt-4"
        :project-id="projectId()"
        node-type="defect"
        :node-id="selected.id"
      />
      <template #footer>
        <button
          type="button"
          class="btn btn-primary"
          @click="openEdit(selected!); showDetail = false"
        >
          编辑
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.85rem;
}

.form-grid .full {
  grid-column: 1 / -1;
}

.row-actions {
  display: flex;
  flex-wrap: nowrap;
  gap: 0.15rem;
}

.defect-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.82rem;
  white-space: nowrap;
}

.desc-cell {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-wrap;
  line-height: 1.4;
}

.status-cell {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  align-items: center;
}

.detail-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem 1.25rem;
}

.detail-grid .full {
  grid-column: 1 / -1;
}

.detail-label {
  display: block;
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  margin-bottom: 0.25rem;
}

.detail-desc {
  margin: 0;
  white-space: pre-wrap;
  line-height: 1.5;
}

.detail-link {
  display: inline-flex;
  align-items: center;
  gap: 0.4rem;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.82rem;
  color: var(--color-primary);
  text-decoration: none;
}

.detail-link:hover {
  text-decoration: underline;
}

.creator-cell {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  min-width: 0;
}

.creator-cell > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty-feats code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.82rem;
  background: var(--color-base-200);
  padding: 0.05rem 0.35rem;
  border-radius: 4px;
}

@media (max-width: 560px) {
  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
