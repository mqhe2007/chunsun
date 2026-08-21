<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { AppPage, AppSelect, useToast } from "@/ui";
import { useAuthStore } from "@/stores/auth";
import { api } from "@/utils/api";
import { useProjectPermissions } from "@/utils/permissions";
import UserAvatar from "@/components/common/UserAvatar.vue";
import UserSearchInput, {
  type UserSuggestion,
} from "@/components/common/UserSearchInput.vue";
import type { ProjectMember, ProjectMemberRole } from "@/types/project";

const route = useRoute();
const toast = useToast();
const authStore = useAuthStore();
const currentUserId = authStore.userId;

const projectId = () => (route.params as Record<string, string>).id;

const loading = ref(false);
const members = ref<ProjectMember[]>([]);
const memberSearchQuery = ref("");
const ownerId = ref("");
const inviting = ref(false);

const inviteForm = ref<{
  selectedUser: UserSuggestion | null;
  role: "ADMIN" | "MEMBER";
}>({ selectedUser: null, role: "MEMBER" });

const suggestions = ref<UserSuggestion[]>([]);
const searchLoading = ref(false);

const roleLabel: Record<ProjectMemberRole, string> = {
  OWNER: "拥有者",
  ADMIN: "管理员",
  MEMBER: "成员",
};

function roleBadgeClass(role: ProjectMemberRole) {
  if (role === "OWNER") return "badge-warning";
  if (role === "ADMIN") return "badge-info";
  return "badge-ghost";
}

const roleChangeOptions = [
  { label: "管理员", value: "ADMIN" },
  { label: "成员", value: "MEMBER" },
];

const inviteRoleOptions = [
  { label: "成员", value: "MEMBER" },
  { label: "管理员", value: "ADMIN" },
];

const isOwner = computed(() => currentUserId === ownerId.value);
const { can } = useProjectPermissions(ownerId, members);
const canManage = computed(() => can("member.invite"));
const canChangeRole = computed(() => can("member.role"));
const canRemoveMembers = computed(() => can("member.remove"));

const filteredMembers = computed(() => {
  const q = memberSearchQuery.value.trim().toLowerCase();
  if (!q) return members.value;
  return members.value.filter(
    m =>
      m.user.email.toLowerCase().includes(q) ||
      (m.user.nickname ?? "").toLowerCase().includes(q),
  );
});

async function searchUsers(query: string) {
  const q = query.trim();
  if (!q) {
    suggestions.value = [];
    return;
  }
  searchLoading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: UserSuggestion[] }>(
      `/users/search`,
      { params: { q } },
    );
    if (data.success) suggestions.value = data.data;
  } catch {
    suggestions.value = [];
  } finally {
    searchLoading.value = false;
  }
}

async function fetchOwner() {
  try {
    const { data } = await api.get<{ success: boolean; data: { userId: string } }>(
      `/projects/${projectId()}`,
    );
    if (data.success) ownerId.value = data.data.userId;
  } catch {
    // ignore
  }
}

async function fetchMembers() {
  loading.value = true;
  try {
    const { data } = await api.get<{ success: boolean; data: ProjectMember[] }>(
      `/projects/${projectId()}/members`,
    );
    if (data.success) members.value = data.data;
  } catch {
    toast.error("获取失败", "无法加载成员列表");
  } finally {
    loading.value = false;
  }
}

async function invite() {
  if (!inviteForm.value.selectedUser) return;
  inviting.value = true;
  try {
    const { data } = await api.post<{
      success: boolean;
      data: ProjectMember;
      error?: string;
    }>(`/projects/${projectId()}/members`, {
      identifier: inviteForm.value.selectedUser.email,
      role: inviteForm.value.role,
    });
    if (data.success) {
      toast.success("邀请成功", "成员已加入项目");
      inviteForm.value = {
        selectedUser: null,
        role: "MEMBER",
      };
      suggestions.value = [];
      await fetchMembers();
    } else {
      const errMap: Record<string, string> = {
        USER_NOT_FOUND: "未找到该用户",
        CANNOT_INVITE_SELF: "不能邀请自己",
        USER_IS_OWNER: "该用户已是项目拥有者",
        FORBIDDEN: "权限不足",
      };
      toast.error("邀请失败", errMap[data.error ?? ""] ?? "操作失败");
    }
  } catch {
    toast.error("邀请失败", "请稍后重试");
  } finally {
    inviting.value = false;
  }
}

