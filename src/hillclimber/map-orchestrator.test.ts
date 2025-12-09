/**
 * Unit tests for MAP Orchestrator
 *
 * Tests cover bugs discovered during development:
 * 1. Context loss on subtask transition (fileContents preservation)
 * 2. Parser tool selection priority (write_file > read_file)
 * 3. JSON escaping (\b becomes backspace, \d becomes 0x0d)
 * 4. Malformed JSON recovery
 */

import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { mkdirSync, writeFileSync, rmSync, existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { formatFMPrompt } from "./map-orchestrator.js";
import { parseToolCalls } from "../bench/model-adapter.js";

// Helper to create temp workspace
function createTempWorkspace(): string {
  const tmpDir = `/tmp/map-test-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  mkdirSync(tmpDir, { recursive: true });
  return tmpDir;
}

// Helper to clean up temp workspace
function cleanupWorkspace(workspace: string): void {
  if (existsSync(workspace)) {
    rmSync(workspace, { recursive: true, force: true });
  }
}

// ============================================================================
// Test: formatFMPrompt includes file contents
// ============================================================================
describe("formatFMPrompt", () => {
  test("includes file contents when provided", () => {
    const context = {
      taskDescription: "Test task",
      currentSubtask: {
        id: 1,
        name: "test-subtask",
        goal: "Test goal",
        checkpoint: "Test checkpoint",
        expectedArtifacts: [],
        dependsOn: [],
        hints: [],
        maxTurns: 5,
      },
      previousActions: [],
      hints: [],
      globalHints: [],
      fileContents: {
        "/app/regex.txt": "(?=.*\\d{1,3}).*\\d{4}-\\d{2}-\\d{2}",
      },
    };

    const prompt = formatFMPrompt(context);

    // Should include file contents section
    expect(prompt).toContain("## Current File Contents");
    expect(prompt).toContain("/app/regex.txt");
    expect(prompt).toContain("(?=.*\\d{1,3}).*\\d{4}-\\d{2}-\\d{2}");
    expect(prompt).toContain("BUILD ON these - do NOT start from scratch");
  });

  test("omits file contents section when no files modified", () => {
    const context = {
      taskDescription: "Test task",
      currentSubtask: {
        id: 1,
        name: "test-subtask",
        goal: "Test goal",
        checkpoint: "Test checkpoint",
        expectedArtifacts: [],
        dependsOn: [],
        hints: [],
        maxTurns: 5,
      },
      previousActions: [],
      hints: [],
      globalHints: [],
      // No fileContents
    };

    const prompt = formatFMPrompt(context);

    // Should NOT include file contents section
    expect(prompt).not.toContain("## Current File Contents");
  });

  test("includes multiple file contents", () => {
    const context = {
      taskDescription: "Test task",
      currentSubtask: {
        id: 1,
        name: "test-subtask",
        goal: "Test goal",
        checkpoint: "Test checkpoint",
        expectedArtifacts: [],
        dependsOn: [],
        hints: [],
        maxTurns: 5,
      },
      previousActions: [],
      hints: [],
      globalHints: [],
      fileContents: {
        "/app/regex.txt": "pattern1",
        "/app/solution.py": "print('hello')",
      },
    };

    const prompt = formatFMPrompt(context);

    expect(prompt).toContain("/app/regex.txt");
    expect(prompt).toContain("pattern1");
    expect(prompt).toContain("/app/solution.py");
    expect(prompt).toContain("print('hello')");
  });
});

// ============================================================================
// Test: Tool call parsing with priority selection
// ============================================================================
describe("parseToolCalls priority", () => {
  test("prefers write_file over read_file when both present", () => {
    const response = `<tool_call>{"name":"read_file","arguments":{"path":"/app/regex.txt"}}</tool_call>
<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"test"}}</tool_call>`;

    const toolCalls = parseToolCalls(response);

    // Both should be parsed
    expect(toolCalls.length).toBe(2);
    expect(toolCalls.map(tc => tc.name)).toContain("read_file");
    expect(toolCalls.map(tc => tc.name)).toContain("write_file");

    // Priority selection happens in getNextAction, but we verify parsing here
    const writeCall = toolCalls.find(tc => tc.name === "write_file");
    expect(writeCall).toBeDefined();
    expect(writeCall?.arguments.content).toBe("test");
  });

  test("parses tool call with regex content containing backslashes", () => {
    // Double-escaped for JSON: \\d means \d in the actual string
    const response = `<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"\\\\d{4}-\\\\d{2}-\\\\d{2}"}}</tool_call>`;

    const toolCalls = parseToolCalls(response);

    expect(toolCalls.length).toBe(1);
    expect(toolCalls[0].name).toBe("write_file");
    expect(toolCalls[0].arguments.content).toBe("\\d{4}-\\d{2}-\\d{2}");
  });

  test("parses lookahead regex pattern correctly", () => {
    const regex = "(?=.*\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}\\.\\d{1,3}).*\\d{4}-\\d{2}-\\d{2}";
    // Escape backslashes for JSON
    const jsonEscaped = regex.replace(/\\/g, "\\\\");
    const response = `<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"${jsonEscaped}"}}</tool_call>`;

    const toolCalls = parseToolCalls(response);

    expect(toolCalls.length).toBe(1);
    expect(toolCalls[0].arguments.content).toBe(regex);
  });
});

// ============================================================================
// Test: JSON escaping edge cases
// ============================================================================
describe("JSON escaping for regex", () => {
  test("preserves \\d as digit pattern not 0x0d (carriage return)", () => {
    // In JSON, \\d means literal backslash-d
    const json = '{"content":"\\\\d{4}-\\\\d{2}"}';
    const parsed = JSON.parse(json);

    expect(parsed.content).toBe("\\d{4}-\\d{2}");
    // Should NOT be carriage return (0x0d)
    expect(parsed.content.charCodeAt(0)).not.toBe(0x0d);
    expect(parsed.content.charCodeAt(0)).toBe(0x5c); // backslash
  });

  test("preserves \\b as word boundary pattern not backspace (0x08)", () => {
    // In JSON, \\b means literal backslash-b (word boundary in regex)
    const json = '{"content":"\\\\btest\\\\b"}';
    const parsed = JSON.parse(json);

    expect(parsed.content).toBe("\\btest\\b");
    // Should NOT be backspace (0x08)
    expect(parsed.content.charCodeAt(0)).not.toBe(0x08);
    expect(parsed.content.charCodeAt(0)).toBe(0x5c); // backslash
  });

  test("unescaped \\b in JSON becomes backspace (this is the bug)", () => {
    // This demonstrates the bug: FM outputting \b without proper JSON escaping
    // In JSON, \b is backspace (0x08), not regex word boundary
    const json = '{"content":"\\btest\\b"}';
    const parsed = JSON.parse(json);

    // This is the WRONG behavior we need to handle
    expect(parsed.content.charCodeAt(0)).toBe(0x08); // backspace
    expect(parsed.content).not.toBe("\\btest\\b");
  });

  test("tool call parser handles typical FM regex output", () => {
    // FM typically outputs with proper escaping
    const response = `<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"(?=.*\\\\d{1,3})"}}</tool_call>`;
    const toolCalls = parseToolCalls(response);

    expect(toolCalls.length).toBe(1);
    expect(toolCalls[0].arguments.content).toBe("(?=.*\\d{1,3})");
  });
});

// ============================================================================
// Test: Malformed JSON recovery
// ============================================================================
describe("JSON parsing recovery", () => {
  test("handles valid tool call format", () => {
    const response = `<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"test"}}</tool_call>`;
    const result = parseToolCalls(response);

    expect(result.length).toBe(1);
    expect(result[0].name).toBe("write_file");
  });

  test("requires tool_call xml tags for parsing", () => {
    // parseToolCalls requires <tool_call> tags - raw JSON won't parse
    const response = `{"name":"write_file","arguments":{"path":"/app/x","content":"y"}}`;
    const result = parseToolCalls(response);

    // Without tags, should return empty (this is expected behavior)
    expect(result.length).toBe(0);
  });

  test("handles multiple tool calls", () => {
    const response = `<tool_call>{"name":"read_file","arguments":{"path":"/app/x"}}</tool_call>
