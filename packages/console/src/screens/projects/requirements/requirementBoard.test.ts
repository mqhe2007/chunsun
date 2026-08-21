import { afterEach, describe, expect, test } from "vitest";
import {
  appendUniqueById,
  BOARD_COLUMNS,
  BOARD_PAGE_SIZE,
  BOARD_STATUSES,
  buildColumnQuery,
  groupCardsByStatus,
  parseListPageMeta,
  readViewMode,
  relocateCardByStatus,
  shouldLoadNextPage,
  writeViewMode,
} from "./requirementBoard";

const memoryStore = new Map<string, string>();
const localStorageStub = {
  getItem: (key: string) => memoryStore.get(key) ?? null,
  setItem: (key: string, value: string) => {
    memoryStore.set(key, value);
  },
  removeItem: (key: string) => {
    memoryStore.delete(key);
  },
  clear: () => memoryStore.clear(),
};
Object.defineProperty(globalThis, "localStorage", { value: localStorageStub, configurable: true });

afterEach(() => {
  memoryStore.clear();
});

describe("requirement board columns", () => {
  test("four columns map 1:1 to requirement statuses", () => {
    expect(BOARD_STATUSES).toEqual(["pending", "running", "completed", "abandoned"]);
    expect(BOARD_COLUMNS.map(c => c.label)).toEqual(["待处理", "运行中", "已完成", "已放弃"]);
  });

  test("cards land in the column of their workflow status", () => {
    const grouped = groupCardsByStatus([
      { id: "a", status: "pending" },
      { id: "b", status: "running" },
      { id: "c", status: "completed" },
      { id: "d", status: "abandoned" },
      { id: "e", status: "unknown" },
    ]);
    expect(grouped.pending.map(c => c.id)).toEqual(["a"]);
    expect(grouped.running.map(c => c.id)).toEqual(["b"]);
    expect(grouped.completed.map(c => c.id)).toEqual(["c"]);
    expect(grouped.abandoned.map(c => c.id)).toEqual(["d"]);
  });

  test("status change moves the card; there is no drag-to-column path", () => {
    const start = groupCardsByStatus([
      { id: "r1", status: "pending" },
      { id: "r2", status: "running" },
    ]);
    const moved = relocateCardByStatus(start, { id: "r1", status: "completed" });
    expect(moved.pending.map(c => c.id)).toEqual([]);
    expect(moved.completed.map(c => c.id)).toEqual(["r1"]);
    expect(moved.running.map(c => c.id)).toEqual(["r2"]);
  });
});

describe("requirement view mode", () => {
  test("persists list/board per project across reload", () => {
    expect(readViewMode("p1")).toBe("list");
    writeViewMode("p1", "board");
    writeViewMode("p2", "list");
    expect(readViewMode("p1")).toBe("board");
    expect(readViewMode("p2")).toBe("list");
  });
});

describe("board column query", () => {
  test("filters by status, page, and optional id/owner", () => {
    const qs = buildColumnQuery({
      status: "running",
      id: "abc",
      ownerId: "u1",
      page: 2,
      pageSize: 20,
    });
    const params = new URLSearchParams(qs);
    expect(params.get("status")).toBe("running");
    expect(params.get("page")).toBe("2");
    expect(params.get("pageSize")).toBe("20");
    expect(params.get("id")).toBe("abc");
    expect(params.get("ownerId")).toBe("u1");
  });
});

describe("column infinite scroll", () => {
  test("appends the next page without duplicating cards", () => {
    const page1 = [{ id: "1", status: "pending" }, { id: "2", status: "pending" }];
    const page2 = [{ id: "2", status: "pending" }, { id: "3", status: "pending" }];
    expect(appendUniqueById(page1, page2).map(c => c.id)).toEqual(["1", "2", "3"]);
  });

  test("loads next page until loaded count reaches total", () => {
    expect(
      shouldLoadNextPage({ loaded: 20, total: 45, page: 1, pageSize: BOARD_PAGE_SIZE }),
    ).toBe(true);
    expect(
      shouldLoadNextPage({ loaded: 45, total: 45, page: 3, pageSize: BOARD_PAGE_SIZE }),
    ).toBe(false);
    expect(shouldLoadNextPage({ loaded: 20, total: 45, page: 1, loading: true })).toBe(false);
  });
});

describe("paginated list meta", () => {
  test("reads page/pageSize/total from list payload", () => {
    expect(
      parseListPageMeta({
        data: [{ id: "1" }],
        meta: { page: 1, pageSize: 20, total: 3 },
      }),
    ).toEqual({ page: 1, pageSize: 20, total: 3 });
    expect(parseListPageMeta({ data: [] })).toBeNull();
  });
});
