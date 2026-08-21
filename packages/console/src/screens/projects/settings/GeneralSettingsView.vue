<script setup lang="ts">
import { TriangleAlert } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { AppField, AppModal, AppPage, useToast } from "@/ui";
import { api } from "@/utils/api";
import { useProjectPermissions } from "@/utils/permissions";
import type { Project, ProjectMember } from "@/types/project";

const route = useRoute();
const router = useRouter();
const toast = useToast();
const projectId = () => (route.params as Record<string, string>).id;

const loading = ref(false);
const deleting = ref(false);
const deleteDialogVisible = ref(false);
const confirmText = ref("");
const project = ref<Project | null>(null);
const ownerId = ref<string | null>(null);
const members = ref<ProjectMember[]>([]);
const form = ref({ name: "", description: "" });
const saving = ref(false);
const errors = ref<Record<string, string>>({});

const { can } = useProjectPermissions(ownerId, members);
const canEdit = computed(() => can("project.update"));
const canDelete = computed(() => can("project.delete"));
const deleteEnabled = computed(() =>
  Boolean(project.value && confirmText.value === project.value.name),
);

async function fetchMeta() {
  loading.value = true;
  try {
    const [{ data: projectRes }, { data: membersRes }] = await Promise.all([
      api.get<{ success: boolean; data: Project }>(`/projects/${projectId()}`),
      api.get<{ success: boolean; data: ProjectMember[] }>(
        `/projects/${projectId()}/members`,
      ),
    ]);
    if (projectRes.success) {
      project.value = projectRes.data;
      ownerId.value = projectRes.data.userId;
      initForm();
    }
    if (membersRes.success) members.value = membersRes.data;
  } finally {
    loading.value = false;
  }
}

function onDelete() {
  if (!project.value) return;
  confirmText.value = "";
  deleteDialogVisible.value = true;
}

function initForm() {
  form.value = {
    name: project.value?.name ?? "",
    description: project.value?.description ?? "",
  };
}

async function onSave() {
  if (!project.value) return;
  errors.value = {};
  if (!form.value.name.trim()) {
    errors.value.name = "请填写项目名称";
    return;
  }
  saving.value = true;
  try {
    const { data } = await api.patch<{ success: boolean; data: Project }>(
      `/projects/${project.value.id}`,
      {
        name: form.value.name.trim(),
        description: form.value.description || undefined,
      },
    );
    if (data.success) {
      project.value = data.data;
      initForm();
      window.dispatchEvent(new CustomEvent("chunsun:project-updated"));
      toast.success("保存成功", "项目信息已更新");
    } else {
      toast.error("保存失败", "项目信息更新失败");
    }
  } catch {
    toast.error("保存失败", "请稍后重试");
  } finally {
    saving.value = false;
  }
}

async function confirmDelete() {
  if (!project.value) return;
  deleting.value = true;
  try {
    const { data } = await api.delete<{ success: boolean }>(`/projects/${project.value.id}`);
    if (data.success) {
      deleteDialogVisible.value = false;
      toast.success("删除成功", "项目已删除");
      router.push("/projects");
    } else {
      toast.error("删除失败", "请稍后重试");
    }
  } catch {
    toast.error("删除失败", "请稍后重试");
  } finally {
    deleting.value = false;
  }
}

onMounted(fetchMeta);
</script>

