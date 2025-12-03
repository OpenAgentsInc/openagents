import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import {
  AsyncHookJSONOutput,
  BaseHookInput,
  HookEvent,
  HookInput,
  HookJSONOutput,
  PostToolUseHookInput,
  PreToolUseHookInput,
  SessionEndHookInput,
  SessionStartHookInput,
  SyncHookJSONOutput,
  isPostToolUseHookInput,
  isPreToolUseHookInput,
  isSessionEndHookInput,
  isSessionStartHookInput,
} from "./hooks.js";

describe("Hook Schemas", () => {
  describe("HookEvent", () => {
    test("accepts valid hook event names", () => {
      const events = [
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "UserPromptSubmit",
        "SessionStart",
        "SessionEnd",
        "Stop",
        "SubagentStop",
        "PreCompact",
      ];

      for (const event of events) {
        expect(() => S.decodeUnknownSync(HookEvent)(event)).not.toThrow();
      }
    });

    test("rejects invalid hook event names", () => {
      expect(() => S.decodeUnknownSync(HookEvent)("InvalidEvent")).toThrow();
    });
  });

  describe("BaseHookInput", () => {
    test("decodes valid base hook input", () => {
      const input = {
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
      };

      const decoded = S.decodeUnknownSync(BaseHookInput)(input);
      expect(decoded.session_id).toBe("sess-123");
      expect(decoded.transcript_path).toBe("/path/to/transcript");
      expect(decoded.cwd).toBe("/workspace");
    });

    test("decodes base hook input with optional permission_mode", () => {
      const input = {
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        permission_mode: "bypassPermissions",
      };

      const decoded = S.decodeUnknownSync(BaseHookInput)(input);
      expect(decoded.permission_mode).toBe("bypassPermissions");
    });
  });

  describe("PreToolUseHookInput", () => {
    test("decodes valid PreToolUse hook input", () => {
      const input = {
        hook_event_name: "PreToolUse",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        tool_name: "Edit",
        tool_input: { file_path: "test.ts", old_string: "old", new_string: "new" },
      };

      const decoded = S.decodeUnknownSync(PreToolUseHookInput)(input);
      expect(decoded.hook_event_name).toBe("PreToolUse");
      expect(decoded.tool_name).toBe("Edit");
      expect(decoded.tool_input).toEqual({ file_path: "test.ts", old_string: "old", new_string: "new" });
    });
  });

  describe("PostToolUseHookInput", () => {
    test("decodes valid PostToolUse hook input", () => {
      const input = {
        hook_event_name: "PostToolUse",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        tool_name: "Edit",
        tool_input: { file_path: "test.ts" },
        tool_response: { success: true },
      };

      const decoded = S.decodeUnknownSync(PostToolUseHookInput)(input);
      expect(decoded.hook_event_name).toBe("PostToolUse");
      expect(decoded.tool_name).toBe("Edit");
      expect(decoded.tool_response).toEqual({ success: true });
    });
  });

  describe("SessionStartHookInput", () => {
    test("decodes valid SessionStart hook input", () => {
      const input = {
        hook_event_name: "SessionStart",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        source: "startup",
      };

      const decoded = S.decodeUnknownSync(SessionStartHookInput)(input);
      expect(decoded.hook_event_name).toBe("SessionStart");
      expect(decoded.source).toBe("startup");
    });

    test("accepts all valid source values", () => {
      const sources = ["startup", "resume", "clear", "compact"];

      for (const source of sources) {
        const input = {
          hook_event_name: "SessionStart",
          session_id: "sess-123",
          transcript_path: "/path/to/transcript",
          cwd: "/workspace",
          source,
        };

        expect(() => S.decodeUnknownSync(SessionStartHookInput)(input)).not.toThrow();
      }
    });
  });

  describe("SessionEndHookInput", () => {
    test("decodes valid SessionEnd hook input", () => {
      const input = {
        hook_event_name: "SessionEnd",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        reason: "logout",
      };

      const decoded = S.decodeUnknownSync(SessionEndHookInput)(input);
      expect(decoded.hook_event_name).toBe("SessionEnd");
      expect(decoded.reason).toBe("logout");
    });
  });

  describe("HookInput union", () => {
    test("decodes PreToolUseHookInput", () => {
      const input = {
        hook_event_name: "PreToolUse",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        tool_name: "Read",
        tool_input: { file_path: "test.ts" },
      };

      const decoded = S.decodeUnknownSync(HookInput)(input);
      expect(isPreToolUseHookInput(decoded)).toBe(true);
    });

    test("decodes PostToolUseHookInput", () => {
      const input = {
        hook_event_name: "PostToolUse",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        tool_name: "Write",
        tool_input: { file_path: "test.ts", content: "data" },
        tool_response: { success: true },
      };

      const decoded = S.decodeUnknownSync(HookInput)(input);
      expect(isPostToolUseHookInput(decoded)).toBe(true);
    });
  });

  describe("Type guards", () => {
    test("isPreToolUseHookInput correctly identifies PreToolUse inputs", () => {
      const input = S.decodeUnknownSync(PreToolUseHookInput)({
        hook_event_name: "PreToolUse",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        tool_name: "Edit",
        tool_input: {},
      });

      expect(isPreToolUseHookInput(input)).toBe(true);
      expect(isPostToolUseHookInput(input)).toBe(false);
    });

    test("isSessionStartHookInput correctly identifies SessionStart inputs", () => {
      const input = S.decodeUnknownSync(SessionStartHookInput)({
        hook_event_name: "SessionStart",
        session_id: "sess-123",
        transcript_path: "/path/to/transcript",
        cwd: "/workspace",
        source: "startup",
      });

      expect(isSessionStartHookInput(input)).toBe(true);
      expect(isSessionEndHookInput(input)).toBe(false);
    });
  });

  describe("HookJSONOutput", () => {
    test("decodes async hook output", () => {
      const output = {
        async: true,
        asyncTimeout: 30,
      };

      const decoded = S.decodeUnknownSync(AsyncHookJSONOutput)(output);
      expect(decoded.async).toBe(true);
      expect(decoded.asyncTimeout).toBe(30);
    });

    test("decodes sync hook output", () => {
      const output = {
        continue: true,
        suppressOutput: false,
      };

      const decoded = S.decodeUnknownSync(SyncHookJSONOutput)(output);
      expect(decoded.continue).toBe(true);
      expect(decoded.suppressOutput).toBe(false);
    });

    test("decodes sync hook output with PreToolUse specific fields", () => {
      const output = {
        continue: true,
        hookSpecificOutput: {
          hookEventName: "PreToolUse",
          permissionDecision: "allow",
          permissionDecisionReason: "Safe operation",
        },
      };

      const decoded = S.decodeUnknownSync(SyncHookJSONOutput)(output);
      expect(decoded.hookSpecificOutput?.hookEventName).toBe("PreToolUse");
    });

    test("decodes hook output union", () => {
      const asyncOutput = { async: true };
      const syncOutput = { continue: false, stopReason: "User requested" };

      expect(() => S.decodeUnknownSync(HookJSONOutput)(asyncOutput)).not.toThrow();
      expect(() => S.decodeUnknownSync(HookJSONOutput)(syncOutput)).not.toThrow();
    });
  });
});
