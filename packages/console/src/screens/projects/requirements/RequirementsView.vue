<script setup lang="ts">
import { Columns3, LayoutList } from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  AppColumn,
  AppDrawer,
  AppField,
  AppMultiSelect,
  AppPage,
  AppSelect,
  AppTable,
  AutoHeightTextarea,
  confirm,
  useToast,
} from "@/ui";
import { api } from "@/utils/api";
import { REQUIREMENT_STATUS_LABEL } from "@/utils/workflow";
import CopyableValue from "@/components/common/CopyableValue.vue";
import UserAvatar from "@/components/common/UserAvatar.vue";
import RequirementsBoard from "./RequirementsBoard.vue";
import {
  readViewMode,
  writeViewMode,
  type RequirementViewMode,
} from "./requirementBoard";

type Requirement = {
  id: string;
  description: string;
  status: string;
  coverage: string;
  origin?: string;
  ownerId?: string | null;
  owner?: {
    id: string;
    nickname: string | null;
    qq: string | null;
  } | null;
  createdAt: string;
  updatedAt: string;
};

type ProjectMemberBrief = {
  userId: string;
  user: {
    id: string;
    email: string;
    nickname: string | null;
  };
};

const route = useRoute();
const router = useRouter();
const toast = useToast();

const loading = ref(false);
const saving = ref(false);
const requirements = ref<Requirement[]>([]);
const members = ref<ProjectMemberBrief[]>([]);
const idFilter = ref("");
const statusFilter = ref<string[]>([]);
const ownerFilter = ref("");
const showForm = ref(false);
const editing = ref<Requirement | null>(null);
const viewMode = ref<RequirementViewMode>("list");
const boardReloadToken = ref(0);

const form = ref({
  description: "",
  ownerId: "" as string,
});

const projectId = () => (route.params as Record<string, string>).id;

function setViewMode(mode: RequirementViewMode) {
  if (viewMode.value === mode) return;
  viewMode.value = mode;
  writeViewMode(projectId(), mode);
  if (mode === "list") void fetchRequirements();
  else boardReloadToken.value += 1;
}

const statusOptions = Object.entries(REQUIREMENT_STATUS_LABEL).map(
  ([value, label]) => ({ label, value }),
);

const ownerOptions = computed(() =>
  members.value.map(m => ({
    label: m.user.nickname || m.user.email,
    value: m.userId,
  })),
);

function ownerLabel(row: Requirement): string {
  return row.owner?.nickname || "—";
}

function statusBadgeClass(status: string) {
  if (status === "completed") return "badge-success";
  if (status === "running") return "badge-info";
  return "badge-ghost";
}

function canMutate(row: Requirement): boolean {
  return row.status === "pending";
}

function openDetail(row: Requirement) {
  router.push(`/projects/${projectId()}/requirements/${row.id}`);
}

function redirectLegacyQuery() {
  const raw = route.query.requirementId;
  const id = Array.isArray(raw) ? raw[0] : raw;
  if (!id || typeof id !== "string") return;
  router.replace(`/projects/${projectId()}/requirements/${id}`);
}

function buildQuery() {
  const params = new URLSearchParams();
  const idQ = idFilter.value.trim();
  if (idQ) params.set("id", idQ);
  if (statusFilter.value.length > 0) params.set("status", statusFilter.value.join(","));
  if (ownerFilter.value) params.set("ownerId", ownerFilter.value);
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

async function fetchRequirements() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: Requirement[] }>(
      `/projects/${projectId()}/requirements${buildQuery()}`,
    );
    if (data.success) {
      requirements.value = data.data;
    }
  } catch {
    toast.error("获取失败", "加载需求列表失败");
  } finally {
    loading.value = false;
  }
}

async function fetchMembers() {
  try {
    const { data } = await api.get<{
      success: boolean;
      data: ProjectMemberBrief[];
    }>(`/projects/${projectId()}/members`);
    if (data.success) members.value = data.data;
  } catch {
    // 全局拦截器已提示错误
  }
}

function resetForm() {
  form.value = { description: "", ownerId: "" };
}

function openCreate() {
  editing.value = null;
  resetForm();
  showForm.value = true;
}

