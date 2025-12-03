import { describe, expect, it } from "bun:test";
import { mkdtempSync } from "fs";
import { SessionManager } from "./session-manager.js";
import { findLatestSession, getSessionSummaries } from "./session-selector.js";

describe("session selector", () => {
  it("returns latest session", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const prevCwd = process.cwd();
    process.chdir(dir);

    const first = new SessionManager(dir);
    first.start("openai", "gpt-4");
    first.saveMessage("user", "first message");

    const second = new SessionManager(dir);
    second.start("openai", "gpt-4");
    second.saveMessage("user", "second message");

    const latest = findLatestSession({ baseDir: dir, cwd: dir });
    const sessions = new SessionManager(dir).listSessions();

    process.chdir(prevCwd);

    expect(latest).toBe(sessions[0]);
  });

  it("summarizes sessions with labels", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const prevCwd = process.cwd();
    process.chdir(dir);

    const session = new SessionManager(dir);
    session.start("openai", "gpt-4");
    session.saveMessage("user", "implement feature xyz");

    const summaries = getSessionSummaries({ baseDir: dir, cwd: dir });

    process.chdir(prevCwd);

    expect(summaries.length).toBeGreaterThan(0);
    expect(summaries[0].path.endsWith(".jsonl")).toBe(true);
    expect(summaries[0].label).toContain("implement feature xyz");
  });
});
