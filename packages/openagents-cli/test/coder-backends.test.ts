import { describe, expect, it } from "vitest";

import {
  backendIds,
  CODER_BACKENDS,
  defaultBackend,
  findBackend,
  nextBackend,
} from "../src/coder-backends.js";

/**
 * The list three surfaces read.
 *
 * `--model` takes its accepted values from it, the status line takes its label
 * from it, and Tab walks it. These pin the properties those three depend on, so
 * adding a backend stays one entry rather than one entry plus three fixes.
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

  it("defaults to the first entry, and finds a backend by the id the server takes", () => {
    expect(defaultBackend()).toBe(CODER_BACKENDS[0]);
    expect(findBackend("gemini-3.7-flash")?.label).toBe("Gemini 3.7 Flash");
    expect(findBackend("gpt-4")).toBeUndefined();
  });

  it("reaches every backend by cycling, and returns to the start", () => {
    const seen: string[] = [];
    let current = defaultBackend();
    for (let step = 0; step < CODER_BACKENDS.length; step += 1) {
      current = nextBackend(current);
      seen.push(current.id);
    }

    expect(new Set(seen)).toEqual(new Set(backendIds()));
    expect(current).toBe(defaultBackend());
  });

  it("publishes ids the chat API's own enum lists", () => {
    // These are the values `POST /api/v3/chat/turns` accepts as `model`, so a
    // change here without the matching server change is a refusal at runtime.
    expect(backendIds()).toEqual(["ox-alpha", "gemini-3.7-flash"]);
  });
});
