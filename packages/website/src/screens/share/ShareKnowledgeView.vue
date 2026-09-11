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

const updatedLabel = computed(() => {
  if (!updatedAt.value) return "";
  try {
    return new Date(updatedAt.value).toLocaleString();
  } catch {
    return "";
  }
});

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
  <div class="site-theme share-page">
    <div class="site-rail share-rail">
      <!-- 独立临时页：无站点导航；轻量出处 + 文档主标题 + 元信息分层 -->
      <header v-if="!loading && !error" class="doc-masthead">
        <h1 class="doc-title">{{ title }}</h1>
        <dl class="doc-meta">
          <div v-if="projectName" class="meta-item">
            <dt>来自项目</dt>
            <dd>{{ projectName }}</dd>
          </div>
          <div v-if="updatedLabel" class="meta-item">
            <dt>最近更新</dt>
            <dd>{{ updatedLabel }}</dd>
          </div>
        </dl>
      </header>

      <div v-if="loading" class="state">加载中…</div>
      <div v-else-if="error" class="state error">
        <p class="error-kicker">分享链接</p>
        <h1 class="error-title">无法打开此文档</h1>
        <p class="error-lead">链接可能已停用、过期或无效。请向文档所有者确认后重试。</p>
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

      <footer class="share-foot">
        <span>春笋</span>
        <span class="foot-sep" aria-hidden="true">·</span>
        <span>公开只读副本，内容以项目内最新版为准</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
/* 宽度仍用站点 site-rail（1120px）；无站点头，偏临时页 */
.share-page {
  min-height: 100vh;
  padding-block: 2.5rem 3.5rem;
  background:
    radial-gradient(900px 420px at 12% -8%, color-mix(in srgb, var(--chunsun-tip) 10%, transparent), transparent 65%),
    var(--chunsun-fog);
  color: var(--chunsun-ink);
}

.share-rail {
  display: flex;
  flex-direction: column;
  gap: 1.75rem;
}

.doc-masthead {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  padding-bottom: 1.1rem;
  border-bottom: 1px solid color-mix(in srgb, var(--chunsun-rain) 18%, transparent);
}

.doc-title {
  margin: 0;
  font-size: clamp(1.65rem, 2.4vw, 2.05rem);
  font-weight: 700;
  line-height: 1.25;
  letter-spacing: -0.02em;
  color: var(--chunsun-ink);
}

.doc-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 0.35rem 1.15rem;
  margin: 0;
}

.meta-item {
  display: inline-flex;
  flex-direction: row;
  flex-wrap: nowrap;
  align-items: baseline;
  gap: 0.35rem;
  min-width: 0;
}

.meta-item dt {
  margin: 0;
  font-size: 0.8rem;
  font-weight: 500;
  color: var(--chunsun-rain);
}

.meta-item dd {
  margin: 0;
  font-size: 0.8rem;
  color: var(--chunsun-ink-muted);
}

.meta-item + .meta-item {
  position: relative;
}

.meta-item + .meta-item::before {
  content: "·";
  position: absolute;
  left: -0.7rem;
  color: var(--chunsun-rain);
  opacity: 0.7;
}

.state {
  padding: 3.5rem 1rem;
  text-align: center;
  color: var(--chunsun-ink-muted);
}

.error-kicker {
  margin: 0 0 0.5rem;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--chunsun-rain);
}

.error-title {
  margin: 0 0 0.55rem;
  font-size: 1.45rem;
  color: var(--chunsun-ink);
}

.error-lead {
  margin: 0 auto;
  max-width: 28rem;
  font-size: 0.95rem;
  line-height: 1.5;
}

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
  padding: 1.35rem 1.45rem;
  border-radius: 0.9rem;
  background: color-mix(in srgb, white 78%, var(--chunsun-fog));
  border: 1px solid color-mix(in srgb, var(--chunsun-rain) 20%, transparent);
  box-shadow: 0 1px 2px color-mix(in srgb, var(--chunsun-ink) 5%, transparent);
}

.toc {
  position: sticky;
  top: 1.25rem;
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

.share-foot {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  padding-top: 0.5rem;
  font-size: 0.78rem;
  color: var(--chunsun-rain);
}

.foot-sep {
  opacity: 0.7;
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
