import { describe, expect, it } from "bun:test";
import { mkdtempSync, existsSync } from "fs";
import { join } from "path";
import { SessionManager } from "./session-manager.js";

describe("SessionManager", () => {
  it("writes and loads session entries", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const mgr = new SessionManager({ baseDir: dir });
    mgr.start("openai", "gpt-5");
    mgr.saveMessage({ role: "user", content: "hello" });
    const sessions = mgr.listSessions();
    expect(sessions.length).toBe(1);
    expect(existsSync(sessions[0])).toBe(true);
    const entries = mgr.load(sessions[0]);
    expect(entries[0]).toHaveProperty("type", "session");
    expect((entries[1] as any).message?.role).toBe("user");
  });

  it("can be disabled with --no-session", () => {
    const mgr = new SessionManager();
    mgr.disable();
    mgr.start();
    mgr.saveMessage({ role: "user", content: "hi" });
    expect(mgr.listSessions().length).toBe(0);
  });

  it("buffers messages before start and flushes on start", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const mgr = new SessionManager({ baseDir: dir });
    mgr.saveMessage({ role: "user", content: "prefill" });
    expect(mgr.listSessions().length).toBe(0);

    mgr.start("openai", "gpt-4o", "medium");
    const sessions = mgr.listSessions();
    expect(sessions.length).toBe(1);
    const entries = mgr.load(sessions[0]);
    expect(entries.find((e) => e.type === "session")).toBeDefined();
    expect(entries.find((e) => e.type === "message")).toBeDefined();
  });

  it("continues most recent session when requested", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const first = new SessionManager({ baseDir: dir });
    first.start("openai", "gpt-4");
    first.saveMessage({ role: "user", content: "first" });
    const sessionPath = first.getSessionFile();

    const resumed = new SessionManager({ baseDir: dir, continueSession: true });
    expect(resumed.getSessionFile()).toBe(sessionPath);
  });

  it("uses custom session path when provided", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const custom = join(dir, "custom.jsonl");
    const mgr = new SessionManager({ sessionPath: custom });
    mgr.start("openai", "gpt-4");
    mgr.saveMessage({ role: "user", content: "custom path" });
    expect(mgr.getSessionFile()).toBe(custom);
    expect(existsSync(custom)).toBe(true);
  });
});
