import { describe, expect, test } from "bun:test";
import {
  isHudMessage,
  serializeHudMessage,
  parseHudMessage,
  HUD_WS_PORT,
  HUD_WS_URL,
  type HudMessage,
  type SessionStartMessage,
  type TaskSelectedMessage,
  type PhaseChangeMessage,
  type TextOutputMessage,
  type ToolCallMessage,
  type ToolResultMessage,
} from "./protocol.js";

describe("protocol constants", () => {
  test("HUD_WS_PORT is 4242", () => {
    expect(HUD_WS_PORT).toBe(4242);
  });

  test("HUD_WS_URL uses correct port", () => {
    expect(HUD_WS_URL).toBe("ws://localhost:4242");
  });
});

describe("isHudMessage", () => {
  test("returns true for valid HudMessage with type field", () => {
    const msg = { type: "session_start", sessionId: "abc", timestamp: "2024-01-01" };
    expect(isHudMessage(msg)).toBe(true);
  });

  test("returns false for null", () => {
    expect(isHudMessage(null)).toBe(false);
  });

  test("returns false for undefined", () => {
    expect(isHudMessage(undefined)).toBe(false);
  });

  test("returns false for string", () => {
    expect(isHudMessage("hello")).toBe(false);
  });

  test("returns false for number", () => {
    expect(isHudMessage(42)).toBe(false);
  });

  test("returns false for array", () => {
    expect(isHudMessage([{ type: "test" }])).toBe(false);
  });

  test("returns false for object without type field", () => {
    expect(isHudMessage({ foo: "bar" })).toBe(false);
  });

  test("returns false for object with non-string type", () => {
    expect(isHudMessage({ type: 123 })).toBe(false);
    expect(isHudMessage({ type: null })).toBe(false);
    expect(isHudMessage({ type: {} })).toBe(false);
  });

  test("returns true for any string type value", () => {
    expect(isHudMessage({ type: "unknown_type" })).toBe(true);
    expect(isHudMessage({ type: "" })).toBe(true);
  });
});

describe("serializeHudMessage", () => {
  test("serializes session_start message", () => {
    const msg: SessionStartMessage = {
      type: "session_start",
      sessionId: "test-123",
      timestamp: "2024-01-01T00:00:00Z",
    };
    const serialized = serializeHudMessage(msg);
    expect(serialized).toBe(JSON.stringify(msg));
  });

  test("serializes task_selected message", () => {
    const msg: TaskSelectedMessage = {
      type: "task_selected",
      task: {
        id: "oa-abc123",
        title: "Test Task",
        status: "in_progress",
        priority: 1,
      },
    };
    const serialized = serializeHudMessage(msg);
    expect(serialized).toContain('"type":"task_selected"');
    expect(serialized).toContain('"id":"oa-abc123"');
  });

  test("serializes phase_change message", () => {
    const msg: PhaseChangeMessage = {
      type: "phase_change",
      phase: "decomposing",
    };
    const serialized = serializeHudMessage(msg);
    expect(serialized).toBe('{"type":"phase_change","phase":"decomposing"}');
  });

  test("serializes text_output message", () => {
    const msg: TextOutputMessage = {
      type: "text_output",
      text: "Hello world",
      source: "claude-code",
    };
    const serialized = serializeHudMessage(msg);
    const parsed = JSON.parse(serialized);
    expect(parsed.type).toBe("text_output");
    expect(parsed.text).toBe("Hello world");
    expect(parsed.source).toBe("claude-code");
  });

  test("serializes tool_call message with arguments", () => {
    const msg: ToolCallMessage = {
      type: "tool_call",
      toolName: "read",
      arguments: JSON.stringify({ path: "/tmp/test.txt" }),
      callId: "call-123",
    };
    const serialized = serializeHudMessage(msg);
    expect(serialized).toContain('"toolName":"read"');
    expect(serialized).toContain('"callId":"call-123"');
  });

  test("serializes tool_result message", () => {
    const msg: ToolResultMessage = {
      type: "tool_result",
      toolName: "read",
      result: JSON.stringify({ content: "file contents" }),
      isError: false,
      callId: "call-123",
    };
    const serialized = serializeHudMessage(msg);
    expect(serialized).toContain('"isError":false');
  });
});

describe("parseHudMessage", () => {
  test("parses valid JSON with type field", () => {
    const json = '{"type":"session_start","sessionId":"abc","timestamp":"2024-01-01"}';
    const parsed = parseHudMessage(json);
    expect(parsed).not.toBeNull();
    expect(parsed?.type).toBe("session_start");
    expect((parsed as SessionStartMessage).sessionId).toBe("abc");
  });

  test("parses task_selected message", () => {
    const msg: TaskSelectedMessage = {
      type: "task_selected",
      task: { id: "t1", title: "Test", status: "open", priority: 2 },
    };
    const parsed = parseHudMessage(JSON.stringify(msg));
    expect(parsed).not.toBeNull();
    expect(parsed?.type).toBe("task_selected");
    expect((parsed as TaskSelectedMessage).task.id).toBe("t1");
  });

  test("returns null for invalid JSON", () => {
    expect(parseHudMessage("not json")).toBeNull();
    expect(parseHudMessage("{broken")).toBeNull();
    expect(parseHudMessage("")).toBeNull();
  });

  test("returns null for valid JSON without type field", () => {
    expect(parseHudMessage('{"foo":"bar"}')).toBeNull();
    expect(parseHudMessage("{}")).toBeNull();
  });

  test("returns null for valid JSON with non-string type", () => {
    expect(parseHudMessage('{"type":123}')).toBeNull();
    expect(parseHudMessage('{"type":null}')).toBeNull();
  });

  test("roundtrips message through serialize and parse", () => {
    const original: HudMessage = {
      type: "commit_created",
      sha: "abc123def",
      message: "feat: add new feature",
    };
    const serialized = serializeHudMessage(original);
    const parsed = parseHudMessage(serialized);
    expect(parsed).toEqual(original);
  });

  test("parses all message types", () => {
    const messages: HudMessage[] = [
      { type: "session_start", sessionId: "s1", timestamp: "2024-01-01" },
      { type: "session_complete", success: true, summary: "Done" },
      { type: "task_selected", task: { id: "t1", title: "T", status: "open", priority: 1 } },
      { type: "task_decomposed", subtasks: [] },
      { type: "subtask_start", subtask: { id: "st1", description: "D", status: "pending" } },
      {
        type: "subtask_complete",
        subtask: { id: "st1", description: "D", status: "done" },
        result: { success: true, filesModified: [], turns: 1 },
      },
      { type: "subtask_failed", subtask: { id: "st1", description: "D", status: "failed" }, error: "Err" },
      { type: "verification_start", command: "bun test" },
      { type: "verification_complete", command: "bun test", passed: true },
      { type: "commit_created", sha: "abc", message: "msg" },
      { type: "push_complete", branch: "main" },
      { type: "phase_change", phase: "idle" },
      { type: "error", phase: "executing_subtask", error: "Something went wrong" },
      { type: "text_output", text: "hello" },
      { type: "tool_call", toolName: "bash", arguments: "{}" },
      { type: "tool_result", toolName: "bash", result: "{}", isError: false },
    ];

    for (const msg of messages) {
      const serialized = serializeHudMessage(msg);
      const parsed = parseHudMessage(serialized);
      expect(parsed).toEqual(msg);
    }
  });
});
