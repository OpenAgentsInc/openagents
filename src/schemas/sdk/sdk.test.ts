/**
 * Comprehensive SDK schema integration tests.
 *
 * Tests encoding/decoding roundtrips, type guards, and validation edge cases
 * across all SDK schemas. Individual modules (agents, hooks) have their own
 * dedicated test files.
 */

import { describe, expect, test } from "bun:test";
import * as S from "effect/Schema";
import {
  // Tool inputs
  BashInput,
  FileEditInput,
  FileReadInput,
  FileWriteInput,
  GlobInput,
  GrepInput,
  GrepOutputMode,
  // Tool outputs
  BashOutput,
  EditOutput,
  GrepContentOutput,
  GrepCountOutput,
  GrepFilesOutput,
  GrepOutput,
  ReadOutput,
  TextContent,
  ToolContent,
  ToolError,
  WriteOutput,
  isTextContent,
  // Messages
  ChatMessage,
  ContentBlock,
  MessageRole,
  SDKAssistantMessage,
  SDKMessage,
  SDKResultMessage,
  SDKUserMessage,
  TextBlock,
  ToolResultBlock,
  ToolUseBlock,
  isSDKAssistantMessage,
  isSDKResultMessage,
  isSDKUserMessage,
  isTextBlock,
  isToolResultBlock,
  isToolUseBlock,
  // Permissions
  AllowPermissionResult,
  DenyPermissionResult,
  PermissionBehavior,
  PermissionMode,
  PermissionResult,
} from "./index.js";

