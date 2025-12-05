import { describe, expect, it } from "bun:test";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { exportSessionToHtml, renderSessionHtml } from "./session-export.js";

describe("session export", () => {
  it("renders HTML transcript from session entries", () => {
    const html = renderSessionHtml(
      [
        { type: "session", id: "session-1", timestamp: "2025-12-05T10:00:00Z", provider: "openai", model: "gpt-4", cwd: "/repo" },
        {
          type: "message",
          timestamp: "2025-12-05T10:01:00Z",
          message: { role: "user", content: "Hello" },
        },
        {
          type: "message",
          timestamp: "2025-12-05T10:02:00Z",
          message: { role: "assistant", content: "Hi there" },
        },
      ],
      "/tmp/session.jsonl",
    );

    expect(html).toContain("session-1");
    expect(html).toContain("openai");
    expect(html).toContain("Hello");
    expect(html).toContain("Hi there");
  });

  it("writes HTML file when exporting", () => {
    const dir = mkdtempSync("/tmp/session-export-");
    const sessionPath = join(dir, "session.jsonl");
    writeFileSync(
      sessionPath,
      [
        JSON.stringify({ type: "session", id: "session-2", timestamp: "2025-12-05T10:00:00Z", provider: "anthropic" }),
        JSON.stringify({ type: "message", timestamp: "2025-12-05T10:01:00Z", message: { role: "user", content: "Question" } }),
      ].join("\n"),
    );

    const out = exportSessionToHtml(sessionPath);
    expect(existsSync(out)).toBe(true);

    const html = readFileSync(out, "utf8");
    expect(html).toContain("session-2");
    expect(html).toContain("anthropic");
    expect(html).toContain("Question");
  });
});
