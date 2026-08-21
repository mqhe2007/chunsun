<script setup lang="ts">
import { onMounted, ref } from "vue";
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
import { formatRelativeTime } from "@/utils/time";
import CopyButton from "@/components/common/CopyButton.vue";

type InvitationCodeItem = {
  id: string;
  code: string;
  inviterId: string;
  role: "ADMIN" | "USER";
  maxUses: number;
  usedCount: number;
  expiresAt: string | null;
  createdAt: string;
};

const toast = useToast();
const loading = ref(false);
const codes = ref<InvitationCodeItem[]>([]);

const createModalOpen = ref(false);
const creating = ref(false);
const createForm = ref({
  role: "USER" as "ADMIN" | "USER",
  maxUses: 1,
  expiresAt: "",
  sendTo: "",
});

const roleOptions = [
  { label: "普通用户", value: "USER" },
  { label: "管理员", value: "ADMIN" },
];

async function fetchCodes() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: InvitationCodeItem[] }>(
      "/admin/invitations",
    );
    if (data.success) {
      codes.value = data.data;
    }
  } catch {
    toast.error("获取失败", "无法加载邀请码列表");
  } finally {
    loading.value = false;
  }
}

function openCreateModal() {
  createForm.value = {
    role: "USER",
    maxUses: 1,
    expiresAt: "",
    sendTo: "",
  };
  createModalOpen.value = true;
}

async function handleCreate() {
  creating.value = true;
  try {
    const expiresAt = createForm.value.expiresAt
      ? new Date(createForm.value.expiresAt).toISOString()
      : undefined;
    const { data } = await api.post<{ success: boolean; data: InvitationCodeItem }>(
      "/admin/invitations",
      {
        role: createForm.value.role,
        maxUses: createForm.value.maxUses,
        expiresAt,
        sendTo: createForm.value.sendTo || undefined,
      },
    );
    if (data.success) {
      toast.success(
        "创建成功",
        createForm.value.sendTo ? "邀请码已创建并发送邮件" : "邀请码已创建",
      );
      createModalOpen.value = false;
      await fetchCodes();
    }
  } catch {
    toast.error("创建失败", "请检查输入后重试");
  } finally {
    creating.value = false;
  }
}

async function confirmDelete(item: InvitationCodeItem) {
  const ok = await confirm({
    title: "删除邀请码",
    message: `确定删除邀请码 ${item.code} 吗？`,
    danger: true,
    confirmLabel: "删除",
  });
  if (ok) await handleDelete(item.id);
}

async function handleDelete(id: string) {
  try {
    await api.delete(`/admin/invitations/${id}`);
    toast.success("已删除", "邀请码已删除");
    await fetchCodes();
  } catch {
    toast.error("删除失败", "请稍后重试");
  }
}

function isExpired(item: InvitationCodeItem): boolean {
  if (!item.expiresAt) return false;
  return new Date(item.expiresAt) < new Date();
}

function isExhausted(item: InvitationCodeItem): boolean {
  return item.usedCount >= item.maxUses;
}

function roleBadgeClass(role: InvitationCodeItem["role"]) {
  return role === "ADMIN" ? "badge-warning" : "badge-info";
}

function statusBadge(item: InvitationCodeItem) {
  if (isExpired(item)) return { label: "已过期", class: "badge-ghost" };
  if (isExhausted(item)) return { label: "已用完", class: "badge-ghost" };
  return { label: "有效", class: "badge-success" };
}

onMounted(fetchCodes);
</script>

<template>
  <AppPage title="邀请码">
    <template #actions>
      <button type="button" class="btn btn-primary" @click="openCreateModal">新建邀请码</button>
    </template>

    <AppTable :rows="codes" :loading="loading" empty="暂无邀请码" striped>
      <AppColumn header="邀请码">
        <template #default="{ row }">
          <div class="code-cell">
            <code class="code-value">{{ (row as InvitationCodeItem).code }}</code>
            <CopyButton :value="(row as InvitationCodeItem).code" />
          </div>
        </template>
      </AppColumn>
      <AppColumn header="角色" width="100px">
        <template #default="{ row }">
          <span class="badge" :class="roleBadgeClass((row as InvitationCodeItem).role)">
            {{ (row as InvitationCodeItem).role === "ADMIN" ? "管理员" : "普通用户" }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="使用情况" width="120px">
        <template #default="{ row }">
          {{ (row as InvitationCodeItem).usedCount }} / {{ (row as InvitationCodeItem).maxUses }}
        </template>
      </AppColumn>
      <AppColumn header="有效期" width="160px">
        <template #default="{ row }">
          <span
            v-if="(row as InvitationCodeItem).expiresAt"
            :class="{ expired: isExpired(row as InvitationCodeItem) }"
          >
            {{ formatRelativeTime((row as InvitationCodeItem).expiresAt!) }}
          </span>
          <span v-else class="text-base-content/60">永久有效</span>
        </template>
      </AppColumn>
      <AppColumn header="创建时间" width="150px">
        <template #default="{ row }">
          {{ formatRelativeTime((row as InvitationCodeItem).createdAt) }}
        </template>
      </AppColumn>
      <AppColumn header="状态" width="100px">
        <template #default="{ row }">
          <span class="badge" :class="statusBadge(row as InvitationCodeItem).class">
            {{ statusBadge(row as InvitationCodeItem).label }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="操作" width="6.5rem">
        <template #default="{ row }">
          <button
            type="button"
            class="btn btn-ghost btn-sm btn-error"
            aria-label="删除"
            @click="confirmDelete(row as InvitationCodeItem)"
          >
            删除
          </button>
        </template>
      </AppColumn>
    </AppTable>

    <AppModal v-model="createModalOpen" title="新建邀请码">
      <div class="create-form">
        <AppField label="角色" html-for="role">
          <select id="role" v-model="createForm.role" class="select w-full">
            <option v-for="opt in roleOptions" :key="opt.value" :value="opt.value">
              {{ opt.label }}
            </option>
          </select>
        </AppField>
        <AppField label="最大使用次数" html-for="maxUses">
          <input
            id="maxUses"
            v-model.number="createForm.maxUses"
            type="number"
            min="1"
            class="input w-full"
          />
        </AppField>
        <AppField label="过期时间（可选）" html-for="expiresAt">
          <input
            id="expiresAt"
            v-model="createForm.expiresAt"
            type="datetime-local"
            class="input w-full"
          />
        </AppField>
        <AppField
          label="发送给（可选）"
          html-for="sendTo"
          hint="填写后将通过邮件发送邀请码。"
        >
          <input
            id="sendTo"
            v-model="createForm.sendTo"
            type="email"
            class="input w-full"
            placeholder="收件人邮箱"
          />
        </AppField>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="createModalOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="creating" @click="handleCreate">
          <span v-if="creating" class="loading loading-spinner loading-sm" />
          创建
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.code-cell {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.code-value {
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.9rem;
  padding: 0.25rem 0.5rem;
  background: var(--color-base-200, #f2f4ee);
  border-radius: 0.375rem;
}

.expired {
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  text-decoration: line-through;
}


.create-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
</style>
