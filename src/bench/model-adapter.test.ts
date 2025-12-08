/**
 * Unit tests for FM adapter functions in model-adapter.ts
 *
 * Tests cover:
 * - truncateMessagesForFM: Context truncation strategy
 * - parseToolCalls: All format parsing
 * - parseDescriptiveToolCall: Key=value parsing
 * - FM_MODEL_CONFIGS: Configuration structure
 */

import { describe, test, expect } from "bun:test";
import {
  truncateMessagesForFM,
  parseToolCalls,
  parseDescriptiveToolCall,
  FM_MODEL_CONFIGS,
  FM_MAX_CONTEXT_CHARS_DEFAULT,
  getFMContextLimit,
  createFMToolParseError,
  KNOWN_FM_TOOLS,
} from "./model-adapter.js";

// Test fixtures
type Message = { role: "system" | "user" | "assistant"; content: string };

describe("truncateMessagesForFM", () => {
  test("returns messages unchanged when within limit", () => {
    const messages: Message[] = [
      { role: "system", content: "Short system prompt" },
      { role: "user", content: "Short user message" },
    ];
    const result = truncateMessagesForFM(messages);
    expect(result).toHaveLength(2);
    expect(result[0].content).toBe("Short system prompt");
    expect(result[1].content).toBe("Short user message");
  });

  test("keeps system and recent message pairs, drops older history", () => {
    const messages: Message[] = [
      { role: "system", content: "A".repeat(100) },
      { role: "user", content: "B".repeat(100) },
      { role: "assistant", content: "C".repeat(100) },
      { role: "user", content: "D".repeat(100) },
      { role: "assistant", content: "E".repeat(100) },
      { role: "user", content: "F".repeat(100) },
    ];
    // Total: ~600 chars, over 400 limit
    const result = truncateMessagesForFM(messages, 400);
    // Should keep system (first) and recent message pairs (last 2-3 pairs)
    expect(result.length).toBeGreaterThan(2); // Should preserve recent pairs
    expect(result[0].role).toBe("system");
    // Last message should be preserved (may be truncated but should contain "F")
    const lastContent = result[result.length - 1].content;
    expect(lastContent.includes("F") || lastContent.includes("...[truncated]")).toBe(true);
  });

  test("truncates system message if too long", () => {
    const messages: Message[] = [
      { role: "system", content: "X".repeat(2000) },
      { role: "user", content: "Short" },
    ];
    const result = truncateMessagesForFM(messages, 500);
    // System should be truncated
    expect(result[0].content.length).toBeLessThan(500);
  });

  test("is deterministic - same input produces same output", () => {
    const messages: Message[] = [
      { role: "system", content: "System" },
      { role: "user", content: "User 1" },
      { role: "assistant", content: "Response 1" },
      { role: "user", content: "User 2" },
    ];
    const result1 = truncateMessagesForFM(messages);
    const result2 = truncateMessagesForFM(messages);
    expect(result1).toEqual(result2);
  });

  test("handles empty messages array", () => {
    const messages: Message[] = [];
    const result = truncateMessagesForFM(messages);
    expect(result).toHaveLength(0);
  });

  test("handles single system message", () => {
    const messages: Message[] = [
      { role: "system", content: "Only system" },
    ];
    const result = truncateMessagesForFM(messages);
    expect(result).toHaveLength(1);
    expect(result[0].role).toBe("system");
  });

  test("respects custom maxChars parameter", () => {
    const messages: Message[] = [
      { role: "system", content: "A".repeat(100) },
      { role: "user", content: "B".repeat(100) },
    ];
    // With 150 limit, should truncate
    const result = truncateMessagesForFM(messages, 150);
    const totalChars = result.reduce((sum, m) => sum + m.content.length, 0);
    expect(totalChars).toBeLessThanOrEqual(200); // Some overhead
  });
});

