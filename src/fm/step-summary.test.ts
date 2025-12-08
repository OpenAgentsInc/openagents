import { describe, expect, it } from "bun:test";
import {
  summarizeToolResult,
  buildPreviousField,
  STEP_SUMMARY_LIMITS
} from "./step-summary.js";

describe("summarizeToolResult", () => {
  it("summarizes read_file with stats", () => {
    const result = summarizeToolResult(
      1, "read_file", true,
      "line1\nline2\nline3",
      { path: "test.txt" }
    );
    expect(result.message).toContain("test.txt");
    expect(result.message).toContain("3 lines");
  });

  it("summarizes write_file with byte count", () => {
    const result = summarizeToolResult(
      1, "write_file", true,
      "Created test.txt",
      { path: "test.txt", content: "hello world" }
    );
    expect(result.message).toContain("11 bytes");
  });

  it("summarizes run_command with truncated command", () => {
    const longCmd = "gcc -static -o image image.c -lm -Wall -Werror -O2";
    const result = summarizeToolResult(
      1, "run_command", true,
      "",
      { command: longCmd }
    );
    expect(result.message.length).toBeLessThanOrEqual(STEP_SUMMARY_LIMITS.maxMessageChars);
  });

  it("truncates unknown tool output", () => {
    const longOutput = "x".repeat(200);
    const result = summarizeToolResult(1, "unknown_tool", true, longOutput);
    expect(result.message.length).toBeLessThanOrEqual(STEP_SUMMARY_LIMITS.maxMessageChars);
    expect(result.message).toContain("...");
  });
});

describe("buildPreviousField", () => {
  it("returns 'none' for empty history", () => {
    expect(buildPreviousField([])).toBe("none");
  });

  it("keeps only last 3 entries", () => {
    const history = [
      { step: 1, tool: "read_file", success: true, message: "Read a.txt" },
      { step: 2, tool: "read_file", success: true, message: "Read b.txt" },
      { step: 3, tool: "read_file", success: true, message: "Read c.txt" },
      { step: 4, tool: "read_file", success: true, message: "Read d.txt" },
    ];
    const result = buildPreviousField(history);
    expect(result).not.toContain("a.txt");
    expect(result).toContain("b.txt");
    expect(result).toContain("d.txt");
  });
});