function openEdit(row: Requirement) {
  if (!canMutate(row)) return;
  editing.value = row;
  form.value = {
    description: row.description,
    ownerId: row.ownerId ?? "",
  };
  showForm.value = true;
}

async function saveRequirement() {
  if (!form.value.description.trim()) {
    toast.warn("请填写", "描述必填");
    return;
  }

  const payload = {
    description: form.value.description.trim(),
    ownerId: form.value.ownerId || null,
  };

  saving.value = true;
  try {
    if (editing.value) {
      const { data } = await api.patch<{
        success: boolean;
        data?: Requirement;
        error?: string;
      }>(`/projects/${projectId()}/requirements/${editing.value.id}`, payload);
      if (!data.success) {
        toast.error("更新失败", data.error ?? "未知错误");
        return;
      }
      toast.success("已更新");
    } else {
      const { data } = await api.post<{
        success: boolean;
        data?: { id: string };
        error?: string;
      }>(`/projects/${projectId()}/requirements`, payload);
      if (!data.success) {
        toast.error("创建失败", data.error ?? "未知错误");
        return;
      }
      toast.success("已创建", data.data?.id ?? form.value.description.trim());
    }
    showForm.value = false;
    editing.value = null;
    resetForm();
    boardReloadToken.value += 1;
    await Promise.all([fetchRequirements(), fetchMembers()]);
  } catch (err: unknown) {
    const msg =
      (err as { response?: { data?: { error?: string } } })?.response?.data
        ?.error ?? "请求异常";
    toast.error(editing.value ? "更新失败" : "创建失败", msg);
  } finally {
    saving.value = false;
  }
}

