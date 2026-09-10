import { describe, expect, test } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  test("renders headings, lists and emphasis", () => {
    const html = renderMarkdown("# 标题\n\n- 甲\n- 乙\n\n**加粗**");
    expect(html).toContain("<h1>标题</h1>");
    expect(html).toContain("<li>甲</li>");
    expect(html).toContain("<li>乙</li>");
    expect(html).toContain("<strong>加粗</strong>");
  });

  test("renders code block", () => {
    const html = renderMarkdown("```ts\nconst a = 1;\n```");
    expect(html).toContain("<pre><code class=\"language-ts\">");
    expect(html).toContain("const a = 1;");
  });

  test("renders table", () => {
    const html = renderMarkdown("| A | B |\n|---|---|\n| 1 | 2 |");
    expect(html).toContain("<table>");
    expect(html).toContain("<td>1</td>");
  });

  test("renders links", () => {
    const html = renderMarkdown("访问 https://example.com");
    expect(html).toContain('<a href="https://example.com"');
  });

  test("strips raw html for safety (html=false)", () => {
    const html = renderMarkdown("<script>alert(1)</script>\n\n正文");
    expect(html).not.toContain("<script>");
    expect(html).toContain("正文");
  });

  test("returns empty string for blank input", () => {
    expect(renderMarkdown("")).toBe("");
    expect(renderMarkdown("   ")).toBe("");
    expect(renderMarkdown(null as unknown as string)).toBe("");
  });
});
