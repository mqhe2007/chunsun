<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import {
  AppColumn,
  AppField,
  AppModal,
  AppPage,
  AppSelect,
  AppTable,
  PasswordInput,
  confirm,
  useToast,
} from "@/ui";
import { api } from "@/utils/api";
import UserAvatar from "@/components/common/UserAvatar.vue";

type UserItem = {
  id: string;
  email: string;
  nickname: string | null;
  qq: string | null;
  role: "ADMIN" | "USER";
  status: "ACTIVE" | "INACTIVE" | "LOCKED";
  emailVerified: boolean;
  createdAt: string;
};

const toast = useToast();

const loading = ref(false);
const users = ref<UserItem[]>([]);
const total = ref(0);
const page = ref(1);
const pageSize = ref(20);

const createModalOpen = ref(false);
const creating = ref(false);
const createForm = ref({
  email: "",
  password: "",
  nickname: "",
  role: "USER" as "ADMIN" | "USER",
  status: "ACTIVE" as "ACTIVE" | "INACTIVE" | "LOCKED",
});

const editModalOpen = ref(false);
const editing = ref(false);
const editTarget = ref<UserItem | null>(null);
const editForm = ref({
  role: "USER" as "ADMIN" | "USER",
  status: "ACTIVE" as "ACTIVE" | "INACTIVE" | "LOCKED",
});

const roleOptions = [
  { label: "普通用户", value: "USER" },
  { label: "管理员", value: "ADMIN" },
];

const statusOptions = [
  { label: "正常", value: "ACTIVE" },
  { label: "禁用", value: "INACTIVE" },
  { label: "锁定", value: "LOCKED" },
];

function roleBadgeClass(role: UserItem["role"]) {
  return role === "ADMIN" ? "badge-warning" : "badge-info";
}

/** 展示态：账户 status 与邮箱验证分开存，未验证时 ACTIVE 应对用户显示「未激活」。 */
function displayStatus(user: UserItem): "ACTIVE" | "INACTIVE" | "LOCKED" | "UNVERIFIED" {
  if (user.status === "ACTIVE" && user.emailVerified === false) return "UNVERIFIED";
  return user.status;
}

function statusBadgeClass(user: UserItem) {
  const s = displayStatus(user);
  if (s === "ACTIVE") return "badge-success";
  if (s === "UNVERIFIED") return "badge-warning";
  if (s === "LOCKED") return "badge-error";
  return "badge-ghost";
}

function statusLabel(user: UserItem) {
  const s = displayStatus(user);
  if (s === "ACTIVE") return "正常";
  if (s === "UNVERIFIED") return "未激活";
  if (s === "INACTIVE") return "禁用";
  return "已锁定";
}

async function fetchUsers() {
  loading.value = true;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: UserItem[];
      meta: { total: number; page: number; pageSize: number; totalPages: number };
    }>("/users/admin/list", {
      params: {
        page: page.value,
        pageSize: pageSize.value,
      },
    });
    if (data.success) {
      users.value = data.data;
      total.value = data.meta.total;
    }
  } catch {
    toast.error("获取失败", "无法获取用户列表");
  } finally {
    loading.value = false;
  }
}

watch([page, pageSize], fetchUsers);

function openCreateModal() {
  createForm.value = { email: "", password: "", nickname: "", role: "USER", status: "ACTIVE" };
  createModalOpen.value = true;
}

async function handleCreate() {
  if (!createForm.value.email.trim() || !createForm.value.password.trim()) {
    toast.warn("验证失败", "邮箱和密码为必填项");
    return;
  }
  creating.value = true;
  try {
    await api.post("/users/admin/create", createForm.value);
    toast.success("创建成功", `用户 ${createForm.value.email} 已创建`);
    createModalOpen.value = false;
    await fetchUsers();
  } catch {
    toast.error("创建失败", "邮箱已存在");
  } finally {
    creating.value = false;
  }
}

function openEditModal(user: UserItem) {
  editTarget.value = user;
  editForm.value = { role: user.role, status: user.status };
  editModalOpen.value = true;
}

async function handleEdit() {
  if (!editTarget.value) return;
  editing.value = true;
  try {
    await api.patch(`/users/admin/${editTarget.value.id}`, editForm.value);
    toast.success("更新成功", "用户信息已更新");
    editModalOpen.value = false;
    await fetchUsers();
  } catch {
    toast.error("更新失败", "操作失败，请重试");
  } finally {
    editing.value = false;
  }
}

async function confirmDelete(user: UserItem) {
  const ok = await confirm({
    title: "确认删除",
    message: `确定要删除用户 "${user.email}" 吗？此操作不可撤销。`,
    confirmLabel: "删除",
    danger: true,
  });
  if (!ok) return;
  try {
    await api.delete(`/users/admin/${user.id}`);
    toast.success("已删除", `用户 ${user.email} 已删除`);
    await fetchUsers();
  } catch {
    toast.error("删除失败", "操作失败，请重试");
  }
}

