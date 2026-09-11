import MarkdownIt from "markdown-it";

export const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
});

function slugify(text: string): string {
  const base = text
    .trim()
    .toLowerCase()
    .replace(/[^a-zA-Z0-9\u4e00-\u9fff\s-_]/g, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return base || "section";
}

md.core.ruler.push("heading_anchors", state => {
  const counts: Record<string, number> = {};
  const tokens = state.tokens;
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token.type !== "heading_open") continue;
    const level = Number(token.tag.slice(1));
    if (level < 1 || level > 3) continue;
    const inline = tokens[i + 1];
    const title = inline?.type === "inline" ? inline.content : "";
    const base = slugify(title);
    const n = counts[base] ?? 0;
    counts[base] = n + 1;
    const id = n === 0 ? base : `${base}-${n}`;
    token.attrSet("id", id);
  }
});

export type TocItem = {
  id: string;
  level: number;
  text: string;
};

export function extractToc(text: string): TocItem[] {
  if (!text || !text.trim()) return [];
  const tokens = md.parse(text, {});
  const items: TocItem[] = [];
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.type !== "heading_open") continue;
    const level = Number(t.tag.slice(1));
    if (level < 1 || level > 3) continue;
    const inline = tokens[i + 1];
    const textContent = inline?.type === "inline" ? inline.content : "";
    const id = t.attrGet("id");
    if (id == null || id === "") continue;
    items.push({ id: String(id), level, text: textContent });
  }
  return items;
}

export function renderMarkdown(text: string): string {
  if (!text || !text.trim()) return "";
  return md.render(text);
}