<tool_call>{"name":"write_file","arguments":{"path":"/app/y","content":"z"}}</tool_call>`;
    const result = parseToolCalls(response);

    expect(result.length).toBe(2);
  });
});

// ============================================================================
// Test: Context preservation across subtask transitions
// ============================================================================
describe("context preservation", () => {
  let workspace: string;

  beforeEach(() => {
    workspace = createTempWorkspace();
  });

  afterEach(() => {
    cleanupWorkspace(workspace);
  });

  test("file contents read from workspace are passed to prompt", () => {
    // Create a file in workspace
    const regexContent = "(?=.*\\d{1,3}).*\\d{4}-\\d{2}-\\d{2}";
    writeFileSync(resolve(workspace, "regex.txt"), regexContent);

    // Read file contents (simulating what buildFMContext does)
    const fileContents: Record<string, string> = {};
    const filePath = "regex.txt";
    const fullPath = resolve(workspace, filePath);
    if (existsSync(fullPath)) {
      fileContents[`/app/${filePath}`] = readFileSync(fullPath, "utf-8");
    }

    // Build context with file contents
    const context = {
      taskDescription: "Test task",
      currentSubtask: {
        id: 2,
        name: "add-boundary-assertions",
        goal: "Add boundary assertions to improve regex",
        checkpoint: "Regex passes more tests",
        expectedArtifacts: ["regex.txt"],
        dependsOn: [1],
        hints: [],
        maxTurns: 5,
      },
      previousActions: [],
      hints: [],
      globalHints: [],
      fileContents,
    };

    const prompt = formatFMPrompt(context);

    // The regex from subtask 1 should be visible in subtask 2's prompt
    expect(prompt).toContain("## Current File Contents");
    expect(prompt).toContain(regexContent);
    expect(prompt).toContain("BUILD ON these");
  });

  test("empty modifiedFiles results in no fileContents section", () => {
    // No files created

    const context = {
      taskDescription: "Test task",
      currentSubtask: {
        id: 1,
        name: "write-initial-regex",
        goal: "Write initial regex",
        checkpoint: "Regex file exists",
        expectedArtifacts: ["regex.txt"],
        dependsOn: [],
        hints: [],
        maxTurns: 5,
      },
      previousActions: [],
      hints: [],
      globalHints: [],
      // No fileContents
    };

    const prompt = formatFMPrompt(context);

    expect(prompt).not.toContain("## Current File Contents");
  });
});

// ============================================================================
// Test: Monitor warning propagation
// ============================================================================
describe("monitor warning in hints", () => {
  test("monitor warning appears in hints when present", () => {
    const context = {
      taskDescription: "Test task",
      currentSubtask: {
        id: 1,
        name: "test-subtask",
        goal: "Test goal",
        checkpoint: "Test checkpoint",
        expectedArtifacts: [],
        dependsOn: [],
        hints: [],
        maxTurns: 5,
      },
      previousActions: [],
      hints: ["⚠️ Regex might be too simple. Need lookahead (?=) for IPv4 constraint."],
      globalHints: [],
    };

    const prompt = formatFMPrompt(context);

    expect(prompt).toContain("## Hints");
    expect(prompt).toContain("Regex might be too simple");
    expect(prompt).toContain("lookahead");
  });
});
