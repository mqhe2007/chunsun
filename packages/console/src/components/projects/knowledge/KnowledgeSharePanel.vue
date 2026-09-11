<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { api } from "@/utils/api";
import { AppField, useToast } from "@/ui";

const props = defineProps<{
  projectId: string;
  docId: string;
  /** 系统文档不可分享 */
  shareable: boolean;
}>();

const toast = useToast();
const open = defineModel<boolean>("open", { default: false });

const loading = ref(false);
const saving = ref(false);
const hasToken = ref(false);
const enabled = ref(false);
const active = ref(false);
const expiresAt = ref<string | null>(null);
const shareUrl = ref("");
const expirePreset = ref<"none" | "7d" | "30d" | "custom">("none");
const customExpires = ref("");

const shareApi = computed(
  () => `/projects/${props.projectId}/knowledge/documents/${props.docId}/share`,
);

const needsRotateForUrl = computed(() => hasToken.value && !shareUrl.value);

function expiresIsoFromPreset(): string | null {
  if (expirePreset.value === "none") return null;
  if (expirePreset.value === "custom") {
    if (!customExpires.value) return null;
    return new Date(customExpires.value).toISOString();
  }
  const days = expirePreset.value === "7d" ? 7 : 30;
  return new Date(Date.now() + days * 24 * 60 * 60 * 1000).toISOString();
}

function applyStatus(data: {
  hasToken: boolean;
  enabled: boolean;
  active?: boolean;
  expiresAt: string | null;
  url?: string | null;
}) {
  hasToken.value = data.hasToken;
  enabled.value = data.enabled;
  active.value = Boolean(data.active);
  expiresAt.value = data.expiresAt;
  shareUrl.value = data.url?.trim() || "";
}

async function loadStatus() {
  if (!props.shareable) return;
  loading.value = true;
  try {
    const { data } = await api.get<{
      success: boolean;
      data: {
        hasToken: boolean;
        enabled: boolean;
        active?: boolean;
        expiresAt: string | null;
        url?: string | null;
      };
    }>(shareApi.value);
    if (!data.success) throw new Error("load failed");
    applyStatus(data.data);
  } catch {
    toast.error("加载分享状态失败");
  } finally {
    loading.value = false;
  }
}

async function enableOrRotate() {
  saving.value = true;
  try {
    const expires = expiresIsoFromPreset();
    const { data } = await api.post<{
      success: boolean;
      data: {
        token: string;
        url: string;
        enabled: boolean;
        expiresAt: string | null;
        active: boolean;
      };
    }>(shareApi.value, { expiresAt: expires });
    if (!data.success) throw new Error("create failed");
    applyStatus({
      hasToken: true,
      enabled: data.data.enabled,
      active: data.data.active,
      expiresAt: data.data.expiresAt,
      url: data.data.url,
    });
    toast.success("已生成分享链接");
  } catch {
    toast.error("生成失败");
  } finally {
    saving.value = false;
  }
}

async function setEnabled(next: boolean) {
  if (next && !hasToken.value) {
    await enableOrRotate();
    return;
  }
  saving.value = true;
  try {
    if (!next) {
      const { data } = await api.delete<{
        success: boolean;
        data: {
          hasToken: boolean;
          enabled: boolean;
          active?: boolean;
          expiresAt: string | null;
          url?: string | null;
        };
      }>(shareApi.value);
      if (!data.success) throw new Error("disable failed");
      applyStatus(data.data);
      toast.success("已停用分享");
    } else {
      const { data } = await api.patch<{
        success: boolean;
        data: {
          hasToken: boolean;
          enabled: boolean;
          active?: boolean;
          expiresAt: string | null;
          url?: string | null;
        };
      }>(shareApi.value, { enabled: true });
      if (!data.success) throw new Error("enable failed");
      applyStatus(data.data);
      toast.success("已重新启用");
    }
  } catch {
    toast.error("更新失败");
  } finally {
    saving.value = false;
  }
}

