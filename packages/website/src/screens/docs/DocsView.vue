<script setup lang="ts">
/**
 * 春笋文档站主视图：左栏主导航 + 中间正文 + 右栏本页导航。
 * 路由：/docs?doc=<slug>（query 驱动，URL 可分享、刷新可恢复）。
 */
import { ChevronDown, ChevronRight, CircleCheck, Copy, Info, TriangleAlert } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useSiteToast } from "@/composables/useSiteToast";
import {
  docCategories,
  findDoc,
  defaultDocSlug,
  type DocBlock,
  type DocCategory,
} from "./docs-data";
import { resolveDocIcon } from "./doc-icons";

const route = useRoute();
const router = useRouter();

/* ── 当前文档 ── */
const currentSlug = computed(() => {
  const q = route.query.doc;
  const slug = typeof q === "string" && q ? q : defaultDocSlug;
  return findDoc(slug) ? slug : defaultDocSlug;
});

const currentEntry = computed(() => findDoc(currentSlug.value)!);
const currentCategoryKey = computed(() => currentEntry.value.category.key);

function selectDoc(slug: string) {
  if (slug === currentSlug.value) return;
  router.push({ path: "/docs", query: { doc: slug } });
}

/* ── 目录分组折叠（默认展开当前文档所在组） ── */
const collapsed = ref<Set<string>>(new Set());

watch(
  currentCategoryKey,
  (key) => {
    collapsed.value.delete(key);
  },
  { immediate: true },
);

function isCollapsed(category: DocCategory) {
  return collapsed.value.has(category.key);
}

function toggleCategory(category: DocCategory) {
  const next = new Set(collapsed.value);
  if (next.has(category.key)) next.delete(category.key);
  else next.add(category.key);
  collapsed.value = next;
}

/* ── 文档内目录（TOC，仅当前页，右栏展示） ── */
type TocItem = { text: string; blockIndex: number };

const toc = computed<TocItem[]>(() =>
  currentEntry.value.doc.blocks
    .map((block, index) => ({ block, index }))
    .filter(
      (entry): entry is { block: Extract<DocBlock, { t: "h2" }>; index: number } =>
        entry.block.t === "h2",
    )
    .map(({ block, index }) => ({ text: block.text, blockIndex: index })),
);

const hasToc = computed(() => toc.value.length > 0);

/* 当前高亮标题（滚动监听） */
const activeHeadingIndex = ref(-1);

function headingId(blockIndex: number) {
  return `doc-h2-${blockIndex}`;
}

function scrollToHeading(blockIndex: number) {
  document
    .getElementById(headingId(blockIndex))
    ?.scrollIntoView({ behavior: "smooth", block: "start" });
}

/** 固定顶栏高度 + 呼吸空间，作为「当前标题」判定线 */
const HEADING_OFFSET = 150;

function updateActiveHeading() {
  const items = toc.value;
  if (!items.length) {
    if (activeHeadingIndex.value !== -1) activeHeadingIndex.value = -1;
    return;
  }

  // 已滚到底：短页最后一个标题永远到不了判定线，直接高亮最后一项
  const atBottom =
    window.scrollY + window.innerHeight >= document.documentElement.scrollHeight - 2;
  if (atBottom) {
    if (activeHeadingIndex.value !== items.length - 1) {
      activeHeadingIndex.value = items.length - 1;
    }
    return;
  }

  const y = window.scrollY + HEADING_OFFSET;
  let active = -1;
  for (let i = 0; i < items.length; i++) {
    const el = document.getElementById(headingId(items[i].blockIndex));
    if (!el) continue;
    if (el.getBoundingClientRect().top + window.scrollY <= y) active = i;
    else break;
  }
  if (active !== activeHeadingIndex.value) activeHeadingIndex.value = active;
}

onMounted(() => {
  updateActiveHeading();
  window.addEventListener("scroll", updateActiveHeading, { passive: true });
  window.addEventListener("resize", updateActiveHeading, { passive: true });
});

onBeforeUnmount(() => {
  window.removeEventListener("scroll", updateActiveHeading);
  window.removeEventListener("resize", updateActiveHeading);
});

/* 切换文档：回到顶部并重算高亮 */
watch(currentSlug, () => {
  activeHeadingIndex.value = -1;
  window.scrollTo({ top: 0 });
  void nextTick(updateActiveHeading);
});

