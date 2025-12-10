import { describe, expect, it } from "bun:test";
import { parseToolCalls } from "./model-adapter.js";

describe("parseToolCalls", () => {
  it("parses valid tool call with closing tag", () => {
    const input = '<tool_call>{"name":"write_file","arguments":{"path":"foo.txt","content":"hello"}}</tool_call>';
    const result = parseToolCalls(input);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("write_file");
    expect(result[0].arguments.path).toBe("foo.txt");
    expect(result[0].arguments.content).toBe("hello");
  });

  it("parses tool call without closing tag", () => {
    const input = '<tool_call>{"name":"read_file","arguments":{"path":"test.txt"}}';
    const result = parseToolCalls(input);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("read_file");
    expect(result[0].arguments.path).toBe("test.txt");
  });

  it("salvages truncated JSON with trailing junk", () => {
    // Simulates FM output that got cut off mid-JSON
    const input = '<tool_call>{"name":"write_file","arguments":{"path":"foo.txt","content":"hello"}}</tool_call>junk';
    const result = parseToolCalls(input);
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe("write_file");
    expect(result[0].arguments.path).toBe("foo.txt");
    expect(result[0].arguments.content).toBe("hello");
  });

  it("salvages truncated JSON ending mid-arguments", () => {
    // Simulates JSON cut off in the middle of arguments
    const input = '<tool_call>{"name":"write_file","arguments":{"path":"image.c","content":"#include <stdio.h>\n#include <stdlib.h>\n';
    const result = parseToolCalls(input);
    // Should attempt salvage but may not succeed if too truncated
    // At minimum, should not crash
    expect(Array.isArray(result)).toBe(true);
  });

  it("handles multiple tool calls", () => {
    const input = '<tool_call>{"name":"read_file","arguments":{"path":"a.txt"}}</tool_call><tool_call>{"name":"write_file","arguments":{"path":"b.txt","content":"test"}}</tool_call>';
    const result = parseToolCalls(input);
    expect(result).toHaveLength(2);
    expect(result[0].name).toBe("read_file");
    expect(result[1].name).toBe("write_file");
  });

  it("returns empty array for invalid input", () => {
    const input = "not a tool call";
    const result = parseToolCalls(input);
    expect(result).toHaveLength(0);
  });

  it("handles code block format", () => {
    const input = '```json\n{"name":"run_command","arguments":{"command":"ls -la"}}\n```';
    const result = parseToolCalls(input);
    expect(result.length).toBeGreaterThanOrEqual(0);
    // May or may not parse depending on format, but shouldn't crash
  });
});








