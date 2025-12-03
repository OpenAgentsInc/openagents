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

  describe("createBranchedSession", () => {
    it("creates a new session file with header from state", () => {
      const dir = mkdtempSync("/tmp/sessions-");
      const mgr = new SessionManager({ baseDir: dir });
      mgr.start("anthropic", "claude-3");

      const state = {
        model: { provider: "anthropic", id: "claude-3" },
        thinkingLevel: "high",
        messages: [
          { role: "user", content: "hello" },
          { role: "assistant", content: "hi there" },
        ],
      };

      const branchedPath = mgr.createBranchedSession(state, -1);
      expect(existsSync(branchedPath)).toBe(true);

      const entries = mgr.load(branchedPath);
      expect(entries.length).toBe(1);
      expect(entries[0]).toHaveProperty("type", "session");
      const header = entries[0] as any;
      expect(header.provider).toBe("anthropic");
      expect(header.model).toBe("claude-3");
      expect(header.thinkingLevel).toBe("high");
    });

    it("copies messages up to branchFromIndex", () => {
      const dir = mkdtempSync("/tmp/sessions-");
      const mgr = new SessionManager({ baseDir: dir });
      mgr.start("openai", "gpt-4");

      const state = {
        model: { provider: "openai", id: "gpt-4" },
        thinkingLevel: "off",
        messages: [
          { role: "user", content: "first" },
          { role: "assistant", content: "response 1" },
          { role: "user", content: "second" },
          { role: "assistant", content: "response 2" },
          { role: "user", content: "third" },
        ],
      };

      const branchedPath = mgr.createBranchedSession(state, 2);
      const entries = mgr.load(branchedPath);

      const messages = entries.filter((e) => e.type === "message");
      expect(messages.length).toBe(3);
      expect((messages[0] as any).message.content).toBe("first");
      expect((messages[1] as any).message.content).toBe("response 1");
      expect((messages[2] as any).message.content).toBe("second");
    });

    it("creates empty session (no messages) when branchFromIndex is -1", () => {
      const dir = mkdtempSync("/tmp/sessions-");
      const mgr = new SessionManager({ baseDir: dir });
      mgr.start("openai", "gpt-4");

      const state = {
        model: { provider: "openai", id: "gpt-4" },
        thinkingLevel: "off",
        messages: [
          { role: "user", content: "message" },
        ],
      };

      const branchedPath = mgr.createBranchedSession(state, -1);
      const entries = mgr.load(branchedPath);

      expect(entries.length).toBe(1);
      expect(entries[0]).toHaveProperty("type", "session");
    });

    it("copies all messages when branchFromIndex equals last index", () => {
      const dir = mkdtempSync("/tmp/sessions-");
      const mgr = new SessionManager({ baseDir: dir });
      mgr.start("openai", "gpt-4");

      const state = {
        model: { provider: "openai", id: "gpt-4" },
        thinkingLevel: "medium",
        messages: [
          { role: "user", content: "a" },
          { role: "assistant", content: "b" },
          { role: "user", content: "c" },
        ],
      };

      const branchedPath = mgr.createBranchedSession(state, 2);
      const entries = mgr.load(branchedPath);

      const messages = entries.filter((e) => e.type === "message");
      expect(messages.length).toBe(3);
    });

    it("creates unique session file per branch", () => {
      const dir = mkdtempSync("/tmp/sessions-");
      const mgr = new SessionManager({ baseDir: dir });
      mgr.start("openai", "gpt-4");

      const state = {
        model: { provider: "openai", id: "gpt-4" },
        thinkingLevel: "off",
        messages: [],
      };

      const branch1 = mgr.createBranchedSession(state, -1);
      const branch2 = mgr.createBranchedSession(state, -1);

      expect(branch1).not.toBe(branch2);
      expect(existsSync(branch1)).toBe(true);
      expect(existsSync(branch2)).toBe(true);
    });
  });
});
