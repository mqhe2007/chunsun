import { describe, expect, test } from "vitest";
import {
  buildFoldedDoc,
  foldText,
  locateAnnotations,
  locateAnchor,
  normalizeAnchorText,
  type FoldedDoc,
} from "./annotationAnchor";

/** 合成 FoldedDoc：把若干 (节点文本) 段拼起来，模拟 MarkdownIt 拆出的多文本节点。
 * 与 buildFoldedDoc 同规则：折叠空白（连续空白 → 单个条目）、同段复用同一节点对象。 */
function docFrom(...segments: string[]): FoldedDoc {
  const folded: string[] = [];
  const nodes: (Text | null)[] = [];
  const spans: FoldedDoc["spans"] = [];
  let prevSpace = false;
  segments.forEach((seg, idx) => {
    const node = { seg: idx } as unknown as Text;
    for (let i = 0; i < seg.length; i++) {
      const ch = seg[i]!;
      if (/\s/u.test(ch)) {
        if (!prevSpace) {
          folded.push(" ");
          nodes.push(node);
          spans.push({ node, start: i, end: i + 1 });
          prevSpace = true;
        }
      } else {
        folded.push(ch);
        nodes.push(node);
        spans.push({ node, start: i, end: i + 1 });
        prevSpace = false;
      }
    }
  });
  return { folded: folded.join(""), nodes, spans };
}

function textOf(doc: FoldedDoc, ranges: ReturnType<typeof locateAnchor>): string {
  return ranges!.map(r => (r.node as unknown as { seg: number }).seg + ":" + r.start + "-" + r.end).join(",");
}

describe("foldText / normalizeAnchorText", () => {
  test("连续空白折叠为单空格", () => {
    expect(foldText("a\n\n  b")).toBe("a b");
    expect(foldText("第一行\n第二行")).toBe("第一行 第二行");
  });

  test("normalizeAnchorText 首尾 trim", () => {
    expect(normalizeAnchorText("  使用 nanoid 生成主键。 ")).toBe("使用 nanoid 生成主键。");
  });
});

describe("locateAnchor", () => {
  test("跨行内标记节点的锚点可定位（粗体 + 行内代码 + 链接）", () => {
    // 渲染后 DOM：<p>我们约定 <strong>必须</strong> 用 <code>nanoid</code> 生成 <a>主键</a>。</p>
    const doc = docFrom("我们约定 ", "必须", " 用 ", "nanoid", " 生成 ", "主键", "。");
    const ranges = locateAnchor(doc, "必须 用 nanoid 生成 主键");
    expect(ranges).not.toBeNull();
    // 应切回各节点：strong、正文（含折叠空格）、code、正文、a
    expect(textOf(doc, ranges)).toBe("1:0-2,2:0-3,3:0-6,4:0-4,5:0-2");
  });

  test("源码换行（渲染后是空格）不影响匹配", () => {
    const doc = docFrom("第一行\n第二行\n\n第三行");
    expect(locateAnchor(doc, "第一行 第二行")).not.toBeNull();
    expect(locateAnchor(doc, "第二行 第三行")).not.toBeNull();
  });

  test("多处命中时前后文消歧", () => {
    // "必须"出现两次，锚点 prefix 指向第二处（第7-9字是"第二次"，必须从第10字起）
    const doc = docFrom("第一次必须赢，第二次必须稳。");
    const hit = locateAnchor(doc, "必须", "第二次", "稳");
    expect(hit).not.toBeNull();
    expect((hit![0]!.node as unknown as { seg: number }).seg).toBe(0);
    expect(hit![0]!.start).toBe(10);
  });

  test("找不到返回 null（漂移）", () => {
    const doc = docFrom("文档已被整段重写。");
    expect(locateAnchor(doc, "我们约定要用 nanoid")).toBeNull();
  });

  test("空锚点返回 null（整篇批注不参与高亮）", () => {
    const doc = docFrom("随便什么正文");
    expect(locateAnchor(doc, "")).toBeNull();
    expect(locateAnchor(doc, "   ")).toBeNull();
  });

  test("锚点带首尾空白仍可匹配", () => {
    const doc = docFrom("前文 使用 nanoid 生成主键 后文");
    expect(locateAnchor(doc, "  使用 nanoid 生成主键 ")).not.toBeNull();
  });
});

describe("locateAnnotations", () => {
  const doc = docFrom("正文甲 正文乙");

  test("open / stale 参与，resolved 与整篇批注不参与", () => {
    const out = locateAnnotations(doc, [
      { id: "a", status: "open", anchorText: "正文甲" },
      { id: "b", status: "stale", anchorText: "正文乙" },
      { id: "c", status: "resolved", anchorText: "正文甲" },
      { id: "d", status: "open", anchorText: null },
      { id: "e", status: "open", anchorText: "不存在的锚点" },
    ]);
    expect(out.map(o => o.id)).toEqual(["a", "b"]);
  });
});

describe("buildFoldedDoc", () => {
  test("在 jsdom 缺省的 node 环境下跳过（仅 node 适配器占位断言）", () => {
    // buildFoldedDoc 依赖 document（TreeWalker）；本仓库 vitest 跑在 node 环境，
    // DOM 适配器的行为由 e2e（阅读页高亮检查）覆盖，这里只断言导出存在。
    expect(typeof buildFoldedDoc).toBe("function");
  });
});