/* ── 渲染辅助 ── */
const toast = useSiteToast();

/**
 * 渲染文本：把数据中的 `<实例>` 占位符替换为当前部署实例的真实 origin。
 * 例如 curl -fsSL <实例>/cli/install.sh | sh → curl -fsSL http://<origin>/cli/install.sh | sh
 */
function renderText(text: string) {
  return text.replace(/<实例>/g, window.location.origin);
}

function copyCode(text: string) {
  navigator.clipboard
    .writeText(text)
    .then(() => {
      toast.add({ severity: "success", summary: "已复制", life: 2000 });
    })
    .catch(() => {
      toast.add({
        severity: "warn",
        summary: "复制失败",
        detail: "请手动复制",
        life: 2500,
      });
    });
}
</script>

<template>
  <div class="docs-page">
    <div class="site-rail docs-rail">
      <!-- 页面头：次级页面样式（标题 + 一行说明） -->
      <header class="docs-header">
        <h1 class="docs-title">使用文档</h1>
        <p class="docs-lead">从接入到验收，一个功能一篇文档。</p>
      </header>

      <div class="docs-body" :class="{ 'docs-body--no-toc': !hasToc }">
        <!-- 左栏：主导航（类目 + 文档） -->
        <aside class="docs-sidebar" aria-label="文档目录">
          <nav class="docs-nav">
            <div v-for="category in docCategories" :key="category.key" class="docs-category">
              <button
                type="button"
                class="docs-category-head"
                :class="{ active: currentCategoryKey === category.key }"
                :aria-expanded="!isCollapsed(category)"
                @click="toggleCategory(category)"
              >
                <component
                  :is="resolveDocIcon(category.icon)"
                  class="docs-category-icon"
                  :size="16"
                  aria-hidden="true"
                />
                <span class="docs-category-label">{{ category.label }}</span>
                <ChevronRight
                  v-if="isCollapsed(category)"
                  class="docs-category-caret"
                  :size="14"
                  aria-hidden="true"
                />
                <ChevronDown
                  v-else
                  class="docs-category-caret"
                  :size="14"
                  aria-hidden="true"
                />
              </button>
              <ul v-show="!isCollapsed(category)" class="docs-category-items">
                <li v-for="doc in category.docs" :key="doc.slug">
                  <a
                    class="docs-nav-item"
                    :class="{ active: currentSlug === doc.slug }"
                    :href="`/docs?doc=${doc.slug}`"
                    @click.prevent="selectDoc(doc.slug)"
                  >
                    <component
                      :is="resolveDocIcon(doc.icon)"
                      class="docs-nav-icon"
                      :size="15"
                      aria-hidden="true"
                    />
                    <span class="docs-nav-title">{{ doc.title }}</span>
                  </a>
                </li>
              </ul>
            </div>
          </nav>
        </aside>

        <!-- 中间：正文 -->
        <main class="docs-content">
          <article class="docs-article">
            <header class="docs-article-head">
              <div class="docs-article-icon" aria-hidden="true">
                <component :is="resolveDocIcon(currentEntry.doc.icon)" :size="22" />
              </div>
              <div>
                <h2 class="docs-article-title">{{ currentEntry.doc.title }}</h2>
                <p class="docs-article-desc">{{ currentEntry.doc.desc }}</p>
              </div>
            </header>

            <div
              v-for="(block, i) in currentEntry.doc.blocks"
              :key="i"
              class="docs-block"
            >
              <!-- 段落 -->
              <p v-if="block.t === 'p'" class="docs-p">{{ renderText(block.text) }}</p>

              <!-- 二级标题 -->
              <h3 v-else-if="block.t === 'h2'" :id="headingId(i)" class="docs-h2">
                {{ renderText(block.text) }}
              </h3>

              <!-- 三级标题 -->
              <h4 v-else-if="block.t === 'h3'" class="docs-h3">{{ renderText(block.text) }}</h4>

              <!-- 无序列表 -->
              <ul v-else-if="block.t === 'ul'" class="docs-list">
                <li v-for="(item, j) in block.items" :key="j">{{ renderText(item) }}</li>
              </ul>

              <!-- 有序列表 -->
              <ol v-else-if="block.t === 'ol'" class="docs-list docs-list-ordered">
                <li v-for="(item, j) in block.items" :key="j">{{ renderText(item) }}</li>
              </ol>

              <!-- 代码块 -->
              <div v-else-if="block.t === 'code'" class="docs-code-wrap">
                <div class="docs-code-bar">
                  <span class="docs-code-lang">{{ block.lang ?? "text" }}</span>
                  <button
                    type="button"
                    class="docs-code-copy"
                    aria-label="复制代码"
                    @click="copyCode(block.code)"
                  >
                    <Copy :size="14" aria-hidden="true" />
                    复制
                  </button>
                </div>
                <pre class="docs-code"><code>{{ renderText(block.code) }}</code></pre>
              </div>

              <!-- 提示 -->
              <div v-else-if="block.t === 'note'" class="docs-note" :class="`note-${block.kind}`">
                <component
                  :is="block.kind === 'info' ? Info : block.kind === 'success' ? CircleCheck : TriangleAlert"
                  class="docs-note-icon"
                  :size="16"
                  aria-hidden="true"
                />
                <span class="docs-note-text">{{ renderText(block.text) }}</span>
              </div>

              <!-- 表格 -->
              <div v-else-if="block.t === 'table'" class="docs-table-wrap">
                <table class="docs-table">
                  <thead>
                    <tr>
                      <th v-for="(head, j) in block.head" :key="j">{{ renderText(head) }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="(row, r) in block.rows" :key="r">
                      <td v-for="(cell, c) in row" :key="c">{{ renderText(cell) }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </article>
        </main>

        <!-- 右栏：本页导航 -->
        <aside v-if="hasToc" class="docs-toc-panel" aria-label="本页目录">
          <div class="docs-toc">
            <p class="docs-toc-title">本页目录</p>
            <a
              v-for="(heading, i) in toc"
              :key="heading.blockIndex"
              class="docs-toc-item"
              :class="{ active: activeHeadingIndex === i }"
              href="#"
              @click.prevent="scrollToHeading(heading.blockIndex)"
            >
              {{ heading.text }}
            </a>
          </div>
        </aside>
      </div>
    </div>
  </div>
</template>

<style scoped>
.docs-page {
  /* 内容宽度对齐营销页规范：--site-rail-max 1120px（见 tokens.css），不单独放宽 */
  min-height: 100vh;
  padding-top: calc(4.4rem + 2rem);
  padding-bottom: 5rem;
  background:
    radial-gradient(1200px 480px at 85% -10%, color-mix(in srgb, var(--chunsun-tip) 9%, transparent), transparent 70%),
    var(--chunsun-fog);
}

.docs-rail {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

/* ── 页面头（次级页面样式：对齐控制台 console-page 观感） ── */
.docs-header {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.docs-title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1.3;
  color: var(--chunsun-ink);
}

.docs-lead {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.4;
  color: var(--chunsun-ink-muted);
}

/* ── 主体三栏：左主导航 / 中正文 / 右本页导航 ── */
.docs-body {
  display: grid;
  /* 左栏/右栏按内容收窄（最长文档标题约 10.5rem、TOC 最长标题约 11rem） */
  grid-template-columns: minmax(10.5rem, 12rem) minmax(0, 1fr) minmax(11rem, 12rem);
  gap: 1.75rem;
  align-items: start;
}

/* 无二级标题的短文档：退化为两栏 */
.docs-body--no-toc {
  grid-template-columns: minmax(10.5rem, 12rem) minmax(0, 1fr);
  gap: 1.75rem;
}

.docs-body--no-toc .docs-article {
  max-width: 52rem;
  margin-inline: auto;
}

/* ── 左栏：主导航 ── */
.docs-sidebar {
  position: sticky;
  top: calc(4.4rem + 1.25rem);
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.docs-category + .docs-category {
  margin-top: 0.25rem;
}

.docs-category-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.5rem 0.6rem;
  border: none;
  border-radius: 9px;
  background: transparent;
  color: var(--chunsun-ink);
  font-size: 0.9rem;
  font-weight: 700;
  cursor: pointer;
  text-align: left;
  transition: background 0.16s ease, color 0.16s ease;
}

.docs-category-head:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 8%, transparent);
}

.docs-category-head.active {
  color: var(--chunsun-node);
}

.docs-category-icon {
  flex-shrink: 0;
  color: var(--chunsun-node);
}

.docs-category-caret {
  margin-left: auto;
  flex-shrink: 0;
  color: var(--chunsun-rain);
}

.docs-category-items {
  margin: 0.2rem 0 0.5rem;
  padding: 0;
  list-style: none;
}

.docs-nav-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.42rem 0.6rem 0.42rem 1.55rem;
  border-radius: 8px;
  font-size: 0.87rem;
  line-height: 1.35;
  color: var(--chunsun-ink-muted);
  text-decoration: none;
  transition: background 0.16s ease, color 0.16s ease;
}

