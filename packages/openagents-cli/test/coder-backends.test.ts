import { describe, expect, it } from "vitest";

import { backendIds, CODER_BACKENDS } from "../src/coder-backends.js";

/**
 * The list two surfaces read.
 *
 * `--model` takes its accepted values from it and the status line takes its
 * label from it. These pin the properties those two depend on, so adding a
 * backend stays one entry rather than one entry plus two fixes.
 */
describe("coder backends", () => {
  it("names each backend once", () => {
    expect(backendIds()).toEqual([...new Set(backendIds())]);
    expect(CODER_BACKENDS.length).toBeGreaterThan(1);
  });

  it("gives every backend a label a status line can show", () => {
    for (const backend of CODER_BACKENDS) {
      expect(backend.label).not.toBe("");
      expect(backend.id).not.toBe("");
    }
  });

  it("publishes ids the chat API's own enum lists", () => {
    // These are the values `POST /api/v3/chat/turns` accepts as `model`, so a
    // change here without the matching server change is a refusal at runtime.
    expect(backendIds()).toEqual(["ox-alpha", "gemini-3.7-flash"]);
  });
});
