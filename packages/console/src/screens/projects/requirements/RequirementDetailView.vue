<script setup lang="ts">
import { Pencil, Search, Trash2 } from "@lucide/vue";
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  AppDrawer,
  AppField,
  AppPage,
  AppSelect,
  AutoHeightTextarea,
  confirm,
  useToast,
} from "@/ui";
import { api } from "@/utils/api";
import { REQUIREMENT_STATUS_LABEL } from "@/utils/workflow";
import RequirementHarnessSection from "@/components/projects/RequirementHarnessSection.vue";
import DependencySection from "@/components/projects/DependencySection.vue";
import CopyableValue from "@/components/common/CopyableValue.vue";

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
  createdBy?: string | null;
  creator?: {
    id: string;
    nickname: string | null;
    qq: string | null;
    email: string | null;
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
const notFound = ref(false);
const requirement = ref<Requirement | null>(null);
const members = ref<ProjectMemberBrief[]>([]);
const showForm = ref(false);
const form = ref({
  description: "",
  ownerId: "" as string,
});

const projectId = computed(
  () => (route.params as Record<string, string>).id ?? "",
);
const requirementId = computed(
  () => (route.params as Record<string, string>).requirementId ?? "",
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

function creatorLabel(row: Requirement): string {
  return row.creator?.nickname || row.creator?.email || "—";
}

function statusBadgeClass(status: string) {
  if (status === "completed") return "badge-success";
  if (status === "running") return "badge-info";
  return "badge-ghost";
}

function canMutate(row: Requirement): boolean {
  return row.status === "pending";
}

function formatDateTime(value: string) {
  return new Date(value).toLocaleString();
}

function goBack() {
  router.push(`/projects/${projectId.value}/requirements`);
}

async function fetchRequirement() {
  if (!projectId.value || !requirementId.value) return;
  loading.value = true;
  notFound.value = false;
  try {
    const { data } = await api.get<{
      success: boolean;
      data?: Requirement;
      error?: string;
    }>(`/projects/${projectId.value}/requirements/${requirementId.value}`);
    if (data.success && data.data) {
      requirement.value = data.data;
    } else {
      requirement.value = null;
      notFound.value = true;
    }
  } catch {
    requirement.value = null;
    notFound.value = true;
    toast.add({
      severity: "error",
      summary: "获取失败",
      detail: "加载需求详情失败",
      life: 3000,
    });
  } finally {
    loading.value = false;
  }
}

async function fetchMembers() {
  if (!projectId.value) return;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: ProjectMemberBrief[];
    }>(`/projects/${projectId.value}/members`);
    if (data.success) members.value = data.data;
  } catch {
    // 全局拦截器已提示错误
  }
}

function openEdit() {
  const row = requirement.value;
  if (!row || !canMutate(row)) return;
  form.value = {
    description: row.description,
    ownerId: row.ownerId ?? "",
  };
  showForm.value = true;
}

async function saveRequirement() {
  const row = requirement.value;
  if (!row) return;
  if (!form.value.description.trim()) {
    toast.add({
      severity: "warn",
      summary: "请填写",
      detail: "描述必填",
      life: 2500,
    });
    return;
  }

  saving.value = true;
  try {
    const { data } = await api.patch<{
      success: boolean;
      data?: Requirement;
      error?: string;
    }>(`/projects/${projectId.value}/requirements/${row.id}`, {
      description: form.value.description.trim(),
      ownerId: form.value.ownerId || null,
    });
    if (!data.success) {
      toast.add({
        severity: "error",
        summary: "更新失败",
        detail: data.error ?? "未知错误",
        life: 3000,
      });
      return;
    }
    toast.add({ severity: "success", summary: "已更新", life: 2000 });
    showForm.value = false;
    if (data.data) requirement.value = data.data;
    else await fetchRequirement();
  } catch (err: unknown) {
    const msg =
      (err as { response?: { data?: { error?: string } } })?.response?.data
        ?.error ?? "请求异常";
    toast.add({
      severity: "error",
      summary: "更新失败",
      detail: msg,
      life: 3000,
    });
  } finally {
    saving.value = false;
  }
}

