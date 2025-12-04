import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { createTBEmitter, createTBOutputCallback } from "./emit.js";

class FakeWebSocket {
  static OPEN = 1;
  static sent: string[] = [];
  readyState = FakeWebSocket.OPEN;
  onopen: ((ev: any) => void) | null = null;
  onclose: ((ev: any) => void) | null = null;
  constructor(_url: string) {
    queueMicrotask(() => {
      this.onopen?.({});
    });
  }
  send(message: string) {
    FakeWebSocket.sent.push(message);
  }
  close() {
    this.readyState = 3;
    this.onclose?.({});
  }
  static reset() {
    FakeWebSocket.sent = [];
  }
}

describe("tbench-hud emitter", () => {
  const originalWebSocket = globalThis.WebSocket;

  beforeEach(() => {
    FakeWebSocket.reset();
    globalThis.WebSocket = FakeWebSocket as any;
  });

  afterEach(() => {
    globalThis.WebSocket = originalWebSocket;
  });

  test("runStart/runComplete manage runId and emit messages", async () => {
    const emitter = createTBEmitter({ url: "ws://fake" });
    const suite = { name: "Suite", version: "1.0.0", tasks: [] };
    const runId = emitter.runStart(suite, ["task-1"]);

    await new Promise((r) => setTimeout(r, 0)); // allow queue flush
    expect(runId).toMatch(/^tb-/);
    expect(FakeWebSocket.sent.some((msg) => msg.includes("\"type\":\"tb_run_start\""))).toBe(true);

    emitter.runComplete({
      passRate: 1,
      passed: 1,
      failed: 0,
      timeout: 0,
      error: 0,
      totalDurationMs: 100,
    });
    await new Promise((r) => setTimeout(r, 0));
    expect(FakeWebSocket.sent.some((msg) => msg.includes("\"type\":\"tb_run_complete\""))).toBe(true);

    // After runComplete, task events should be dropped (no new sends)
    const beforeCount = FakeWebSocket.sent.length;
    emitter.taskStart({ id: "t1", name: "Task", category: "cat", difficulty: "easy" }, 0, 1);
    await new Promise((r) => setTimeout(r, 0));
    expect(FakeWebSocket.sent.length).toBe(beforeCount);
  });

  test("task events emit when runId is active", async () => {
    const emitter = createTBEmitter({ url: "ws://fake" });
    const suite = { name: "Suite", version: "1.0.0", tasks: [] };
    emitter.runStart(suite, ["task-1"]);
    await new Promise((r) => setTimeout(r, 0));

    emitter.taskStart({ id: "t1", name: "Task", category: "cat", difficulty: "easy" }, 0, 1);
    emitter.taskProgress("t1", "agent", 2, 10);
    emitter.taskOutput("t1", "chunk", "agent");
    emitter.taskComplete("t1", {
      outcome: "success",
      durationMs: 5,
      turns: 2,
      tokens: 10,
    });
    await new Promise((r) => setTimeout(r, 0));

    const sentTypes = FakeWebSocket.sent.map((msg) => JSON.parse(msg).type);
    expect(sentTypes).toEqual(
      expect.arrayContaining([
        "tb_task_start",
        "tb_task_progress",
        "tb_task_output",
        "tb_task_complete",
      ]),
    );
  });

  test("createTBOutputCallback forwards chunks with source", async () => {
    const emitter = createTBEmitter({ url: "ws://fake" });
    emitter.runStart({ name: "Suite", version: "1.0.0", tasks: [] }, ["t1"]);
    await new Promise((r) => setTimeout(r, 0));

    const onOutput = createTBOutputCallback(emitter, "t1", "agent");
    onOutput("hello");
    await new Promise((r) => setTimeout(r, 0));

    const last = JSON.parse(FakeWebSocket.sent.at(-1)!);
    expect(last.type).toBe("tb_task_output");
    expect(last.source).toBe("agent");
    expect(last.text).toBe("hello");
  });
});