describe("SDK Schema Integration Tests", () => {
  describe("Tool Input Schemas", () => {
    describe("File Operations", () => {
      test("FileReadInput: encoding/decoding roundtrip", () => {
        const input = { file_path: "/path/to/file.ts", offset: 10, limit: 50 };

        const encoded = S.encodeSync(FileReadInput)(input);
        const decoded = S.decodeUnknownSync(FileReadInput)(encoded);

        expect(decoded).toEqual(input);
      });

      test("FileEditInput: validates required fields", () => {
        const valid = {
          file_path: "src/test.ts",
          old_string: "const x = 1",
          new_string: "const x = 2",
        };

        expect(() => S.decodeUnknownSync(FileEditInput)(valid)).not.toThrow();

        // Missing required field
        expect(() =>
          S.decodeUnknownSync(FileEditInput)({
            file_path: "test.ts",
            old_string: "old",
          })
        ).toThrow();
      });

      test("FileWriteInput: accepts content string", () => {
        const input = {
          file_path: "/new/file.ts",
          content: "export const foo = 'bar';",
        };

        const decoded = S.decodeUnknownSync(FileWriteInput)(input);
        expect(decoded.content).toBe("export const foo = 'bar';");
      });
    });

    describe("Shell Operations", () => {
      test("BashInput: accepts command and optional fields", () => {
        const minimal = { command: "ls -la" };
        const complete = {
          command: "npm test",
          description: "Run tests",
          timeout: 30000,
          run_in_background: true,
        };

        expect(() => S.decodeUnknownSync(BashInput)(minimal)).not.toThrow();
        expect(() => S.decodeUnknownSync(BashInput)(complete)).not.toThrow();

        const decoded = S.decodeUnknownSync(BashInput)(complete);
        expect(decoded.timeout).toBe(30000);
      });
    });

    describe("Search Operations", () => {
      test("GrepInput: validates output_mode enum", () => {
        const validModes = ["content", "files_with_matches", "count"];

        for (const mode of validModes) {
          const input = { pattern: "test", output_mode: mode };
          expect(() => S.decodeUnknownSync(GrepInput)(input)).not.toThrow();
        }

        // Invalid mode
        expect(() =>
          S.decodeUnknownSync(GrepInput)({
            pattern: "test",
            output_mode: "invalid",
          })
        ).toThrow();
      });

      test("GlobInput: accepts pattern and optional path", () => {
        const minimal = { pattern: "**/*.ts" };
        const withPath = { pattern: "*.js", path: "/src" };

        const decoded1 = S.decodeUnknownSync(GlobInput)(minimal);
        const decoded2 = S.decodeUnknownSync(GlobInput)(withPath);

        expect(decoded1.pattern).toBe("**/*.ts");
        expect(decoded2.path).toBe("/src");
      });
    });
  });

  describe("Tool Output Schemas", () => {
    describe("File Operation Outputs", () => {
      test("ReadOutput: content array structure", () => {
        const output = {
          content: [{ type: "text", text: "file contents here" }],
        };

        const decoded = S.decodeUnknownSync(ReadOutput)(output);
        expect(decoded.content.length).toBe(1);
        expect(isTextContent(decoded.content[0])).toBe(true);
      });

      test("EditOutput: structured output with replacements", () => {
        const output = {
          message: "File edited successfully",
          file_path: "src/test.ts",
          replacements: 1,
        };

        const decoded = S.decodeUnknownSync(EditOutput)(output);
        expect(decoded.message).toContain("success");
        expect(decoded.file_path).toBe("src/test.ts");
        expect(decoded.replacements).toBe(1);
      });

      test("WriteOutput: structured output with path", () => {
        const output = {
          message: "File written successfully",
          file_path: "/new/file.ts",
          bytes_written: 1234,
        };

        const decoded = S.decodeUnknownSync(WriteOutput)(output);
        expect(decoded.message).toContain("success");
        expect(decoded.bytes_written).toBe(1234);
      });

      test("ToolError: error with reason and message", () => {
        const error = {
          reason: "command_failed",
          message: "Pattern not found",
        };

        expect(() => S.decodeUnknownSync(ToolError)(error)).not.toThrow();
      });
    });

    describe("Shell Operation Outputs", () => {
      test("BashOutput: captures stdout and exit code", () => {
        const output = {
          output: "Command output here\nAnother line",
          exit_code: 0,
          shell_id: "shell-123",
        };

        const decoded = S.decodeUnknownSync(BashOutput)(output);
        expect(decoded.output).toContain("Command output");
        expect(decoded.exit_code).toBe(0);
        expect(decoded.shell_id).toBe("shell-123");
      });
    });

    describe("Search Operation Outputs", () => {
      test("GrepOutput: union of output modes", () => {
        const contentOutput = {
          mode: "content",
          matches: [{ file: "test.ts", line_number: 10, line: "match here" }],
          total_matches: 1,
        };

        const filesOutput = {
          mode: "files_with_matches",
          files: ["file1.ts", "file2.ts"],
          total_files: 2,
        };

        const countOutput = {
          mode: "count",
          counts: [{ file: "file.ts", count: 5 }],
          total: 5,
        };

        expect(() => S.decodeUnknownSync(GrepContentOutput)(contentOutput)).not.toThrow();
        expect(() => S.decodeUnknownSync(GrepFilesOutput)(filesOutput)).not.toThrow();
        expect(() => S.decodeUnknownSync(GrepCountOutput)(countOutput)).not.toThrow();

        // Test union
        expect(() => S.decodeUnknownSync(GrepOutput)(contentOutput)).not.toThrow();
        expect(() => S.decodeUnknownSync(GrepOutput)(filesOutput)).not.toThrow();
        expect(() => S.decodeUnknownSync(GrepOutput)(countOutput)).not.toThrow();
      });
    });

    describe("ToolContent", () => {
      test("TextContent: type guard correctness", () => {
        const textContent = { type: "text" as const, text: "Hello" };
        const imageContent = {
          type: "image" as const,
          data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
          mimeType: "image/png",
        };

        const decodedText = S.decodeUnknownSync(TextContent)(textContent);
        const decodedImage = S.decodeUnknownSync(ToolContent)(imageContent);

        expect(isTextContent(decodedText)).toBe(true);
        expect(isTextContent(decodedImage)).toBe(false);
      });
    });
  });

  describe("Message Schemas", () => {
    describe("Content Blocks", () => {
      test("TextBlock: basic structure", () => {
        const block = { type: "text", text: "Hello, world!" };

        const decoded = S.decodeUnknownSync(TextBlock)(block);
        expect(decoded.type).toBe("text");
        expect(decoded.text).toBe("Hello, world!");
        expect(isTextBlock(decoded)).toBe(true);
      });

      test("ToolUseBlock: tool call structure", () => {
        const block = {
          type: "tool_use",
          id: "tool-1",
          name: "Read",
          input: { file_path: "test.ts" },
        };

        const decoded = S.decodeUnknownSync(ToolUseBlock)(block);
        expect(decoded.name).toBe("Read");
        expect(isToolUseBlock(decoded)).toBe(true);
      });

      test("ToolResultBlock: tool result structure", () => {
        const block = {
          type: "tool_result",
          tool_use_id: "tool-1",
          content: [{ type: "text", text: "Success" }],
        };

        const decoded = S.decodeUnknownSync(ToolResultBlock)(block);
        expect(decoded.tool_use_id).toBe("tool-1");
        expect(isToolResultBlock(decoded)).toBe(true);
      });

      test("ContentBlock: union discrimination", () => {
        const textBlock = { type: "text", text: "Text" };
        const toolBlock = { type: "tool_use", id: "t1", name: "Edit", input: {} };

        const decoded1 = S.decodeUnknownSync(ContentBlock)(textBlock);
        const decoded2 = S.decodeUnknownSync(ContentBlock)(toolBlock);

        expect(isTextBlock(decoded1)).toBe(true);
        expect(isToolUseBlock(decoded2)).toBe(true);
      });
    });

    describe("SDK Messages", () => {
      test("SDKUserMessage: validates structure", () => {
        const message = {
          type: "user",
          session_id: "sess-123",
          message: {
            role: "user",
            content: "Hello!",
          },
          parent_tool_use_id: null,
        };

        const decoded = S.decodeUnknownSync(SDKUserMessage)(message);
        expect(decoded.type).toBe("user");
        expect(isSDKUserMessage(decoded)).toBe(true);
      });

      test("SDKAssistantMessage: validates structure", () => {
        const message = {
          type: "assistant",
          uuid: "msg-123",
          session_id: "sess-123",
          message: { role: "assistant", content: "Hello!" },
          parent_tool_use_id: null,
        };

        const decoded = S.decodeUnknownSync(SDKAssistantMessage)(message);
        expect(decoded.uuid).toBe("msg-123");
        expect(isSDKAssistantMessage(decoded)).toBe(true);
      });

      test("SDKResultMessage: complete structure", () => {
        const success = {
          type: "result",
          subtype: "success",
          uuid: "result-123",
          session_id: "sess-123",
          duration_ms: 1500,
          duration_api_ms: 1200,
          is_error: false,
          num_turns: 5,
          total_cost_usd: 0.0123,
          usage: { inputTokens: 1000, outputTokens: 500 },
          permission_denials: [],
        };

        const decoded = S.decodeUnknownSync(SDKResultMessage)(success);
        expect(decoded.subtype).toBe("success");
        expect(decoded.num_turns).toBe(5);
        expect(isSDKResultMessage(decoded)).toBe(true);
      });
    });

    describe("Chat Messages", () => {
      test("ChatMessage: role validation", () => {
        const validRoles = ["user", "assistant"];

        for (const role of validRoles) {
          const message = { role, content: "Test" };
          expect(() => S.decodeUnknownSync(ChatMessage)(message)).not.toThrow();
        }
      });

      test("MessageRole: literal type", () => {
        expect(() => S.decodeUnknownSync(MessageRole)("user")).not.toThrow();
        expect(() => S.decodeUnknownSync(MessageRole)("assistant")).not.toThrow();
        expect(() => S.decodeUnknownSync(MessageRole)("system")).not.toThrow();
        expect(() => S.decodeUnknownSync(MessageRole)("invalid")).toThrow();
      });
    });
  });

  describe("Permission Schemas", () => {
    test("PermissionMode: valid modes", () => {
      const modes = ["default", "acceptEdits", "bypassPermissions", "plan"];

      for (const mode of modes) {
        expect(() => S.decodeUnknownSync(PermissionMode)(mode)).not.toThrow();
      }
    });

    test("PermissionBehavior: allow/deny/ask", () => {
      const behaviors = ["allow", "deny", "ask"];

      for (const behavior of behaviors) {
        expect(() => S.decodeUnknownSync(PermissionBehavior)(behavior)).not.toThrow();
      }
    });

    test("PermissionResult: allow and deny variants", () => {
      const allow = {
        behavior: "allow",
        updatedInput: { file_path: "test.ts" },
      };
      const deny = {
        behavior: "deny",
        message: "Not safe",
        interrupt: false,
      };

      expect(() => S.decodeUnknownSync(AllowPermissionResult)(allow)).not.toThrow();
      expect(() => S.decodeUnknownSync(DenyPermissionResult)(deny)).not.toThrow();
      expect(() => S.decodeUnknownSync(PermissionResult)(allow)).not.toThrow();
      expect(() => S.decodeUnknownSync(PermissionResult)(deny)).not.toThrow();
    });
  });

  describe("Edge Cases and Validation", () => {
    test("Empty strings are rejected where inappropriate", () => {
      expect(() =>
        S.decodeUnknownSync(FileReadInput)({ file_path: "" })
      ).toThrow();

      expect(() =>
        S.decodeUnknownSync(BashInput)({ command: "" })
      ).toThrow();

      expect(() => S.decodeUnknownSync(GrepInput)({ pattern: "" })).toThrow();
    });

    test("Optional fields can be omitted", () => {
      const fileRead = { file_path: "/test.ts" };
      const bash = { command: "ls" };
      const grep = { pattern: "test" };

      expect(() => S.decodeUnknownSync(FileReadInput)(fileRead)).not.toThrow();
      expect(() => S.decodeUnknownSync(BashInput)(bash)).not.toThrow();
      expect(() => S.decodeUnknownSync(GrepInput)(grep)).not.toThrow();
    });

    test("Numeric fields validate ranges", () => {
      // Negative offset should fail
      expect(() =>
        S.decodeUnknownSync(FileReadInput)({
          file_path: "/test.ts",
          offset: -1,
        })
      ).toThrow();

      // Negative limit should fail
      expect(() =>
        S.decodeUnknownSync(FileReadInput)({
          file_path: "/test.ts",
          limit: -10,
        })
      ).toThrow();
    });

    test("Union types properly discriminate", () => {
      const grepContentMode = "content";
      const grepFilesMode = "files_with_matches";
      const grepCountMode = "count";

      expect(() => S.decodeUnknownSync(GrepOutputMode)(grepContentMode)).not.toThrow();
      expect(() => S.decodeUnknownSync(GrepOutputMode)(grepFilesMode)).not.toThrow();
      expect(() => S.decodeUnknownSync(GrepOutputMode)(grepCountMode)).not.toThrow();
    });
  });
});
