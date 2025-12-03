import { describe, expect, it } from "bun:test";
import { mkdtempSync, existsSync } from "fs";
import { join } from "path";
import { SessionManager } from "./session-manager.js";

describe("SessionManager", () => {
  it("writes and loads session entries", () => {
    const dir = mkdtempSync("/tmp/sessions-");
    const mgr = new SessionManager(dir);
    mgr.start("openai", "gpt-5");
    mgr.saveMessage("user", "hello");
    const sessions = mgr.listSessions();
    expect(sessions.length).toBe(1);
    expect(existsSync(sessions[0])).toBe(true);
    const entries = mgr.load(sessions[0]);
    expect(entries[0]).toHaveProperty("type", "session");
    expect(entries[1]).toHaveProperty("role", "user");
  });

  it("can be disabled with --no-session", () => {
    const mgr = new SessionManager();
    mgr.disable();
    mgr.start();
    mgr.saveMessage("user", "hi");
    expect(mgr.listSessions().length).toBe(0);
  });
});