describe("parseToolCalls", () => {
  describe("<tool_call> tag format", () => {
    test("parses valid tool_call tag", () => {
      const text = `<tool_call>{"name":"write_file","arguments":{"path":"hello.txt","content":"Hello"}}</tool_call>`;
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("write_file");
      expect(calls[0].arguments.path).toBe("hello.txt");
      expect(calls[0].arguments.content).toBe("Hello");
    });

    test("parses multiple tool_call tags", () => {
      const text = `
        <tool_call>{"name":"read_file","arguments":{"path":"a.txt"}}</tool_call>
        <tool_call>{"name":"write_file","arguments":{"path":"b.txt","content":"data"}}</tool_call>
      `;
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(2);
      expect(calls[0].name).toBe("read_file");
      expect(calls[1].name).toBe("write_file");
    });

    test("handles malformed JSON in tool_call tag", () => {
      const text = `<tool_call>{"name":"write_file", invalid json}</tool_call>`;
      const calls = parseToolCalls(text);
      // Should skip malformed and return empty
      expect(calls).toHaveLength(0);
    });

    test("handles missing arguments field", () => {
      const text = `<tool_call>{"name":"read_file"}</tool_call>`;
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("read_file");
      expect(calls[0].arguments).toEqual({});
    });
  });

  describe("markdown JSON block format", () => {
    test("parses ```json block with tool_call wrapper", () => {
      const text = "```json\n{\"tool_call\":{\"name\":\"run_command\",\"arguments\":{\"command\":\"ls\"}}}\n```";
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("run_command");
      expect(calls[0].arguments.command).toBe("ls");
    });

    test("parses ```json block without wrapper", () => {
      const text = "```json\n{\"name\":\"write_file\",\"arguments\":{\"path\":\"test.txt\",\"content\":\"test\"}}\n```";
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("write_file");
    });

    test("parses response wrapper with descriptive tool call", () => {
      const text = "```json\n{\"response\":\"Using write_file tool with arguments: path=test.txt, content=hello\"}\n```";
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("write_file");
    });
  });

  describe("descriptive format fallback", () => {
    test("parses 'Using X tool with arguments:' format", () => {
      const text = "Using write_file tool with arguments: path=hello.txt, content=Hello, world!";
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("write_file");
      expect(calls[0].arguments.path).toBe("hello.txt");
    });
  });

  describe("edge cases", () => {
    test("returns empty array for text with no tool calls", () => {
      const text = "I will now complete the task. TASK_COMPLETE";
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(0);
    });

    test("handles text with partial/broken tool_call tags", () => {
      const text = "<tool_call>broken";
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(0);
    });

    test("picks first valid tool_call when multiple candidates exist", () => {
      const text = `
        <tool_call>invalid</tool_call>
        <tool_call>{"name":"write_file","arguments":{"path":"a.txt","content":"first"}}</tool_call>
        <tool_call>{"name":"write_file","arguments":{"path":"b.txt","content":"second"}}</tool_call>
      `;
      const calls = parseToolCalls(text);
      // Should have 2 valid ones (skips invalid)
      expect(calls.length).toBeGreaterThanOrEqual(1);
      expect(calls[0].arguments.path).toBe("a.txt");
    });

    test("handles extra whitespace and newlines", () => {
      const text = `
        <tool_call>
          {
            "name": "write_file",
            "arguments": {
              "path": "test.txt",
              "content": "content"
            }
          }
        </tool_call>
      `;
      const calls = parseToolCalls(text);
      expect(calls).toHaveLength(1);
      expect(calls[0].name).toBe("write_file");
    });
  });
});

