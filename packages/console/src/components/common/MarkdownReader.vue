<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { extractToc, renderMarkdown, type TocItem } from "@/utils/markdown";
import "@/assets/markdown.css";

// showToc 必须显式 default true：Vue 对缺省的 Boolean prop 会 casting 成 false，
// 不给默认值的话 TOC 永远不渲染。
const props = withDefaults(
  defineProps<{
    content: string;
    showToc?: boolean;
  }>(),
  { showToc: true },
);

const rootEl = ref<HTMLElement | null>(null);
const activeId = ref("");

const html = computed(() => renderMarkdown(props.content ?? ""));
const toc = computed<TocItem[]>(() =>
  props.showToc === false ? [] : extractToc(props.content ?? ""),
);

function scrollTo(id: string) {
  const el = rootEl.value?.querySelector(`#${CSS.escape(id)}`);
  el?.scrollIntoView({ behavior: "smooth", block: "start" });
  activeId.value = id;
}

function onScroll() {
  if (!rootEl.value || toc.value.length === 0) return;
  const body = rootEl.value.querySelector(".markdown-body");
  if (!body) return;
  const containerTop = scrollTarget instanceof HTMLElement ? scrollTarget.getBoundingClientRect().top : 0;
  let current = toc.value[0]?.id ?? "";
  for (const item of toc.value) {
    const el = body.querySelector(`#${CSS.escape(item.id)}`) as HTMLElement | null;
    if (!el) continue;
    if (el.getBoundingClientRect().top - containerTop <= 120) current = item.id;
  }
  activeId.value = current;
}

let scrollTarget: HTMLElement | Window = window;

function findScrollTarget(): HTMLElement | Window {
  let parent = rootEl.value?.parentElement ?? null;
  while (parent) {
    const overflowY = getComputedStyle(parent).overflowY;
    if (overflowY === "auto" || overflowY === "scroll" || overflowY === "overlay") {
      return parent;
    }
    parent = parent.parentElement;
  }
  return window;
}

function bindScrollTarget(next: HTMLElement | Window) {
  if (next === scrollTarget) return;
  scrollTarget.removeEventListener("scroll", onScroll);
  scrollTarget = next;
  scrollTarget.addEventListener("scroll", onScroll, { passive: true });
}

onMounted(() => {
  bindScrollTarget(findScrollTarget());
  void nextTick(onScroll);
});

onUnmounted(() => scrollTarget.removeEventListener("scroll", onScroll));
watch(html, () => void nextTick(onScroll));
</script>

<template>
  <div ref="rootEl" class="md-reader flex min-w-0 flex-col gap-4 lg:flex-row lg:items-start">
    <aside
      v-if="toc.length"
      class="toc-panel w-full shrink-0 lg:sticky lg:top-4 lg:max-h-[calc(100vh-6rem)] lg:w-56 lg:overflow-y-auto lg:rounded-box lg:border lg:border-base-300 lg:bg-base-100 lg:px-2 lg:py-3 lg:shadow-sm"
    >
      <details class="collapse collapse-arrow border border-base-300 bg-base-100 lg:hidden" open>
        <summary class="collapse-title text-sm font-medium min-h-0 py-2">目录</summary>
        <div class="collapse-content">
          <nav class="toc-nav flex flex-col gap-1">
            <button
              v-for="item in toc"
              :key="item.id"
              type="button"
              class="toc-link btn btn-ghost btn-xs justify-start font-normal h-auto min-h-0 py-1"
              :class="activeId === item.id ? 'bg-base-200 text-primary' : ''"
              :style="{ paddingLeft: `${(item.level - 1) * 0.65 + 0.35}rem` }"
              @click="scrollTo(item.id)"
            >
              {{ item.text }}
            </button>
          </nav>
        </div>
      </details>
      <nav class="toc-nav hidden flex-col gap-1 lg:flex">
        <p class="mb-2 border-b border-base-300 px-1 pb-2 text-xs font-medium text-base-content/50">目录</p>
        <button
          v-for="item in toc"
          :key="`d-${item.id}`"
          type="button"
          class="toc-link btn btn-ghost btn-xs justify-start font-normal h-auto min-h-0 py-1"
          :class="activeId === item.id ? 'bg-base-200 text-primary' : ''"
          :style="{ paddingLeft: `${(item.level - 1) * 0.65 + 0.35}rem` }"
          @click="scrollTo(item.id)"
        >
          {{ item.text }}
        </button>
      </nav>
    </aside>
    <div
      class="markdown-body md-reader-body min-w-0 flex-1 rounded-box border border-base-300 bg-base-100 p-4 shadow-sm lg:p-6"
      v-html="html"
    />
  </div>
</template>
