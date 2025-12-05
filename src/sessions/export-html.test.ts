import { describe, expect, test } from "bun:test";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { exportSessionToHtml, parseSessionFile, renderSessionHtml } from "./export-html.js";
import type { SessionEntry } from "./schema.js";

const sampleEntries: SessionEntry[] = [
  {
    type: "session_start",
    uuid: "u1",
    parentUuid: null,
    timestamp: "2025-12-05T10:00:00.000Z",
    sessionId: "session-test",
    taskId: "oa-123",
    cwd: "/tmp/repo",
    model: "x-ai/grok-4.1-fast:free",
    provider: "openrouter",
    version: "1",
    gitBranch: "main",
  },
  {
    type: "user",
    uuid: "u2",
    parentUuid: "u1",
    timestamp: "2025-12-05T10:00:05.000Z",
    sessionId: "session-test",
    message: { role: "user", content: "Hello world" },
  },
  {
    type: "assistant",
    uuid: "u3",
    parentUuid: "u2",
    timestamp: "2025-12-05T10:00:06.000Z",
    sessionId: "session-test",
    message: {
      role: "assistant",
      content: [
        { type: "text", text: "Working on it" },
        { type: "tool_use", id: "tool-1", name: "bash", input: { command: "ls" } },
      ],
    },
    usage: { inputTokens: 10, outputTokens: 20 },
  },
  {
    type: "tool_result",
    uuid: "u4",
    parentUuid: "u3",
    timestamp: "2025-12-05T10:00:07.000Z",
    sessionId: "session-test",
    message: {
      role: "user",
      content: [{ type: "tool_result", tool_use_id: "tool-1", content: "file.txt", is_error: false }],
    },
  },
  {
    type: "session_end",
    uuid: "u5",
    parentUuid: "u4",
    timestamp: "2025-12-05T10:00:08.000Z",
    sessionId: "session-test",
    outcome: "success",
    totalTurns: 1,
    totalUsage: { inputTokens: 10, outputTokens: 20 },
    filesModified: ["README.md"],
    commits: ["abc123"],
  },
];

describe("session export-html", () => {
  test("parseSessionFile decodes JSONL entries", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "session-export-"));
    const jsonlPath = path.join(dir, "session.jsonl");
    fs.writeFileSync(
      jsonlPath,
      sampleEntries.map((e) => JSON.stringify(e)).join("\n"),
      "utf-8",
    );

    const parsed = parseSessionFile(jsonlPath);
    expect(parsed).toHaveLength(sampleEntries.length);
    expect(parsed[0]?.type).toBe("session_start");
    expect(parsed[1]?.type).toBe("user");

    fs.rmSync(dir, { recursive: true });
  });

  test("renderSessionHtml includes metadata and content", () => {
    const html = renderSessionHtml(sampleEntries, "session.jsonl");
    expect(html).toContain("Session session-test");
    expect(html).toContain("oa-123");
    expect(html).toContain("bash");
    expect(html).toContain("Hello world");
    expect(html).toContain("file.txt");
  });

  test("exportSessionToHtml writes HTML file", () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "session-export-"));
    const jsonlPath = path.join(dir, "session.jsonl");
    fs.writeFileSync(
      jsonlPath,
      sampleEntries.map((e) => JSON.stringify(e)).join("\n"),
      "utf-8",
    );

    const outPath = exportSessionToHtml(jsonlPath);
    expect(fs.existsSync(outPath)).toBe(true);

    const html = fs.readFileSync(outPath, "utf-8");
    expect(html).toContain("Session session-test");
    expect(html).toContain("Working on it");

    fs.rmSync(dir, { recursive: true });
  });
});
