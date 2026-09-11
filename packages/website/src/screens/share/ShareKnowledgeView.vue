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
    <header class="share-header">
      <p class="eyebrow">春笋 · 公开文档</p>
      <template v-if="!loading && !error">
        <h1>{{ title }}</h1>
        <p class="meta">
          <span v-if="projectName">{{ projectName }}</span>
          <span v-if="updatedAt"> · 更新 {{ new Date(updatedAt).toLocaleString() }}</span>
        </p>
      </template>
    </header>

    <main class="share-main">
      <div v-if="loading" class="state">加载中…</div>
      <div v-else-if="error" class="state error">
        <h2>链接无效或已失效</h2>
        <p>请向文档所有者确认分享是否仍启用、是否过期。</p>
      </div>
      <div v-else class="reader">
        <aside v-if="toc.length" class="toc">
          <p class="toc-title">目录</p>
          <button
            v-for="item in toc"
            :key="item.id"
            type="button"
            class="toc-item"
            :style="{ paddingLeft: `${(item.level - 1) * 0.75 + 0.25}rem` }"
            @click="scrollTo(item.id)"
          >
            {{ item.text }}
          </button>
        </aside>
        <article class="markdown-body" v-html="html" />
      </div>
    </main>
  </div>
</template>

<style scoped>
.share-page {
  min-height: 100vh;
  background:
    radial-gradient(1200px 500px at 10% -10%, color-mix(in oklab, #0d6e4f 16%, transparent), transparent),
    linear-gradient(180deg, #f7faf8 0%, #eef3f0 100%);
  color: #1a2e24;
}

.share-header {
  max-width: 56rem;
  margin: 0 auto;
  padding: 2.5rem 1.25rem 1rem;
}

.eyebrow {
  margin: 0 0 0.5rem;
  font-size: 0.8rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: color-mix(in oklab, #0d6e4f 70%, black);
}

.share-header h1 {
  margin: 0;
  font-size: clamp(1.5rem, 3vw, 2rem);
  line-height: 1.25;
  font-weight: 700;
}

.meta {
  margin: 0.55rem 0 0;
  font-size: 0.9rem;
  color: color-mix(in oklab, #1a2e24 55%, white);
}

.share-main {
  max-width: 56rem;
  margin: 0 auto;
  padding: 0 1.25rem 3rem;
}

.state {
  padding: 3rem 1rem;
  text-align: center;
  color: color-mix(in oklab, #1a2e24 60%, white);
}

.state.error h2 {
  margin: 0 0 0.5rem;
  font-size: 1.25rem;
}

.reader {
  display: grid;
  gap: 1.25rem;
}

@media (min-width: 900px) {
  .reader {
    grid-template-columns: 12rem minmax(0, 1fr);
    align-items: start;
  }
}

.toc {
  position: sticky;
  top: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 0.75rem;
  border-radius: 0.75rem;
  background: color-mix(in oklab, white 80%, transparent);
  border: 1px solid color-mix(in oklab, #1a2e24 10%, transparent);
}

.toc-title {
  margin: 0 0 0.35rem;
  font-size: 0.75rem;
  font-weight: 600;
  color: color-mix(in oklab, #1a2e24 50%, white);
}

.toc-item {
  appearance: none;
  border: 0;
  background: transparent;
  text-align: left;
  font: inherit;
  font-size: 0.82rem;
  line-height: 1.35;
  color: inherit;
  cursor: pointer;
  padding: 0.25rem 0.2rem;
  border-radius: 0.35rem;
}

.toc-item:hover {
  background: color-mix(in oklab, #0d6e4f 10%, transparent);
}

.markdown-body {
  padding: 1.25rem 1.35rem;
  border-radius: 0.9rem;
  background: color-mix(in oklab, white 88%, transparent);
  border: 1px solid color-mix(in oklab, #1a2e24 10%, transparent);
  box-shadow: 0 10px 30px color-mix(in oklab, #1a2e24 6%, transparent);
}
</style>