describe("parseDescriptiveToolCall", () => {
  describe("write_file parsing", () => {
    test("parses path and content", () => {
      const text = "Using write_file tool with arguments: path=hello.txt, content=Hello, world!";
      const result = parseDescriptiveToolCall(text);
      expect(result).not.toBeNull();
      expect(result?.name).toBe("write_file");
      expect(result?.arguments.path).toBe("hello.txt");
      expect(result?.arguments.content).toBe("Hello, world!");
    });

    test("handles content with commas", () => {
      const text = "Using write_file tool with arguments: path=data.csv, content=a,b,c,d";
      const result = parseDescriptiveToolCall(text);
      expect(result?.arguments.content).toBe("a,b,c,d");
    });

    test("handles content with equals signs", () => {
      const text = "Using write_file tool with arguments: path=config.ini, content=key=value";
      const result = parseDescriptiveToolCall(text);
      expect(result?.arguments.content).toBe("key=value");
    });

    test("handles multiline content", () => {
      const text = "Using write_file tool with arguments: path=test.txt, content=line1\nline2\nline3";
      const result = parseDescriptiveToolCall(text);
      expect(result?.arguments.content).toContain("\n");
    });
  });

  describe("read_file parsing", () => {
    test("parses path", () => {
      const text = "Using read_file tool with arguments: path=src/main.ts";
      const result = parseDescriptiveToolCall(text);
      expect(result?.name).toBe("read_file");
      expect(result?.arguments.path).toBe("src/main.ts");
    });
  });

  describe("run_command parsing", () => {
    test("parses command", () => {
      const text = "Using run_command tool with arguments: command=ls -la";
      const result = parseDescriptiveToolCall(text);
      expect(result?.name).toBe("run_command");
      expect(result?.arguments.command).toBe("ls -la");
    });

    test("handles command with pipes", () => {
      const text = "Using run_command tool with arguments: command=cat file.txt | grep pattern";
      const result = parseDescriptiveToolCall(text);
      expect(result?.arguments.command).toContain("| grep");
    });
  });

  describe("edit_file parsing", () => {
    test("parses path, old_text, new_text", () => {
      const text = "Using edit_file tool with arguments: path=file.ts, old_text=foo, new_text=bar";
      const result = parseDescriptiveToolCall(text);
      expect(result?.name).toBe("edit_file");
      expect(result?.arguments.path).toBe("file.ts");
      expect(result?.arguments.old_text).toBe("foo");
      expect(result?.arguments.new_text).toBe("bar");
    });
  });

  describe("case insensitivity", () => {
    test("handles uppercase tool names", () => {
      const text = "USING READ_FILE TOOL WITH ARGUMENTS: path=data.json";
      const result = parseDescriptiveToolCall(text);
      expect(result?.name).toBe("read_file");
    });

    test("handles mixed case", () => {
      const text = "Using Write_File tool with arguments: path=test.txt, content=hello";
      const result = parseDescriptiveToolCall(text);
      expect(result?.name).toBe("write_file");
    });
  });

  describe("edge cases", () => {
    test("returns null for non-matching text", () => {
      const text = "This is just regular text without tool calls";
      const result = parseDescriptiveToolCall(text);
      expect(result).toBeNull();
    });

    test("handles unknown tool names with generic parsing", () => {
      const text = "Using custom_tool tool with arguments: param1=value1, param2=value2";
      const result = parseDescriptiveToolCall(text);
      expect(result?.name).toBe("custom_tool");
      expect(result?.arguments.param1).toBe("value1");
    });
  });
});

describe("FM_MODEL_CONFIGS", () => {
  test("defines apple-foundation config", () => {
    expect(FM_MODEL_CONFIGS["apple-foundation"]).toBeDefined();
    expect(FM_MODEL_CONFIGS["apple-foundation"].maxContextChars).toBe(1100);
  });

  test("defines default config", () => {
    expect(FM_MODEL_CONFIGS["default"]).toBeDefined();
    expect(FM_MODEL_CONFIGS["default"].maxContextChars).toBe(1100);
  });

  test("FM_MAX_CONTEXT_CHARS_DEFAULT is 1100", () => {
    expect(FM_MAX_CONTEXT_CHARS_DEFAULT).toBe(1100);
  });
});

describe("getFMContextLimit", () => {
  test("returns model-specific limit when available", () => {
    expect(getFMContextLimit("apple-foundation")).toBe(1100);
  });

  test("returns default limit for unknown models", () => {
    expect(getFMContextLimit("unknown-model")).toBe(FM_MAX_CONTEXT_CHARS_DEFAULT);
  });

  test("returns default limit when no model specified", () => {
    expect(getFMContextLimit()).toBe(FM_MAX_CONTEXT_CHARS_DEFAULT);
    expect(getFMContextLimit(undefined)).toBe(FM_MAX_CONTEXT_CHARS_DEFAULT);
  });
});

describe("createFMToolParseError", () => {
  test("creates error with correct structure", () => {
    const error = createFMToolParseError(
      "some raw text that is very long".repeat(10),
      "no_valid_format",
      "additional details"
    );

    expect(error.type).toBe("FM_TOOL_PARSE_ERROR");
    expect(error.reason).toBe("no_valid_format");
    expect(error.rawSnippet.length).toBeLessThanOrEqual(200);
    expect(error.details).toBe("additional details");
    expect(error.timestamp).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  });

  test("truncates rawSnippet to 200 chars", () => {
    const longText = "x".repeat(500);
    const error = createFMToolParseError(longText, "json_parse_error");
    expect(error.rawSnippet.length).toBe(200);
  });
});

describe("KNOWN_FM_TOOLS", () => {
  test("includes essential tools", () => {
    expect(KNOWN_FM_TOOLS).toContain("write_file");
    expect(KNOWN_FM_TOOLS).toContain("read_file");
    expect(KNOWN_FM_TOOLS).toContain("run_command");
    expect(KNOWN_FM_TOOLS).toContain("edit_file");
  });

  test("includes search tools", () => {
    expect(KNOWN_FM_TOOLS).toContain("grep");
    expect(KNOWN_FM_TOOLS).toContain("glob");
    expect(KNOWN_FM_TOOLS).toContain("find");
  });
});