.docs-nav-item:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 7%, transparent);
  color: var(--chunsun-ink);
}

.docs-nav-item.active {
  background: color-mix(in srgb, var(--chunsun-shoot) 12%, transparent);
  color: var(--chunsun-node);
  font-weight: 600;
}

.docs-nav-icon {
  opacity: 0.85;
  flex-shrink: 0;
}

/* ── 右栏：本页导航 ── */
.docs-toc-panel {
  position: sticky;
  top: calc(4.4rem + 1.25rem);
  max-height: calc(100vh - 4.4rem - 2.5rem);
  overflow-y: auto;
  /* 仅在三栏模式下贴近正文：1.75rem 间距 − 0.5rem 负外边距 = 视觉 1.25rem */
  margin-left: -0.5rem;
}

.docs-toc {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  padding: 1rem 1.1rem;
  border: 1px solid color-mix(in srgb, var(--chunsun-rain) 16%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, white 62%, var(--chunsun-fog));
}

.docs-toc-title {
  margin: 0 0 0.5rem;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--chunsun-rain);
}

.docs-toc-item {
  display: block;
  padding: 0.32rem 0.55rem;
  margin-inline: -0.55rem;
  border-radius: 7px;
  font-size: 0.84rem;
  line-height: 1.4;
  color: var(--chunsun-ink-muted);
  text-decoration: none;
  transition: background 0.15s ease, color 0.15s ease;
}

