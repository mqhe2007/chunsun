import { describe, expect, test } from "vitest";
import {
  buildCreateProjectPayload,
  createProjectFormState,
} from "./projectFormState";

describe("project form state", () => {
  test("create payload excludes templateRepo and accessToken", () => {
    const payload = buildCreateProjectPayload({
      name: "春笋",
      description: "platform",
    });

    expect(payload).toEqual({
      name: "春笋",
      description: "platform",
    });
    expect("templateRepo" in payload).toBe(false);
    expect("accessToken" in payload).toBe(false);
  });

  test("default form state only includes visible fields", () => {
    expect(createProjectFormState()).toEqual({
      name: "",
      description: "",
    });
  });
});