async function confirmDelete() {
  const row = requirement.value;
  if (!row || !canMutate(row)) return;
  const ok = await confirm({
    title: "删除需求",
    message: `确定删除需求「${row.id}」？此操作不可撤回。`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    await api.delete(
      `/projects/${projectId.value}/requirements/${row.id}`,
    );
    toast.add({
      severity: "success",
      summary: "已删除",
      life: 2000,
    });
    goBack();
  } catch {
    toast.add({
      severity: "error",
      summary: "删除失败",
      life: 3000,
    });
  }
}

watch(requirementId, () => {
  void fetchRequirement();
});

onMounted(async () => {
  await Promise.all([fetchRequirement(), fetchMembers()]);
});
</script>

<template>
  <AppPage
    title="需求详情"
    :back="{ to: `/projects/${projectId}/requirements`, label: '返回需求列表' }"
  >
    <template v-if="requirement" #title-extra>
      <div class="status-cell">
        <span class="badge" :class="statusBadgeClass(requirement.status)">
          {{
            REQUIREMENT_STATUS_LABEL[requirement.status] ??
            requirement.status
          }}
        </span>
        <span
          v-if="requirement.origin === 'defect'"
          class="badge badge-warning"
        >
          来自缺陷
        </span>
      </div>
    </template>
    <template v-if="requirement && canMutate(requirement)" #actions>
      <button type="button" class="btn btn-ghost" @click="openEdit">
        <Pencil :size="14" aria-hidden="true" />
        编辑
      </button>
      <button
        type="button"
        class="btn btn-ghost btn-error"
        @click="confirmDelete"
      >
        <Trash2 :size="14" aria-hidden="true" />
        删除
      </button>
    </template>

    <div v-if="loading" class="empty-state">
      <span class="loading loading-spinner loading-lg text-primary" />
    </div>

    <template v-else-if="requirement">
      <div class="detail-layout">
        <section class="panel meta-panel">
          <div class="panel-head">
            <h2 class="panel-title">基本信息</h2>
          </div>
          <div class="detail-grid">
            <div class="full">
              <span class="detail-label">ID</span>
              <CopyableValue
                :value="requirement.id"
                label="复制 ID"
                value-class="req-id"
              />
            </div>
            <div>
              <span class="detail-label">负责人</span>
              <div v-if="requirement.owner" class="owner-cell">
                <UserAvatar :qq="requirement.owner.qq" :size="24" />
                <span>{{ ownerLabel(requirement) }}</span>
              </div>
              <span v-else class="text-base-content/60">—</span>
            </div>
            <div>
              <span class="detail-label">创建人</span>
              <div v-if="requirement.creator" class="owner-cell">
                <UserAvatar :qq="requirement.creator.qq" :size="24" />
                <span>{{ creatorLabel(requirement) }}</span>
              </div>
              <span v-else class="text-base-content/60">—</span>
            </div>
            <div>
              <span class="detail-label">状态</span>
              <div class="status-cell">
                <span class="badge" :class="statusBadgeClass(requirement.status)">
                  {{
                    REQUIREMENT_STATUS_LABEL[requirement.status] ??
                    requirement.status
                  }}
                </span>
              </div>
            </div>
            <div>
              <span class="detail-label">创建时间</span>
              <span class="detail-value">{{
                formatDateTime(requirement.createdAt)
              }}</span>
            </div>
            <div>
              <span class="detail-label">更新时间</span>
              <span class="detail-value">{{
                formatDateTime(requirement.updatedAt)
              }}</span>
            </div>
            <div class="full">
              <span class="detail-label">描述</span>
              <p class="detail-desc">{{ requirement.description }}</p>
            </div>
          </div>
        </section>

        <DependencySection
          :project-id="projectId"
          node-type="requirement"
          :node-id="requirement.id"
        />

        <RequirementHarnessSection
          :key="requirement.id"
          :requirement-id="requirement.id"
        />
      </div>
    </template>

    <div v-else-if="notFound" class="empty-state empty-state--message">
      <Search class="empty-icon text-base-content/40" :size="40" aria-hidden="true" />
      <p class="text-base-content/60">需求不存在或不属于当前项目</p>
      <button type="button" class="btn btn-ghost" @click="goBack">
        返回需求列表
      </button>
    </div>

    <AppDrawer v-model="showForm" title="编辑需求">
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
          <AutoHeightTextarea v-model="form.description" />
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="showForm = false">取消</button>
        <button
          type="button"
          class="btn btn-primary"
          :disabled="saving"
          @click="saveRequirement"
        >
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          保存
        </button>
      </template>
    </AppDrawer>
  </AppPage>
</template>

<style scoped>
.empty-state {
  display: flex;
  justify-content: center;
  padding: 3rem;
}

.empty-state--message {
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
}

.empty-icon {
  font-size: 2.5rem;
  line-height: 1;
}

.detail-layout {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.panel {
  border-radius: 12px;
  background: var(--color-base-100);
  padding: 1rem 1.1rem;
  min-width: 0;
}

.panel-head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.35rem 0.75rem;
  margin-bottom: 0.85rem;
}

.panel-title {
  margin: 0;
  font-size: 0.95rem;
  font-weight: 650;
}

.detail-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 1rem 1.25rem;
}

@media (min-width: 768px) {
  .detail-grid {
    grid-template-columns: 1fr 1fr;
  }
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

.detail-value {
  font-size: 0.9rem;
}

.detail-desc {
  margin: 0;
  white-space: pre-wrap;
  line-height: 1.6;
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

.form-grid {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

</style>
