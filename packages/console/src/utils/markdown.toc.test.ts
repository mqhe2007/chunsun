import { describe, expect, it } from "vitest";
import { extractToc, renderMarkdown } from "./markdown";

describe("markdown toc anchors", () => {
  it("extracts h1-h3 with stable ids matching render", () => {
    const md = "# Alpha\n\n## Beta\n\n### Gamma\n\n## Beta\n";
    const toc = extractToc(md);
    expect(toc.map(t => t.id)).toEqual(["alpha", "beta", "gamma", "beta-1"]);
    const html = renderMarkdown(md);
    expect(html).toContain('id="alpha"');
    expect(html).toContain('id="beta-1"');
  });
});
