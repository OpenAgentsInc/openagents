import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { appendRunEventSync, sanitizeEvent } from "./runLog.js";

describe("runLog", () => {
  test("sanitizes secrets in events", () => {
    const event = sanitizeEvent({
      type: "tool_call",
      ts: "now",
      tool: "bash",
      toolCallId: "1",
      args: { token: "super-secret-token", cmd: "echo ok" },
    });

    expect((event as any).args.token).toBe("[redacted]");
  });

  test("writes events to JSONL with sanitization", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "runlog-test-"));
    const runId = "run-test";
    const event = {
      type: "llm_response" as const,
      ts: "now",
      turn: 1,
      hasToolCalls: true,
      message: { role: "assistant", content: "hi", secret: "sk-abc123" },
      toolCalls: [{ id: "tc1", name: "read", arguments: "{\"path\":\"/tmp\"}" }],
    };

    appendRunEventSync(dir, runId, event);

    const dateDir = fs.readdirSync(dir)[0];
    const filePath = path.join(dir, dateDir, `${runId}.jsonl`);
    const content = fs.readFileSync(filePath, "utf8");
    expect(content).toContain("[redacted]");
    const parsed = JSON.parse(content.trim());
    expect(parsed.type).toBe("llm_response");
  });
});
