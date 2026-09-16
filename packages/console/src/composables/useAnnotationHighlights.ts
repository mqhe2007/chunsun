import { nextTick, onBeforeUnmount, watch, type Ref } from "vue";
import {
  buildFoldedDoc,
  locateAnchor,
  locateAnnotations,
  type AnchorRange,
  type FoldedDoc,
} from "@/utils/annotationAnchor";
import type { KnowledgeAnnotation } from "./useAnnotations";

const OPEN_KEY = "ann-open";
const STALE_KEY = "ann-stale";

/**
 * 阅读页内联高亮（需求 u-WPvdvYh4Fw 第 2 步）。
 *
 * **纯附加**：内容仍由 `renderMarkdown` 一次性输出（v-html），本 composable 只在
 * 渲染完成后**读取** DOM 做定位，绝不改渲染管线输出的结构——TOC 锚点跳转与
 * 滚动高亮因此零回归。
 *
 * 优先 CSS Custom Highlight API（`CSS.highlights` + Range）：零 DOM 修改。
 * 不支持时退化为 <mark> 包裹（同节点内的 Range 逐段包裹），v-html 重渲染
 * 会自然清掉旧包裹，重算时再补。
 *
 * 用法：`watch` 触发重算；`hitTest(x, y)` 供点击命中（把高亮连到右栏对应项）。
 */
export function useAnnotationHighlights(
  getRoot: () => HTMLElement | null,
  annotations: Ref<KnowledgeAnnotation[]>,
  enabled: Ref<boolean>,
) {
  const supportsHighlights = typeof CSS !== "undefined" && "highlights" in CSS;
  let folded: FoldedDoc | null = null;
  let located: Array<{ id: string; status: string; ranges: AnchorRange[] }> = [];
  let scheduled = false;
  let flashTimer: number | null = null;

  function clearHighlights() {
    if (supportsHighlights) {
      CSS.highlights.delete(OPEN_KEY);
      CSS.highlights.delete(STALE_KEY);
    }
    const root = getRoot();
    root?.querySelectorAll("mark[data-ann-id]").forEach(m => {
      const parent = m.parentNode;
      if (parent) {
        while (m.firstChild) parent.insertBefore(m.firstChild, m);
        m.remove();
        parent.normalize(); // 合回相邻文本节点，尽量还原 DOM
      }
    });
  }

  function recompute() {
    clearHighlights();
    const root = getRoot();
    if (!root || !enabled.value) return;
    const body = root.querySelector(".markdown-body") ?? root;
    folded = buildFoldedDoc(body);
    located = locateAnnotations(folded, annotations.value) as Array<{
      id: string;
      status: string;
      ranges: AnchorRange[];
    }>;

    if (supportsHighlights) {
      const open = new Highlight();
      const stale = new Highlight();
      for (const item of located) {
        for (const r of item.ranges) {
          const range = new Range();
          range.setStart(r.node, r.start);
          range.setEnd(r.node, r.end);
          (item.status === "stale" ? stale : open).add(range);
        }
      }
      if (open.size > 0) CSS.highlights.set(OPEN_KEY, open);
      if (stale.size > 0) CSS.highlights.set(STALE_KEY, stale);
    } else {
      // 退化路径：逐段包裹（Range 都在单个文本节点内，surroundContents 可用）
      for (const item of located) {
        for (const r of item.ranges) {
          try {
            const range = new Range();
            range.setStart(r.node, r.start);
            range.setEnd(r.node, r.end);
            const mark = document.createElement("mark");
            mark.dataset.annId = item.id;
            mark.className = item.status === "stale" ? "ann-fallback-stale" : "ann-fallback-open";
            range.surroundContents(mark);
          } catch {
            // 包裹失败（节点边界意外）：放弃该段，不影响其余高亮
          }
        }
      }
    }
  }

  function schedule() {
    if (scheduled) return;
    scheduled = true;
    void nextTick(() => {
      scheduled = false;
      recompute();
    });
  }

  /** 点击命中：返回该坐标下被高亮的批注 id（供联动右栏）。 */
  function hitTest(x: number, y: number): string | null {
    const doc = document as Document & {
      caretRangeFromPoint?: (x: number, y: number) => Range | null;
      caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null;
    };
    let node: Node | null = null;
    let offset = 0;
    if (doc.caretRangeFromPoint) {
      const r = doc.caretRangeFromPoint(x, y);
      node = r?.startContainer ?? null;
      offset = r?.startOffset ?? 0;
    } else if (doc.caretPositionFromPoint) {
      const p = doc.caretPositionFromPoint(x, y);
      node = p?.offsetNode ?? null;
      offset = p?.offset ?? 0;
    }
    if (!node || node.nodeType !== Node.TEXT_NODE) return null;
    for (const item of located) {
      for (const r of item.ranges) {
        if (r.node === node && offset >= r.start && offset <= r.end) return item.id;
      }
    }
    return null;
  }

  /** 结束闪烁：拆掉临时 <mark>、还原文本节点，并刷新缓存区间。 */
  function endFlash() {
    if (flashTimer !== null) {
      window.clearTimeout(flashTimer);
      flashTimer = null;
    }
    const marks = document.querySelectorAll("mark.ann-content-flash");
    if (marks.length === 0) return;
    for (const mark of marks) {
      const parent = mark.parentNode;
      if (!parent) continue;
      while (mark.firstChild) parent.insertBefore(mark.firstChild, mark);
      mark.remove();
      parent.normalize();
    }
    // 包裹/还原会拆合文本节点，缓存的 node 引用随之失效；重算一次，
    // 保证再次点击卡片（或点正文高亮）仍能命中。
    schedule();
  }

  /** 从面板点击批注 → 滚动到正文锚点并闪烁。 */
  function scrollToAnchor(id: string) {
    const ann = annotations.value.find(item => item.id === id);
    if (!ann?.anchorText?.trim()) return;

    // 每次都按当前 DOM 重新定位：可重复点击，不依赖上一次的缓存区间。
    // （resolved 批注刻意不参与常态高亮，这里也从正文临时定位。）
    endFlash();
    const root = getRoot();
    if (!root) return;
    const body = root.querySelector(".markdown-body") ?? root;
    const ranges =
      locateAnchor(buildFoldedDoc(body), ann.anchorText, ann.anchorPrefix, ann.anchorSuffix) ?? [];
    if (ranges.length === 0) return;

    // 用临时 <mark> 做闪烁。先获得 mark 再滚动，能精确把选中文本放到
    // 视口中央；长段落里只滚 parentElement 往往仍然看不到锚点。
    const marks: HTMLElement[] = [];
    for (const r of ranges) {
      try {
        const range = new Range();
        range.setStart(r.node, r.start);
        range.setEnd(r.node, r.end);
        const mark = document.createElement("mark");
        mark.className = "ann-content-flash";
        range.surroundContents(mark);
        marks.push(mark);
      } catch {
        // 跨节点边界：跳过该段
      }
    }

    // 包裹失败（锚点跨行内元素）时退化为滚动锚点所在块，至少保证「可见」。
    const scrollTarget: Element | null =
      marks[0] ?? ranges[0]?.node.parentElement ?? body;
    scrollTarget.scrollIntoView({ behavior: "smooth", block: "center" });

    flashTimer = window.setTimeout(endFlash, 2400);
  }

  watch([annotations, enabled], schedule, { deep: true });

  onBeforeUnmount(() => {
    if (flashTimer !== null) window.clearTimeout(flashTimer);
    flashTimer = null;
    clearHighlights();
    folded = null;
    located = [];
  });

  return { recompute: schedule, hitTest, scrollToAnchor, supportsHighlights };
}