onMounted(fetchUsers);
</script>

<template>
  <AppPage title="用户管理">
    <template #actions>
      <button type="button" class="btn btn-primary" @click="openCreateModal">新建用户</button>
    </template>

    <AppTable
      v-model:page="page"
      v-model:page-size="pageSize"
      :rows="users"
      :total="total"
      :loading="loading"
      empty="暂无用户数据"
      striped
    >
      <AppColumn header="用户">
        <template #default="{ row }">
          <div class="user-cell">
            <UserAvatar :qq="(row as UserItem).qq" :size="30" />
            <div class="user-cell-info">
              <div class="user-cell-name">
                {{ (row as UserItem).nickname || (row as UserItem).email }}
              </div>
              <div class="user-cell-sub">{{ (row as UserItem).email }}</div>
            </div>
          </div>
        </template>
      </AppColumn>
      <AppColumn header="邮箱">
        <template #default="{ row }">{{ (row as UserItem).email }}</template>
      </AppColumn>
      <AppColumn header="角色" width="7.5rem">
        <template #default="{ row }">
          <span class="badge whitespace-nowrap" :class="roleBadgeClass((row as UserItem).role)">
            {{ (row as UserItem).role === "ADMIN" ? "管理员" : "普通用户" }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="状态" width="6rem">
        <template #default="{ row }">
          <span class="badge whitespace-nowrap" :class="statusBadgeClass(row as UserItem)">
            {{ statusLabel(row as UserItem) }}
          </span>
        </template>
      </AppColumn>
      <AppColumn header="注册时间" width="180px">
        <template #default="{ row }">
          {{ new Date((row as UserItem).createdAt).toLocaleString("zh-CN") }}
        </template>
      </AppColumn>
      <AppColumn header="操作" width="10rem">
        <template #default="{ row }">
          <div class="action-btns">
            <button
              type="button"
              class="btn btn-ghost btn-sm"
              aria-label="编辑"
              @click="openEditModal(row as UserItem)"
            >
              编辑
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-sm btn-error"
              aria-label="删除"
              @click="confirmDelete(row as UserItem)"
            >
              删除
            </button>
          </div>
        </template>
      </AppColumn>
    </AppTable>

    <AppModal v-model="createModalOpen" title="新建用户">
      <div class="dialog-form">
        <AppField label="邮箱 *" html-for="create-email">
          <input
            id="create-email"
            v-model="createForm.email"
            type="email"
            class="input w-full"
            placeholder="user@example.com"
          />
        </AppField>
        <AppField label="密码 *" html-for="create-password">
          <PasswordInput
            id="create-password"
            v-model="createForm.password"
            placeholder="至少 6 位"
            autocomplete="new-password"
          />
        </AppField>
        <AppField label="昵称" html-for="create-nickname">
          <input
            id="create-nickname"
            v-model="createForm.nickname"
            type="text"
            class="input w-full"
            placeholder="可选"
          />
        </AppField>
        <div class="form-row">
          <AppField label="角色">
            <AppSelect v-model="createForm.role" :options="roleOptions" />
          </AppField>
          <AppField label="状态">
            <AppSelect v-model="createForm.status" :options="statusOptions" />
          </AppField>
        </div>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="createModalOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="creating" @click="handleCreate">
          <span v-if="creating" class="loading loading-spinner loading-sm" />
          创建
        </button>
      </template>
    </AppModal>

    <AppModal v-model="editModalOpen" :title="`编辑用户：${editTarget?.email ?? ''}`">
      <div class="dialog-form">
        <div class="form-row">
          <AppField label="角色">
            <AppSelect v-model="editForm.role" :options="roleOptions" />
          </AppField>
          <AppField label="状态">
            <AppSelect v-model="editForm.status" :options="statusOptions" />
          </AppField>
        </div>
      </div>
      <template #footer>
        <button type="button" class="btn btn-ghost" @click="editModalOpen = false">取消</button>
        <button type="button" class="btn btn-primary" :disabled="editing" @click="handleEdit">
          <span v-if="editing" class="loading loading-spinner loading-sm" />
          保存
        </button>
      </template>
    </AppModal>
  </AppPage>
</template>

<style scoped>
.action-btns {
  display: flex;
  flex-wrap: nowrap;
  gap: 0.25rem;
}

.user-cell {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  min-width: 0;
}

.user-cell-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.user-cell-name {
  font-weight: 600;
  font-size: 0.875rem;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-cell-sub {
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  line-height: 1.2;
}

.dialog-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
}
</style>
