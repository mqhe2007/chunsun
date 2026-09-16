/**
 * 正文选区工具条（「批注」浮层）的落点计算。
 *
 * 输出的是**视口坐标**，直接喂给 `position: fixed` 的元素；元素自身用
 * `translate(-50%, -100%)`（below=false）或 `translate(-50%, 0)`（below=true）对齐，
 * 所以这里只算「中线 x」与「贴边 y」。
 *
 * 规则：
 *  - 水平：对齐选区中线，并钳制在视口内，避免贴边被裁掉；
 *  - 垂直：默认贴在选区上方；上方空间不足（顶部工具条/浏览器 chrome）时翻到下方。
 */

export const SELECTION_TOOLBAR_GAP = 8;
export const SELECTION_TOOLBAR_EDGE = 12;
/** 小于该值认为“上方放不下”。 */
export const SELECTION_TOOLBAR_MIN_TOP = 56;

export type SelectionToolbarPlacement = {
  x: number;
  y: number;
  below: boolean;
};

export function selectionToolbarPlacement(
  rect: { top: number; bottom: number; left: number; width: number },
  viewport: { width: number },
): SelectionToolbarPlacement {
  const center = rect.left + rect.width / 2;
  const maxX = Math.max(SELECTION_TOOLBAR_EDGE, viewport.width - SELECTION_TOOLBAR_EDGE);
  const x = Math.min(Math.max(center, SELECTION_TOOLBAR_EDGE), maxX);
  const below = rect.top < SELECTION_TOOLBAR_MIN_TOP;
  const y = below ? rect.bottom + SELECTION_TOOLBAR_GAP : rect.top - SELECTION_TOOLBAR_GAP;
  return { x, y, below };
}
