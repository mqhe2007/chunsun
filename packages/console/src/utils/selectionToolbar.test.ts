import { describe, expect, test } from "vitest";
import {
  SELECTION_TOOLBAR_EDGE,
  SELECTION_TOOLBAR_GAP,
  selectionToolbarPlacement,
} from "./selectionToolbar";

describe("selectionToolbarPlacement", () => {
  const viewport = { width: 1200 };

  test("默认贴在选区上方，水平对齐中线", () => {
    const place = selectionToolbarPlacement(
      { top: 400, bottom: 420, left: 500, width: 200 },
      viewport,
    );
    expect(place).toEqual({ x: 600, y: 400 - SELECTION_TOOLBAR_GAP, below: false });
  });

  test("贴近左/右边缘时钳制在视口内", () => {
    // 中线 5px → 钳到左边界
    expect(
      selectionToolbarPlacement({ top: 300, bottom: 320, left: 0, width: 10 }, viewport).x,
    ).toBe(SELECTION_TOOLBAR_EDGE);
    // 中线 1200px → 钳到右边界
    expect(
      selectionToolbarPlacement({ top: 300, bottom: 320, left: 1180, width: 40 }, viewport).x,
    ).toBe(1200 - SELECTION_TOOLBAR_EDGE);
    // 完全在视口内则不钳制
    expect(
      selectionToolbarPlacement({ top: 300, bottom: 320, left: 0, width: 40 }, viewport).x,
    ).toBe(20);
  });

  test("上方空间不足时翻到选区下方", () => {
    const place = selectionToolbarPlacement(
      { top: 20, bottom: 44, left: 300, width: 100 },
      viewport,
    );
    expect(place.below).toBe(true);
    expect(place.y).toBe(44 + SELECTION_TOOLBAR_GAP);
  });

  test("窄视口下不会算出越界的 x", () => {
    const place = selectionToolbarPlacement(
      { top: 300, bottom: 320, left: 0, width: 10 },
      { width: 10 },
    );
    expect(place.x).toBe(SELECTION_TOOLBAR_EDGE);
  });
});