.docs-toc-item:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 7%, transparent);
  color: var(--chunsun-ink);
}

.docs-toc-item.active {
  color: var(--chunsun-node);
  font-weight: 600;
}

/* ── 中间：正文 ── */
.docs-article {
  background: color-mix(in srgb, white 78%, var(--chunsun-fog));
  border: 1px solid color-mix(in srgb, var(--chunsun-rain) 20%, transparent);
  border-radius: 16px;
  padding: 2.2rem clamp(1.4rem, 3vw, 2.6rem);
  box-shadow: 0 1px 2px color-mix(in srgb, var(--chunsun-ink) 5%, transparent);
}

.docs-article-head {
  display: flex;
  align-items: flex-start;
  gap: 0.9rem;
  padding-bottom: 1.4rem;
  margin-bottom: 1.4rem;
  border-bottom: 1px solid color-mix(in srgb, var(--chunsun-rain) 18%, transparent);
}

.docs-article-icon {
  display: grid;
  place-items: center;
  width: 2.5rem;
  height: 2.5rem;
  flex-shrink: 0;
  border-radius: 12px;
  background: color-mix(in srgb, var(--chunsun-shoot) 13%, white);
  color: var(--chunsun-node);
  font-size: 1.1rem;
}

.docs-article-title {
  margin: 0;
  font-size: 1.35rem;
  font-weight: 800;
  letter-spacing: -0.015em;
  color: var(--chunsun-ink);
}

.docs-article-desc {
  margin: 0.3rem 0 0;
  font-size: 0.92rem;
  line-height: 1.55;
  color: var(--chunsun-ink-muted);
}

/* 内容块 */
.docs-block + .docs-block {
  margin-top: 1rem;
}

.docs-p {
  margin: 0;
  font-size: 0.96rem;
  line-height: 1.75;
  color: var(--chunsun-ink);
}

.docs-h2 {
  margin: 2rem 0 0.75rem;
  padding-top: 0.5rem;
  font-size: 1.12rem;
  font-weight: 800;
  color: var(--chunsun-ink);
  scroll-margin-top: 6rem;
}

.docs-h3 {
  margin: 1.5rem 0 0.6rem;
  font-size: 1rem;
  font-weight: 700;
  color: var(--chunsun-ink);
}

.docs-list {
  margin: 0;
  padding-left: 1.3rem;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  font-size: 0.95rem;
  line-height: 1.7;
  color: var(--chunsun-ink);
}

