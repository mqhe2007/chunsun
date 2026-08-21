<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import {
  AppColumn,
  AppField,
  AppModal,
  AppPage,
  AppTable,
  confirm,
  useToast,
} from "@/ui";
import { api } from "@/utils/api";
import { useProjectPermissions } from "@/utils/permissions";
import type { ProjectMember } from "@/types/project";

type EnvVarRow = {
  id: string;
  key: string;
  description: string | null;
  isSecret: boolean;
  value: string | null;
  hasValue: boolean;
  createdAt: string;
  updatedAt: string;
};

const route = useRoute();
const toast = useToast();
const projectId = () => (route.params as Record<string, string>).id;

const loading = ref(false);
const saving = ref(false);
const rows = ref<EnvVarRow[]>([]);
const ownerId = ref<string | null>(null);
const members = ref<ProjectMember[]>([]);
const dialogOpen = ref(false);
const editing = ref<EnvVarRow | null>(null);
const showSecretValue = ref(false);

const form = ref({
  key: "",
  value: "",
  description: "",
  isSecret: true,
});

const { can } = useProjectPermissions(ownerId, members);
const canManage = computed(() => can("envVar.write"));

function formatTime(iso: string) {
  return new Date(iso).toLocaleString("zh-CN");
}

function maskDisplay(row: EnvVarRow) {
  return row.hasValue ? "••••••••" : "—";
}

async function fetchMeta() {
  try {
    const [{ data: projectRes }, { data: membersRes }] = await Promise.all([
      api.get<{ success: boolean; data: { userId: string } }>(
        `/projects/${projectId()}`,
      ),
      api.get<{ success: boolean; data: ProjectMember[] }>(
        `/projects/${projectId()}/members`,
      ),
    ]);
    if (projectRes.success) ownerId.value = projectRes.data.userId;
    if (membersRes.success) members.value = membersRes.data;
  } catch {
    // ignore meta errors; table still usable read-only
  }
}

async function fetchRows() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: EnvVarRow[] }>(
      `/projects/${projectId()}/env-vars`,
    );
    if (data.success) rows.value = data.data;
  } catch {
    toast.error("加载失败", "无法加载环境变量");
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  editing.value = null;
  form.value = { key: "", value: "", description: "", isSecret: true };
  showSecretValue.value = false;
  dialogOpen.value = true;
}

function openEdit(row: EnvVarRow) {
  editing.value = row;
  form.value = {
    key: row.key,
    value: "",
    description: row.description ?? "",
    isSecret: row.isSecret,
  };
  showSecretValue.value = false;
  dialogOpen.value = true;
}

async function save() {
  const key = form.value.key.trim();
  if (!/^[A-Z][A-Z0-9_]*$/.test(key)) {
    toast.warn("变量名无效", "须匹配 ^[A-Z][A-Z0-9_]*$，如 DATABASE_URL");
    return;
  }

  if (!editing.value && form.value.value.length === 0) {
    toast.warn("请填写值");
    return;
  }

  saving.value = true;
  try {
    if (editing.value) {
      const body: Record<string, unknown> = {
        key,
        description: form.value.description.trim() || null,
        isSecret: form.value.isSecret,
      };
      if (form.value.value.length > 0) body.value = form.value.value;

      const { data } = await api.patch<{ success: boolean; error?: string }>(
        `/projects/${projectId()}/env-vars/${editing.value.id}`,
        body,
      );
      if (!data.success) throw new Error(data.error ?? "更新失败");
      toast.success("已更新");
    } else {
      const { data } = await api.post<{ success: boolean; error?: string }>(
        `/projects/${projectId()}/env-vars`,
        {
          key,
          value: form.value.value,
          description: form.value.description.trim() || null,
          isSecret: form.value.isSecret,
        },
      );
      if (!data.success) throw new Error(data.error ?? "创建失败");
      toast.success("已添加");
    }
    dialogOpen.value = false;
    await fetchRows();
  } catch (err) {
    toast.error("保存失败", err instanceof Error ? err.message : "请稍后重试");
  } finally {
    saving.value = false;
  }
}

