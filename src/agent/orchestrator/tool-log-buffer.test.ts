import { describe, expect, test } from "bun:test";
import {
  appendToolChunk,
  buildToolPayload,
  ensureToolBuffer,
  type ToolLogBufferMap,
} from "./tool-log-buffer.js";

describe("tool-log-buffer", () => {
  test("buffers chunks and parses JSON input", () => {
    const buffers: ToolLogBufferMap = new Map();
    ensureToolBuffer(buffers, "tool-1", "Edit");
    appendToolChunk(buffers, "tool-1", '{"path":');
    appendToolChunk(buffers, "tool-1", '"file.ts"');
    appendToolChunk(buffers, "tool-1", "}");

    const payload = buildToolPayload(buffers, "tool-1");

    expect(payload).toEqual({
      tool: "Edit",
      id: "tool-1",
      input: { path: "file.ts" },
    });
  });

  test("falls back to raw input when parsing fails and keeps final input fallback", () => {
    const buffers: ToolLogBufferMap = new Map();
    appendToolChunk(buffers, "tool-2", "{not-json");

    const payload = buildToolPayload(buffers, "tool-2", { foo: "bar" });

    expect(payload.id).toBe("tool-2");
    expect(payload.input).toEqual({ foo: "bar" });
  });
});