async function confirmDelete(row: Requirement) {
  if (!canMutate(row)) return;
  const ok = await confirm({
    title: "删除需求",
    message: `确定删除需求「${row.id}」？此操作不可撤回。`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    await api.delete(`/projects/${projectId()}/requirements/${row.id}`);
    toast.success("已删除");
    boardReloadToken.value += 1;
    await fetchRequirements();
  } catch {
    toast.error("删除失败");
  }
}

watch(() => route.query.requirementId, redirectLegacyQuery);

let filterTimer: ReturnType<typeof setTimeout> | null = null;
watch([idFilter, statusFilter, ownerFilter], () => {
  if (filterTimer) clearTimeout(filterTimer);
  filterTimer = setTimeout(() => {
    if (viewMode.value === "list") void fetchRequirements();
    else boardReloadToken.value += 1;
  }, 250);
});

onMounted(async () => {
  redirectLegacyQuery();
  viewMode.value = readViewMode(projectId());
  const jobs: Promise<unknown>[] = [fetchMembers()];
  if (viewMode.value === "list") jobs.unshift(fetchRequirements());
  await Promise.all(jobs);
});
</script>

<template>
  <AppPage
    title="需求"
    :fill="viewMode === 'board'"
    :class="viewMode === 'board' ? 'req-page--board' : ''"
  >
    <template #title-extra>
      <div class="join" role="group" aria-label="视图">
        <button
          type="button"
          class="btn btn-sm join-item"
          :class="viewMode === 'list' ? 'btn-active' : 'btn-ghost'"
          @click="setViewMode('list')"
        >
          <LayoutList :size="14" aria-hidden="true" />
          列表
        </button>
        <button
          type="button"
          class="btn btn-sm join-item"
          :class="viewMode === 'board' ? 'btn-active' : 'btn-ghost'"
          @click="setViewMode('board')"
        >
          <Columns3 :size="14" aria-hidden="true" />
          看板
        </button>
      </div>
    </template>
    <template #actions>
      <button
        type="button"
        class="btn btn-ghost"
        :disabled="loading"
        @click="viewMode === 'list' ? fetchRequirements() : boardReloadToken++"
      >
        <span v-if="loading" class="loading loading-spinner loading-xs" />
        刷新
      </button>
      <button type="button" class="btn btn-primary" @click="openCreate">新建需求</button>
    </template>

    <div class="flex shrink-0 flex-wrap items-center gap-2" :class="viewMode === 'list' ? 'mb-4' : ''">
      <input
        v-model="idFilter"
        type="text"
        class="input w-44"
        placeholder="需求 ID"
      />
      <AppMultiSelect
        v-if="viewMode === 'list'"
        v-model="statusFilter"
        :options="statusOptions"
        placeholder="全部状态"
      />
      <AppSelect
        v-model="ownerFilter"
        class="w-44!"
        :options="ownerOptions"
        clearable
        placeholder="全部负责人"
      />
      <span v-if="viewMode === 'list'" class="text-sm text-base-content/60 sm:ml-auto">
        共 {{ requirements.length }} 条
      </span>
    </div>

    <RequirementsBoard
      v-if="viewMode === 'board'"
      class="min-h-0 flex-1"
      :project-id="projectId()"
      :id-filter="idFilter"
      :owner-filter="ownerFilter"
      :reload-token="boardReloadToken"
      @open-detail="openDetail"
      @edit="openEdit"
      @delete="confirmDelete"
    />

    <AppTable
      v-else
      :rows="requirements"
      :loading="loading"
      empty="暂无需求"
      striped
      row-hover
      paginator
      :rows-per-page="20"
      @row-click="openDetail($event as Requirement)"
    >
      <AppColumn header="ID" width="190px">
        <template #default="{ row }">
          <CopyableValue
            :value="(row as Requirement).id"
            label="复制 ID"
            value-class="req-id"
            stop-propagation
          />
        </template>
      </AppColumn>
      <AppColumn header="负责人" width="140px">
        <template #default="{ row }">
          <div v-if="(row as Requirement).owner" class="owner-cell">
            <UserAvatar :qq="(row as Requirement).owner!.qq" :size="22" />
            <span>{{ ownerLabel(row as Requirement) }}</span>
          </div>
          <span v-else class="text-base-content/60">—</span>
        </template>
      </AppColumn>
      <AppColumn header="描述">
        <template #default="{ row }">
          <span class="cell-ellipsis" :title="(row as Requirement).description">
            {{ (row as Requirement).description }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="状态" width="170px">
        <template #default="{ row }">
          <div class="status-cell">
            <span class="badge" :class="statusBadgeClass((row as Requirement).status)">
              {{ REQUIREMENT_STATUS_LABEL[(row as Requirement).status] ?? (row as Requirement).status }}
            </span>
            <span v-if="(row as Requirement).origin === 'defect'" class="badge badge-warning">
              来自缺陷
            </span>
          </div>
        </template>
      </AppColumn>
      <AppColumn header="更新" width="120px">
        <template #default="{ row }">
          {{ new Date((row as Requirement).updatedAt).toLocaleDateString() }}
        </template>
      </AppColumn>
      <AppColumn header="操作" width="13rem">
        <template #default="{ row }">
          <div class="row-actions" @click.stop>
            <button type="button" class="btn btn-ghost btn-sm" @click="openDetail(row as Requirement)">
              详情
            </button>
            <template v-if="canMutate(row as Requirement)">
              <button type="button" class="btn btn-ghost btn-sm" @click="openEdit(row as Requirement)">
                编辑
              </button>
              <button
                type="button"
                class="btn btn-ghost btn-sm btn-error"
                @click="confirmDelete(row as Requirement)"
              >
                删除
              </button>
            </template>
          </div>
        </template>
      </AppColumn>
    </AppTable>

    <AppDrawer v-model="showForm" :title="editing ? '编辑需求' : '新建需求'">
      <div class="form-grid">
        <AppField label="负责人">
          <AppSelect
            v-model="form.ownerId"
            :options="ownerOptions"
            clearable
            placeholder="选择项目成员；可不指定"
          />
        </AppField>
        <AppField label="描述">
          <AutoHeightTextarea v-model="form.description" placeholder="需求描述" />
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="showForm = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="saving" @click="saveRequirement">
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          {{ editing ? "保存" : "创建" }}
        </button>
      </template>
    </AppDrawer>
  </AppPage>
</template>

<style scoped>
.cell-ellipsis {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.req-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.8rem;
}

.status-cell {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  align-items: center;
}

.owner-cell {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  min-width: 0;
}

.owner-cell > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-actions {
  display: flex;
  flex-wrap: nowrap;
  gap: 0.15rem;
}

.form-grid {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

</style>
