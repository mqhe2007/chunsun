<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import { extractToc, renderMarkdown } from "@/utils/markdown";
import "@/assets/markdown.css";

const route = useRoute();
const token = computed(() => (route.params as Record<string, string>).token ?? "");

const loading = ref(true);
const error = ref(false);
const title = ref("");
const content = ref("");
const projectName = ref("");
const updatedAt = ref<string | null>(null);

const html = computed(() => renderMarkdown(content.value));
const toc = computed(() => extractToc(content.value));

function scrollTo(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
}

onMounted(async () => {
  loading.value = true;
  error.value = false;
  try {
    const res = await fetch(`/api/v1/public/knowledge/shares/${encodeURIComponent(token.value)}`);
    const body = (await res.json()) as {
      success?: boolean;
      data?: {
        title: string;
        content: string;
        projectName?: string;
        updatedAt?: string;
      };
    };
    if (!res.ok || !body.success || !body.data) {
      error.value = true;
      return;
    }
    title.value = body.data.title;
    content.value = body.data.content ?? "";
    projectName.value = body.data.projectName ?? "";
    updatedAt.value = body.data.updatedAt ?? null;
  } catch {
    error.value = true;
  } finally {
    loading.value = false;
  }
});
</script>

<template>
  <div class="share-page">
    <div class="site-rail share-rail">
      <header class="share-header">
        <p class="eyebrow">春笋 · 公开文档</p>
        <template v-if="!loading && !error">
          <h1 class="share-title">{{ title }}</h1>
          <p class="meta">
            <span v-if="projectName">{{ projectName }}</span>
            <span v-if="updatedAt"> · 更新 {{ new Date(updatedAt).toLocaleString() }}</span>
          </p>
        </template>
      </header>

      <div v-if="loading" class="state">加载中…</div>
      <div v-else-if="error" class="state error">
        <h2>链接无效或已失效</h2>
        <p>请向文档所有者确认分享是否仍启用、是否过期。</p>
      </div>
      <div
        v-else
        class="reader"
        :class="{ 'reader--no-toc': toc.length === 0 }"
      >
        <article class="share-article markdown-body" v-html="html" />
        <aside v-if="toc.length" class="toc">
          <p class="toc-title">本页目录</p>
          <button
            v-for="item in toc"
            :key="item.id"
            type="button"
            class="toc-item"
            :style="{ paddingLeft: `${(item.level - 1) * 0.65 + 0.35}rem` }"
            @click="scrollTo(item.id)"
          >
            {{ item.text }}
          </button>
        </aside>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 宽度对齐营销/文档：site-rail → --site-rail-max 1120px（tokens.css） */
.share-page {
  min-height: 100vh;
  padding-top: calc(4.4rem + 2rem);
  padding-bottom: 5rem;
  background:
    radial-gradient(1200px 480px at 85% -10%, color-mix(in srgb, var(--chunsun-tip) 9%, transparent), transparent 70%),
    var(--chunsun-fog);
  color: var(--chunsun-ink);
}

.share-rail {
  display: flex;
  flex-direction: column;
  gap: 1.75rem;
}

.share-header {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.eyebrow {
  margin: 0;
  font-size: 0.8rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--chunsun-node);
}

.share-title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1.3;
  color: var(--chunsun-ink);
}

.meta {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.4;
  color: var(--chunsun-ink-muted);
}

.state {
  padding: 3rem 1rem;
  text-align: center;
  color: var(--chunsun-ink-muted);
}

.state.error h2 {
  margin: 0 0 0.5rem;
  font-size: 1.25rem;
  color: var(--chunsun-ink);
}

/* 与文档页一致：正文 + 右侧 TOC；无 TOC 时正文限宽居中 */
.reader {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(11rem, 12rem);
  gap: 1.75rem;
  align-items: start;
}

.reader--no-toc {
  grid-template-columns: minmax(0, 1fr);
}

.reader--no-toc .share-article {
  max-width: 52rem;
  margin-inline: auto;
}

.share-article {
  min-width: 0;
  padding: 1.25rem 1.35rem;
  border-radius: 0.9rem;
  background: color-mix(in srgb, white 78%, var(--chunsun-fog));
  border: 1px solid color-mix(in srgb, var(--chunsun-rain) 20%, transparent);
  box-shadow: 0 1px 2px color-mix(in srgb, var(--chunsun-ink) 5%, transparent);
}

.toc {
  position: sticky;
  top: calc(4.4rem + 1.25rem);
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  padding: 0.85rem 0.75rem;
  border-radius: 0.75rem;
  background: color-mix(in srgb, white 62%, var(--chunsun-fog));
  border: 1px solid color-mix(in srgb, var(--chunsun-rain) 16%, transparent);
}

.toc-title {
  margin: 0 0 0.4rem;
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--chunsun-rain);
}

.toc-item {
  appearance: none;
  border: 0;
  background: transparent;
  text-align: left;
  font: inherit;
  font-size: 0.82rem;
  line-height: 1.35;
  color: var(--chunsun-ink-muted);
  cursor: pointer;
  padding: 0.3rem 0.4rem;
  border-radius: 0.4rem;
}

.toc-item:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 7%, transparent);
  color: var(--chunsun-ink);
}

@media (max-width: 900px) {
  .reader {
    grid-template-columns: 1fr;
  }

  .toc {
    position: static;
    order: -1;
  }
}
</style>