async function updateRole(member: ProjectMember, role: "ADMIN" | "MEMBER") {
  try {
    const { data } = await api.patch<{ success: boolean }>(
      `/projects/${projectId()}/members/${member.userId}`,
      { role },
    );
    if (data.success) {
      toast.success("已更新", "成员角色已修改");
      await fetchMembers();
    }
  } catch {
    toast.error("更新失败", "请稍后重试");
  }
}

async function removeMember(member: ProjectMember) {
  const isSelf = member.userId === currentUserId;
  try {
    const { data } = await api.delete<{ success: boolean }>(
      `/projects/${projectId()}/members/${member.userId}`,
    );
    if (data.success) {
      toast.success(
        isSelf ? "已退出" : "已移除",
        isSelf
          ? "你已退出该项目"
          : `成员 ${member.user.nickname || member.user.email} 已移除`,
      );
      await fetchMembers();
    }
  } catch {
    toast.error("操作失败", "请稍后重试");
  }
}

onMounted(async () => {
  await fetchOwner();
  await fetchMembers();
});
</script>

<template>
  <AppPage title="成员管理">
    <template #actions>
      <span class="badge badge-neutral">{{ members.length }}</span>
    </template>

    <div v-if="canManage" class="invite-section">
      <div class="invite-row">
        <UserSearchInput
          v-model="inviteForm.selectedUser"
          :suggestions="suggestions"
          :loading="searchLoading"
          class="invite-search"
          @search="searchUsers"
        />
        <AppSelect v-model="inviteForm.role" :options="inviteRoleOptions" />
        <button
          type="button"
          class="btn btn-primary"
          :disabled="!inviteForm.selectedUser || inviting"
          @click="invite"
        >
          <span v-if="inviting" class="loading loading-spinner loading-xs" />
          邀请
        </button>
      </div>
    </div>

    <div v-if="members.length > 5" class="member-search-bar">
      <input
        v-model="memberSearchQuery"
        type="search"
        class="input w-full"
        placeholder="筛选成员..."
      />
    </div>

    <div class="member-list">
      <div v-if="loading" class="list-state">
        <span class="loading loading-spinner loading-md" />
        <span>加载中...</span>
      </div>
      <div v-else-if="filteredMembers.length === 0" class="list-state">
        <span>{{ memberSearchQuery ? "没有匹配的成员" : "暂无成员" }}</span>
      </div>
      <div
        v-for="m in filteredMembers"
        v-else
        :key="m.id"
        class="member-row"
      >
        <UserAvatar :qq="m.user.qq" :size="38" />
        <div class="member-info">
          <div class="member-name">{{ m.user.nickname || m.user.email }}</div>
          <div class="member-sub">
            <span class="member-email">{{ m.user.email }}</span>
          </div>
        </div>

        <div class="member-right">
          <AppSelect
            v-if="canChangeRole && m.role !== 'OWNER'"
            :model-value="m.role"
            :options="roleChangeOptions"
            @update:model-value="updateRole(m, $event as 'ADMIN' | 'MEMBER')"
          />
          <span v-else class="badge" :class="roleBadgeClass(m.role)">
            {{ roleLabel[m.role] }}
          </span>

          <button
            v-if="canRemoveMembers && m.role !== 'OWNER' && m.userId !== currentUserId"
            type="button"
            class="btn btn-ghost btn-sm btn-square btn-error"
            title="移除成员"
            @click="removeMember(m)"
          >
            ✕
          </button>
          <button
            v-else-if="!isOwner && m.userId === currentUserId"
            type="button"
            class="btn btn-ghost btn-sm btn-error"
            @click="removeMember(m)"
          >
            退出
          </button>
        </div>
      </div>
    </div>
  </AppPage>
</template>

<style scoped>
.invite-section {
  padding-bottom: 1rem;
  margin-bottom: 1rem;
}

.invite-row {
  display: flex;
  gap: 0.5rem;
  align-items: center;
  flex-wrap: wrap;
}

.invite-search {
  flex: 1;
}

.member-search-bar {
  margin-bottom: 0.75rem;
}

.member-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.list-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 2rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  font-size: 0.875rem;
}

.member-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.65rem 0.75rem;
  border-radius: 8px;
  background: var(--color-base-100);
  transition: background 0.15s;
  flex-wrap: wrap;
}

.member-row:hover {
  background: color-mix(in oklab, var(--color-primary) 8%, var(--color-base-100));
}

.member-info {
  flex: 1;
  min-width: 140px;
}

.member-name {
  font-size: 0.9rem;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.member-sub {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  margin-top: 0.1rem;
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
  min-width: 0;
}

.member-email {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.member-right {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex-shrink: 0;
  margin-left: auto;
}

@media (max-width: 720px) {
  .member-right {
    margin-left: 0;
  }
}
</style>