<template>
  <AppPage title="通用设置">
    <div v-if="loading" class="empty-state">
      <span class="loading loading-spinner loading-lg text-primary" />
    </div>

    <div v-else class="setting-list">
      <div v-if="project" class="setting-row setting-row--stack">
        <div class="setting-copy">
          <span class="setting-name">项目信息</span>
          <span class="setting-desc">
            项目名称与描述，将展示于项目列表与详情页。
          </span>
        </div>
        <form v-if="canEdit" class="setting-form" @submit.prevent="onSave">
          <AppField label="项目名称 *" html-for="project-name" :error="errors.name">
            <input
              id="project-name"
              v-model="form.name"
              type="text"
              class="input w-full"
              :class="{ 'input-error': errors.name }"
            />
          </AppField>
          <AppField label="项目描述" html-for="project-desc">
            <textarea
              id="project-desc"
              v-model="form.description"
              rows="3"
              class="textarea w-full"
            />
          </AppField>
          <div class="form-actions">
            <button type="submit" class="btn btn-primary" :disabled="saving">
              <span v-if="saving" class="loading loading-spinner loading-xs" />
              保存
            </button>
          </div>
        </form>
        <div v-else class="setting-form-readonly">
          <div class="readonly-row">
            <span class="readonly-label">项目名称</span>
            <span class="readonly-value">{{ project.name }}</span>
          </div>
          <div class="readonly-row">
            <span class="readonly-label">项目描述</span>
            <span class="readonly-value">{{ project.description || "—" }}</span>
          </div>
        </div>
      </div>

      <div v-if="project && canDelete" class="setting-row setting-row--danger">
        <div class="setting-copy">
          <span class="setting-name">删除项目</span>
          <span class="setting-desc">
            删除后不可恢复：项目数据与磁盘文件将一并清除，请谨慎操作。
          </span>
        </div>
        <button
          type="button"
          class="btn btn-error"
          :disabled="deleting"
          @click="onDelete"
        >
          <span v-if="deleting" class="loading loading-spinner loading-xs" />
          删除项目
        </button>
      </div>
    </div>

    <AppModal v-model="deleteDialogVisible" title="确认删除项目？">
      <div class="delete-dialog-body">
        <TriangleAlert class="delete-dialog-icon text-error" :size="28" aria-hidden="true" />
        <div class="delete-dialog-text">
          <p class="delete-dialog-msg">
            即将删除项目 <strong>"{{ project?.name }}"</strong>，此操作将同时删除磁盘文件，且不可撤回。
          </p>
          <AppField
            :label="`请输入项目名称 ${project?.name} 以确认删除`"
            html-for="delete-project-confirm"
          >
            <input
              id="delete-project-confirm"
              v-model="confirmText"
              type="text"
              class="input w-full"
              :disabled="deleting"
              placeholder="输入项目名称以确认"
              @keyup.enter="deleteEnabled && confirmDelete()"
            />
          </AppField>
        </div>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" :disabled="deleting" @click="deleteDialogVisible = false">
          取消
        </button>
        <button
          type="button"
          class="btn btn-error"
          :disabled="!deleteEnabled || deleting"
          @click="confirmDelete"
        >
          <span v-if="deleting" class="loading loading-spinner loading-xs" />
          删除
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.setting-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  max-width: 40rem;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.85rem 1rem;
  border-radius: 10px;
  background: var(--color-base-100);
}

.setting-copy {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 0;
}

.setting-name {
  font-weight: 600;
  font-size: 0.95rem;
}

.setting-desc {
  font-size: 0.82rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  line-height: 1.5;
}

.empty-state {
  display: flex;
  justify-content: center;
  padding: 3rem 0;
}

.setting-row--stack {
  flex-direction: column;
  align-items: stretch;
  gap: 0.85rem;
}

.setting-form {
  display: grid;
  gap: 1rem;
}

.form-actions {
  display: flex;
  justify-content: flex-end;
}

.setting-form-readonly {
  display: grid;
  gap: 0.6rem;
}

.readonly-row {
  display: flex;
  gap: 1rem;
  align-items: baseline;
}

.readonly-label {
  width: 4.5rem;
  flex-shrink: 0;
  font-size: 0.85rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.readonly-value {
  font-size: 0.9rem;
  overflow-wrap: anywhere;
}

.setting-row--danger {
  background: color-mix(in oklch, var(--color-error) 8%, var(--color-base-100));
}

.setting-row--danger .setting-name {
  color: var(--color-error);
}

.delete-dialog-body {
  display: flex;
  gap: 1rem;
  align-items: flex-start;
}

.delete-dialog-icon {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  background: color-mix(in oklch, var(--color-error) 15%, transparent);
  color: var(--color-error);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.25rem;
  flex-shrink: 0;
}

.delete-dialog-text {
  flex: 1;
  min-width: 0;
}

.delete-dialog-msg {
  font-size: 0.875rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  line-height: 1.55;
  overflow-wrap: anywhere;
  margin: 0 0 1rem;
}
</style>
