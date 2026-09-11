import { describe, expect, test } from "vitest";
import { partitionProjectsByOwnership } from "./projectListPartition";

describe("partitionProjectsByOwnership", () => {
  const projects = [
    { id: "a", userId: "u1", name: "mine-1" },
    { id: "b", userId: "u2", name: "shared-1" },
    { id: "c", userId: "u1", name: "mine-2" },
    { id: "d", userId: "u3", name: "shared-2" },
  ];

  test("splits owned vs participating by creator userId", () => {
    const { mine, participating } = partitionProjectsByOwnership(projects, "u1");
    expect(mine.map((p) => p.id)).toEqual(["a", "c"]);
    expect(participating.map((p) => p.id)).toEqual(["b", "d"]);
  });

  test("treats all as participating when current user is missing", () => {
    const { mine, participating } = partitionProjectsByOwnership(projects, null);
    expect(mine).toEqual([]);
    expect(participating.map((p) => p.id)).toEqual(["a", "b", "c", "d"]);
  });
});
