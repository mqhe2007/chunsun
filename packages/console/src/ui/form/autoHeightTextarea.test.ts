import { describe, expect, test } from "vitest";
import { computeAutoHeight } from "./autoHeightTextarea";

describe("computeAutoHeight", () => {
  test("uses scrollHeight when above minimum", () => {
    expect(computeAutoHeight(120, 72)).toBe("120px");
  });

  test("respects minimum height for short content", () => {
    expect(computeAutoHeight(40, 72)).toBe("72px");
  });

  test("caps at maxHeight when provided", () => {
    expect(computeAutoHeight(400, 72, 240)).toBe("240px");
  });
});
