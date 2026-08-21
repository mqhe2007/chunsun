import { onMounted, onUnmounted, ref } from "vue";
import { api } from "@/utils/api";

const POLL_INTERVAL = 60 * 1000;

export function useUnreadNotifications() {
  const unreadCount = ref(0);
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  async function fetchUnreadCount() {
    try {
      const { data } = await api.get<{ success: boolean; data: { count: number } }>(
        "/notifications/unread-count",
      );
      if (data.success) unreadCount.value = data.data.count;
    } catch {
      // 角标失败时保持上次计数
    }
  }

  function onVisibilityChange() {
    if (document.visibilityState === "visible") {
      void fetchUnreadCount();
    }
  }

  onMounted(() => {
    void fetchUnreadCount();
    pollTimer = setInterval(() => {
      if (document.visibilityState === "visible") {
        void fetchUnreadCount();
      }
    }, POLL_INTERVAL);
    document.addEventListener("visibilitychange", onVisibilityChange);
  });

  onUnmounted(() => {
    if (pollTimer) clearInterval(pollTimer);
    document.removeEventListener("visibilitychange", onVisibilityChange);
  });

  return { unreadCount };
}
