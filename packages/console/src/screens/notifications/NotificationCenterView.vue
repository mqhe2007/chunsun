<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { AppColumn, AppPage, AppTable, useToast } from "@/ui";
import { api } from "@/utils/api";
import { formatRelativeTime } from "@/utils/time";

type NotificationItem = {
  id: string;
  type: string;
  title: string;
  body?: string;
  link?: string;
  isRead: boolean;
  readAt?: string;
  createdAt: string;
};

const router = useRouter();
const toast = useToast();

const notifications = ref<NotificationItem[]>([]);
const loading = ref(false);
const total = ref(0);
const page = ref(1);
const pageSize = ref(20);
const unreadOnly = ref(false);

async function fetchNotifications() {
  loading.value = true;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: NotificationItem[];
      meta: { total: number; page: number; pageSize: number; totalPages: number };
    }>("/notifications", {
      params: {
        page: page.value,
        pageSize: pageSize.value,
        unreadOnly: unreadOnly.value,
      },
    });
    if (data.success) {
      notifications.value = data.data;
      total.value = data.meta.total;
    }
  } catch {
    toast.error("加载失败", "无法加载通知列表");
  } finally {
    loading.value = false;
  }
}

watch([page, pageSize], fetchNotifications);

async function markAsRead(item: NotificationItem) {
  if (item.isRead) return;
  try {
    await api.patch(`/notifications/${item.id}/read`);
    item.isRead = true;
    item.readAt = new Date().toISOString();
  } catch {
    // ignore
  }
}

async function markAllAsRead() {
  try {
    await api.post("/notifications/read-all");
    notifications.value.forEach(n => {
      n.isRead = true;
      n.readAt = new Date().toISOString();
    });
    toast.success("已标记", "全部通知已标记为已读");
  } catch {
    toast.error("操作失败", "请稍后重试");
  }
}

function navigateTo(item: NotificationItem) {
  markAsRead(item);
  if (item.link) {
    router.push(item.link);
  }
}

function toggleUnreadOnly() {
  unreadOnly.value = !unreadOnly.value;
  page.value = 1;
  fetchNotifications();
}

onMounted(fetchNotifications);
</script>

<template>
  <AppPage title="通知中心">
    <template #actions>
      <button
        type="button"
        class="btn"
        :class="unreadOnly ? 'btn-soft btn-primary' : 'btn-ghost'"
        @click="toggleUnreadOnly"
      >
        {{ unreadOnly ? "显示全部" : "仅看未读" }}
      </button>
      <button type="button" class="btn btn-ghost" @click="markAllAsRead">全部已读</button>
    </template>

    <AppTable
      v-model:page="page"
      v-model:page-size="pageSize"
      :rows="notifications"
      :total="total"
      :loading="loading"
      empty="暂无通知"
      striped
      row-hover
    >
      <AppColumn header="通知">
        <template #default="{ row }">
          <div
            class="notification-row"
            :class="{ unread: !(row as NotificationItem).isRead }"
            @click="navigateTo(row as NotificationItem)"
          >
            <div class="notification-dot-wrap">
              <span v-if="!(row as NotificationItem).isRead" class="unread-dot" />
            </div>
            <div class="notification-content">
              <div class="notification-title">{{ (row as NotificationItem).title }}</div>
              <div v-if="(row as NotificationItem).body" class="notification-body">
                {{ (row as NotificationItem).body }}
              </div>
              <div class="notification-time">
                {{ formatRelativeTime((row as NotificationItem).createdAt) }}
              </div>
            </div>
          </div>
        </template>
      </AppColumn>
      <AppColumn header="操作" width="8.5rem">
        <template #default="{ row }">
          <button
            v-if="!(row as NotificationItem).isRead"
            type="button"
            class="btn btn-ghost btn-sm"
            @click.stop="markAsRead(row as NotificationItem)"
          >
            标记已读
          </button>
        </template>
      </AppColumn>
    </AppTable>
  </AppPage>
</template>

<style scoped>
.notification-row {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.5rem 0;
  cursor: pointer;
}

.notification-dot-wrap {
  width: 10px;
  flex-shrink: 0;
  padding-top: 0.35rem;
}

.unread-dot {
  display: block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--color-primary);
}

.notification-content {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.notification-title {
  font-weight: 500;
}

.notification-row.unread .notification-title {
  font-weight: 600;
}

.notification-body {
  font-size: 0.875rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

.notification-time {
  font-size: 0.75rem;
  color: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}
</style>
