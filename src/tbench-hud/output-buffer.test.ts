import { describe, expect, test } from "bun:test";
import {
  appendAndFlush,
  clearAllBuffers,
  createBuffer,
  flushAllBuffers,
  forceFlush,
  getBufferContent,
  getOrCreateBuffer,
} from "./output-buffer.js";

describe("tbench-hud output buffer", () => {
  test("flushes on newline and preserves tail", () => {
    const buffer = createBuffer("task-1", "agent");
    const emitted: string[] = [];

    const flushed = appendAndFlush(buffer, "hello\nworld", (line) => emitted.push(line));

    expect(flushed).toBe(1);
    expect(emitted).toEqual(["hello"]);
    expect(getBufferContent(buffer)).toBe("world");
  });

  test("flushes on size threshold", () => {
    const buffer = createBuffer("task-1", "agent");
    const emitted: string[] = [];

    const flushed = appendAndFlush(buffer, "abcd", (line) => emitted.push(line), {
      sizeThreshold: 3,
    });

    expect(flushed).toBe(1);
    expect(emitted).toEqual(["abcd"]);
    expect(getBufferContent(buffer)).toBe("");
  });

  test("flushes on time threshold", () => {
    const buffer = createBuffer("task-1", "agent");
    buffer.lastFlushTime = Date.now() - 1000; // force timeout path
    buffer.buffer = "stale";
    const emitted: string[] = [];

    const flushed = appendAndFlush(buffer, "", (line) => emitted.push(line));

    expect(flushed).toBe(1);
    expect(emitted).toEqual(["stale"]);
  });

  test("forceFlush empties buffer", () => {
    const buffer = createBuffer("task-1", "agent");
    buffer.buffer = "tail";
    const emitted: string[] = [];

    const flushed = forceFlush(buffer, (line) => emitted.push(line));

    expect(flushed).toBe(true);
    expect(emitted).toEqual(["tail"]);
    expect(getBufferContent(buffer)).toBe("");
  });

  test("flushAllBuffers emits all and clears", () => {
    const buffers = new Map<string, ReturnType<typeof createBuffer>>();
    const b1 = getOrCreateBuffer(buffers, "task-1", "agent");
    const b2 = getOrCreateBuffer(buffers, "task-2", "verification");
    b1.buffer = "one";
    b2.buffer = "two";

    const emitted: Array<{ taskId: string; source: string; text: string }> = [];
    const flushed = flushAllBuffers(buffers, (taskId, source, text) =>
      emitted.push({ taskId, source, text }),
    );

    expect(flushed).toBe(2);
    expect(emitted).toEqual([
      { taskId: "task-1", source: "agent", text: "one" },
      { taskId: "task-2", source: "verification", text: "two" },
    ]);
    expect(b1.buffer).toBe("");
    expect(b2.buffer).toBe("");

    clearAllBuffers(buffers);
    expect(buffers.size).toBe(0);
  });
});