async function onToggleChange(ev: Event) {
  const input = ev.target as HTMLInputElement;
  const next = input.checked;
  const prev = !next;
  await setEnabled(next);
  if (enabled.value !== next) {
    input.checked = prev;
  }
}

async function copyUrl() {
  if (!shareUrl.value) {
    toast.warn(
      needsRotateForUrl.value
        ? "旧链接无法恢复，请重新生成一次"
        : "请先生成分享链接",
    );
    return;
  }
  try {
    await navigator.clipboard.writeText(shareUrl.value);
    toast.success("已复制");
  } catch {
    toast.error("复制失败");
  }
}

watch(open, v => {
  if (v) void loadStatus();
});

onMounted(() => {
  if (open.value) void loadStatus();
});
</script>

<template>
  <dialog class="modal" :class="{ 'modal-open': open }">
    <div class="modal-box max-w-lg">
      <h3 class="text-lg font-semibold">分享文档</h3>
      <p v-if="!shareable" class="mt-2 text-sm text-base-content/60">
        系统固定文档（宪法 / 项目记忆）不支持对外分享。
      </p>
      <template v-else>
        <p class="mt-2 text-sm text-base-content/60">
          打开「启用分享」即生成只读公开链接；可随时停用或重新生成（旧链接立即失效）。
        </p>
        <div v-if="loading" class="mt-4 flex justify-center py-6">
          <span class="loading loading-spinner" />
        </div>
        <div v-else class="mt-4 flex flex-col gap-4">
          <div class="flex items-center justify-between gap-3">
            <span class="text-sm">启用分享</span>
            <input
              type="checkbox"
              class="toggle"
              :checked="enabled"
              :disabled="saving"
              @change="onToggleChange"
            />
          </div>
          <p v-if="hasToken" class="text-xs text-base-content/50">
            状态：{{ active ? "有效" : "已停用或已过期" }}
            <template v-if="expiresAt"> · 过期 {{ new Date(expiresAt).toLocaleString() }}</template>
          </p>

          <AppField
            v-if="shareUrl"
            label="当前链接"
            hint="项目成员可随时查看与复制；对外仍为只读访问。"
          >
            <div class="flex gap-2">
              <input
                type="text"
                class="input input-bordered w-full font-mono text-xs"
                :value="shareUrl"
                readonly
              />
              <button type="button" class="btn btn-ghost shrink-0" @click="copyUrl">
                复制
              </button>
            </div>
          </AppField>
          <p
            v-else-if="needsRotateForUrl"
            class="rounded-box border border-warning/40 bg-warning/10 px-3 py-2 text-sm text-warning"
          >
            此分享生成于升级前，链接无法回显。请点「重新生成链接」一次即可恢复查看。
          </p>

          <AppField label="过期（生成/轮换时生效）" html-for="share-expire">
            <select
              id="share-expire"
              v-model="expirePreset"
              class="select select-bordered w-full"
            >
              <option value="none">永不过期</option>
              <option value="7d">7 天</option>
              <option value="30d">30 天</option>
              <option value="custom">自定义</option>
            </select>
          </AppField>
          <AppField
            v-if="expirePreset === 'custom'"
            label="自定义过期时间"
            html-for="share-expire-custom"
          >
            <input
              id="share-expire-custom"
              v-model="customExpires"
              type="datetime-local"
              class="input input-bordered w-full"
            />
          </AppField>
        </div>
      </template>
      <div class="modal-action flex-wrap">
        <button type="button" class="btn btn-ghost" @click="open = false">关闭</button>
        <template v-if="shareable">
          <button
            type="button"
            class="btn btn-ghost"
            :disabled="!shareUrl"
            @click="copyUrl"
          >
            复制链接
          </button>
          <button
            type="button"
            class="btn btn-primary"
            :disabled="saving"
            @click="enableOrRotate"
          >
            <span v-if="saving" class="loading loading-spinner loading-sm" />
            {{ hasToken ? "重新生成链接" : "生成链接" }}
          </button>
        </template>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop" @submit.prevent="open = false">
      <button type="submit">close</button>
    </form>
  </dialog>
</template>
