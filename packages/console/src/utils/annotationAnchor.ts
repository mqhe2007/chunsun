/**
 * 批注锚点定位（需求 u-WPvdvYh4Fw 第 2 步，阅读页高亮）。
 *
 * 批注存的是**渲染后可见文本**（anchorText + 前后文片段），不存 DOM 路径——
 * 渲染器升级后仍可靠纯文本重定位。定位管线：
 *
 *   DOM 文本节点序列 → 拼接成全文（折叠空白）→ indexOf 找锚点 → 映射回节点区间
 *
 * **必须拼接整棵子树再查找**：MarkdownIt 会把行内内容拆成多个文本节点
 * （`**粗体**` / `` `code` `` / 链接各成节点），假设锚点落在单节点里，
 * 跨行内标记的锚点将永远定位失败、被误判为漂移。
 *
 * 折叠规则与后端 `routes/project_knowledge_annotation.rs::markdown_visible_text`
 * 保持同口径（空白折叠为单空格）：批注锚点写入前后端各校验一次，规则漂移
 * 会表现为「后端判定 open、前端高亮不出来」，所以两边注释互相引用。
 *
 * 高亮渲染优先用 CSS Custom Highlight API（不改 DOM、零包裹节点，
 * TOC 跳转 / 滚动高亮完全不受影响）；不可用时退化为 <mark> 包裹。
 */

export type FoldedDoc = {
  /** 折叠空白后的拼接全文。 */
  folded: string;
  /** folded[i] 所处文本节点（可能为 null：构造时被过滤的节点）。 */
  nodes: (Text | null)[];
  /** folded[i] 对应该节点内的原始区间 [start, end)（代用对字符 end > start）。 */
  spans: Array<{ node: Text | null; start: number; end: number }>;
};

/** 一个锚点在 DOM 里的落点：按节点切开的原始文本区间（可跨节点）。 */
export type AnchorRange = { node: Text; start: number; end: number };

/** 空白折叠：连续空白 → 单空格。与后端一致，不 trim（trim 语义由调用方决定）。 */
export function foldText(input: string): string {
  let out = "";
  let prevSpace = false;
  for (const ch of input) {
    if (/\s/u.test(ch)) {
      if (!prevSpace) {
        out += " ";
        prevSpace = true;
      }
    } else {
      out += ch;
      prevSpace = false;
    }
  }
  return out;
}

/** 与 foldText 同口径、且首尾 trim（锚点入库前用这个形态）。 */
export function normalizeAnchorText(input: string): string {
  return foldText(input).trim();
}

/** 遍历子树收集文本节点，拼出 FoldedDoc。跳过 script/style 等不可见节点。 */
export function buildFoldedDoc(root: Element): FoldedDoc {
  const folded: string[] = [];
  const nodes: (Text | null)[] = [];
  const spans: Array<{ node: Text | null; start: number; end: number }> = [];

  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let prevSpace = false;
  let current = walker.nextNode() as Text | null;
  while (current) {
    const parent = current.parentElement;
    const tag = parent?.tagName.toLowerCase();
    if (tag !== "script" && tag !== "style") {
      const text = current.data;
      let i = 0;
      while (i < text.length) {
        const ch = text[i]!;
        if (/\s/u.test(ch)) {
          if (!prevSpace) {
            folded.push(" ");
            nodes.push(current);
            spans.push({ node: current, start: i, end: i + 1 });
            prevSpace = true;
          }
          i += 1;
        } else {
          // 非空白：按 code point 推进（代理对占两个 UTF-16 单元）
          const cp = text.codePointAt(i)!;
          const width = cp > 0xffff ? 2 : 1;
          folded.push(text.slice(i, i + width));
          nodes.push(current);
          spans.push({ node: current, start: i, end: i + width });
          prevSpace = false;
          i += width;
        }
      }
    }
    current = walker.nextNode() as Text | null;
  }
  return { folded: folded.join(""), nodes, spans };
}

/**
 * 在 FoldedDoc 里定位锚点，返回按节点切开的区间；找不到返回 null。
 *
 * 多处命中时用前后文片段消歧：prefix 命中 +2、suffix 命中 +1，
 * 同分取最早出现（与"从上往下读"的直觉一致，且结果确定）。
 * 上下文也是折叠域里的精确匹配——截取的片段若横跨空白差异则不加分，仅此而已。
 */
export function locateAnchor(
  doc: FoldedDoc,
  anchorText: string,
  prefix?: string | null,
  suffix?: string | null,
): AnchorRange[] | null {
  const needle = normalizeAnchorText(anchorText);
  if (!needle) return null;

  const hay = doc.folded;
  const candidates: number[] = [];
  let at = hay.indexOf(needle);
  while (at !== -1 && candidates.length < 50) {
    candidates.push(at);
    at = hay.indexOf(needle, at + 1);
  }
  if (candidates.length === 0) return null;

  const pre = prefix ? normalizeAnchorText(prefix) : "";
  const suf = suffix ? normalizeAnchorText(suffix) : "";
  let best = candidates[0]!;
  let bestScore = -1;
  for (const start of candidates) {
    const end = start + needle.length;
    let score = 0;
    if (pre && start >= pre.length && hay.startsWith(pre, start - pre.length)) score += 2;
    if (suf && hay.startsWith(suf, end)) score += 1;
    if (score > bestScore) {
      best = start;
      bestScore = score;
    }
  }

  // 折叠下标区间 → 按节点分组的原始区间
  const ranges: AnchorRange[] = [];
  const end = best + needle.length;
  let i = best;
  while (i < end) {
    const span = doc.spans[i]!;
    if (!span.node) {
      i += 1;
      continue;
    }
    const start = span.start;
    let last = span;
    let j = i + 1;
    while (j < end) {
      const next = doc.spans[j]!;
      if (next.node !== span.node) break;
      last = next;
      j += 1;
    }
    ranges.push({ node: span.node, start, end: last.end });
    i = j;
  }
  return ranges.length > 0 ? ranges : null;
}

export type LocatedAnnotation = {
  id: string;
  status: string;
  ranges: AnchorRange[];
};

/**
 * 批量定位：open / stale 的锚点批注参与高亮；整篇批注与 resolved 不参与。
 * stale 是服务端列表读取时判定的（同规则），这里只负责把结果画出来。
 */
export function locateAnnotations(
  doc: FoldedDoc,
  annotations: Array<{ id: string; status: string; anchorText?: string | null }>,
): LocatedAnnotation[] {
  const out: LocatedAnnotation[] = [];
  for (const ann of annotations) {
    if (ann.status !== "open" && ann.status !== "stale") continue;
    if (!ann.anchorText || !ann.anchorText.trim()) continue;
    const ranges = locateAnchor(doc, ann.anchorText);
    if (ranges) out.push({ id: ann.id, status: ann.status, ranges });
  }
  return out;
}
