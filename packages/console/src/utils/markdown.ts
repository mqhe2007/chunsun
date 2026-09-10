import MarkdownIt from "markdown-it";

/**
 * 共享 MarkdownIt 实例：项目内所有 md 文本渲染共用同一配置。
 * html=false：禁止原始 HTML，避免 XSS；linkify=true：自动识别链接。
 */
export const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
});

/** 渲染 md 文本为 HTML；空文本返回空字符串。 */
export function renderMarkdown(text: string): string {
  if (!text || !text.trim()) return "";
  return md.render(text);
}