.docs-list-ordered {
  padding-left: 1.5rem;
}

/* 代码块 */
.docs-code-wrap {
  overflow: hidden;
  border-radius: 10px;
  border: 1px solid color-mix(in srgb, var(--chunsun-ink) 14%, transparent);
  background: var(--chunsun-code-bg);
}

.docs-code-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.45rem 0.85rem;
  background: color-mix(in srgb, var(--chunsun-ink) 92%, white);
  border-bottom: 1px solid color-mix(in srgb, white 10%, transparent);
}

.docs-code-lang {
  font-size: 0.72rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: color-mix(in srgb, var(--chunsun-fog) 70%, transparent);
}

.docs-code-copy {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.22rem 0.55rem;
  border: 1px solid color-mix(in srgb, white 18%, transparent);
  border-radius: 6px;
  background: transparent;
  color: color-mix(in srgb, var(--chunsun-fog) 80%, transparent);
  font-size: 0.74rem;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.docs-code-copy:hover {
  background: color-mix(in srgb, white 10%, transparent);
  color: var(--chunsun-fog);
}

.docs-code {
  margin: 0;
  padding: 1rem 1.1rem;
  overflow-x: auto;
  font-family: ui-monospace, "SF Mono", "Cascadia Code", Menlo, Consolas, monospace;
  font-size: 0.85rem;
  line-height: 1.65;
  color: var(--chunsun-code-color);
  white-space: pre;
}

/* 提示 */
.docs-note {
  display: flex;
  align-items: flex-start;
  gap: 0.55rem;
  padding: 0.75rem 0.95rem;
  border-radius: 10px;
  font-size: 0.9rem;
  line-height: 1.6;
  border: 1px solid transparent;
}

.docs-note.note-info {
  background: var(--chunsun-color-info-bg);
  border-color: var(--chunsun-color-info-border);
  color: var(--chunsun-color-info-text);
}

.docs-note.note-success {
  background: var(--chunsun-color-success-bg);
  border-color: var(--chunsun-color-success-border);
  color: var(--chunsun-color-success-text);
}

.docs-note.note-warn {
  background: var(--chunsun-color-warn-bg);
  border-color: var(--chunsun-color-warn-border);
  color: var(--chunsun-color-warn-text);
}

.docs-note-icon {
  margin-top: 0.2rem;
  font-size: 0.95rem;
  flex-shrink: 0;
}

/* 表格 */
.docs-table-wrap {
  overflow-x: auto;
  border: 1px solid color-mix(in srgb, var(--chunsun-rain) 22%, transparent);
  border-radius: 10px;
}

.docs-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.88rem;
}

.docs-table th,
.docs-table td {
  padding: 0.6rem 0.9rem;
  text-align: left;
  border-bottom: 1px solid color-mix(in srgb, var(--chunsun-rain) 16%, transparent);
  vertical-align: top;
}

.docs-table thead th {
  background: color-mix(in srgb, var(--chunsun-mist) 70%, white);
  font-weight: 700;
  color: var(--chunsun-ink);
}

.docs-table tbody tr:last-child td {
  border-bottom: none;
}

.docs-table tbody tr:hover {
  background: color-mix(in srgb, var(--chunsun-shoot) 4%, transparent);
}

.docs-table td {
  color: var(--chunsun-ink-muted);
  line-height: 1.6;
}

/* ── 中屏：隐藏右栏本页导航，退化为两栏 ── */
@media (max-width: 1100px) {
  .docs-body,
  .docs-body--no-toc {
    grid-template-columns: minmax(10.5rem, 12rem) minmax(0, 1fr);
    gap: 1.75rem;
  }

  .docs-toc-panel {
    display: none;
  }
}

/* ── 窄屏：单列，主导航置顶可折叠，本页导航移至正文前 ── */
@media (max-width: 900px) {
  .docs-body,
  .docs-body--no-toc {
    grid-template-columns: 1fr;
    gap: 1.5rem;
  }

  .docs-sidebar {
    position: static;
    order: 1;
  }

  .docs-toc-panel {
    display: block;
    position: static;
    max-height: none;
    overflow: visible;
    margin-left: 0;
    order: 2;
  }

  .docs-content {
    order: 3;
  }
}
</style>