async function confirmDelete(row: EnvVarRow) {
  const ok = await confirm({
    title: "删除环境变量",
    message: `确认删除环境变量 ${row.key}？`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    const { data } = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/env-vars/${row.id}`,
    );
    if (data.success) {
      toast.success("已删除");
      await fetchRows();
    }
  } catch {
    toast.error("删除失败", "请稍后重试");
  }
}

onMounted(async () => {
  await fetchMeta();
  await fetchRows();
});
</script>

<template>
  <AppPage title="环境变量">
    <template v-if="canManage" #actions>
      <button type="button" class="btn btn-primary" @click="openCreate">添加变量</button>
    </template>

    <div class="env-hint alert alert-info text-sm">
      <span>
        控制台不展示明文，取值请用 CLI
        <code>chunsun env get &lt;KEY&gt;</code>；团队共享写入此处，个人私密请写本地
        <code>.env</code>。Secret 值存库加密。
      </span>
    </div>

    <AppTable :rows="rows" :loading="loading" empty="暂无环境变量" striped>
      <AppColumn header="Key">
        <template #default="{ row }">
          <code class="mono">{{ (row as EnvVarRow).key }}</code>
        </template>
      </AppColumn>
      <AppColumn header="值">
        <template #default="{ row }">
          <span class="mono muted">{{ maskDisplay(row as EnvVarRow) }}</span>
        </template>
      </AppColumn>
      <AppColumn header="说明">
        <template #default="{ row }">
          {{ (row as EnvVarRow).description || "—" }}
        </template>
      </AppColumn>
      <AppColumn header="类型" width="100px">
        <template #default="{ row }">
          <span
            class="badge"
            :class="(row as EnvVarRow).isSecret ? 'badge-warning' : 'badge-ghost'"
          >
            {{ (row as EnvVarRow).isSecret ? "Secret" : "明文" }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="更新时间" width="190px">
        <template #default="{ row }">
          {{ formatTime((row as EnvVarRow).updatedAt) }}
        </template>
      </AppColumn>
      <AppColumn v-if="canManage" header="操作" width="10rem">
        <template #default="{ row }">
          <div class="action-btns">
            <button
              type="button"
              class="btn btn-ghost btn-sm"
              aria-label="编辑"
              @click="openEdit(row as EnvVarRow)"
            >
              编辑
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-sm btn-error"
              aria-label="删除"
              @click="confirmDelete(row as EnvVarRow)"
            >
              删除
            </button>
          </div>
        </template>
      </AppColumn>
    </AppTable>

    <AppModal v-model="dialogOpen" :title="editing ? `编辑：${editing.key}` : '添加环境变量'">
      <div class="dialog-form">
        <AppField label="Key *" html-for="env-key">
          <input
            id="env-key"
            v-model="form.key"
            type="text"
            class="input w-full font-mono"
            placeholder="如 STAGING_API_BASE"
            :disabled="Boolean(editing)"
          />
        </AppField>
        <AppField
          :label="editing ? '值（留空则不修改）' : '值 *'"
          html-for="env-value"
        >
          <label
            v-if="form.isSecret"
            class="input flex items-center gap-2 w-full"
          >
            <input
              id="env-value"
              v-model="form.value"
              :type="showSecretValue ? 'text' : 'password'"
              class="grow font-mono"
              :placeholder="editing ? '留空则保持原值' : ''"
            />
            <button
              type="button"
              class="btn btn-ghost btn-sm"
              @click="showSecretValue = !showSecretValue"
            >
              {{ showSecretValue ? "隐藏" : "显示" }}
            </button>
          </label>
          <input
            v-else
            id="env-value"
            v-model="form.value"
            type="text"
            class="input w-full font-mono"
            :placeholder="editing ? '留空则保持原值' : ''"
          />
        </AppField>
        <AppField label="说明" html-for="env-desc">
          <input
            id="env-desc"
            v-model="form.description"
            type="text"
            class="input w-full"
            placeholder="给同事 / Agent 的选用提示"
          />
        </AppField>
        <label class="fieldset-label cursor-pointer justify-start gap-3">
          <input v-model="form.isSecret" type="checkbox" class="checkbox" />
          标记为 Secret（列表掩码）
        </label>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="dialogOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="saving" @click="save">
          <span v-if="saving" class="loading loading-spinner loading-sm" />
          保存
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.env-hint code {
  font-size: 0.85em;
}

.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.88rem;
}

.muted {
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.action-btns {
  display: inline-flex;
  flex-wrap: nowrap;
  gap: 0.15rem;
}

.dialog-form {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}
</style>
