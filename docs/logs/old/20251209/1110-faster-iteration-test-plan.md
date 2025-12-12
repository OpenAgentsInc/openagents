# Plan: Fix Context Loss Bug & Build Test Suite

## Problem Summary

Two issues blocking faster iteration:
1. **Context loss bug**: FM loses regex when advancing subtasks (65.2% → 0%)
2. **No unit tests**: Bugs discovered during long runs, not caught early

## Bugs Discovered This Session

| Bug | Root Cause | Impact |
|-----|------------|--------|
| **Context loss on subtask transition** | `buildFMContext()` doesn't read workspace files | 65.2% → 0% regression |
| **Parser takes read_file over write_file** | First tool call selected, not best | FM wasted turns reading |
| **JSON `\b` becomes backspace** | `\b` in JSON = 0x08, not regex boundary | Regex broken silently |
| **FM outputs malformed JSON** | Missing closing braces in tool calls | Parse failures |

---

## Part 1: Fix Context Loss Bug

### Root Cause Analysis

**File:** `src/hillclimber/map-orchestrator.ts`

When FM advances from subtask 1 → subtask 2:
- `buildFMContext()` builds prompt with subtask goal
- Goal says "improve regex with boundary assertions"
- **BUT** actual regex.txt content is NOT included
- FM starts from scratch, loses IPv4 lookahead

**The fix:** Read workspace files and include in FM context.

### Implementation

**Step 1: Add file contents to FMContext interface** (`map-orchestrator.ts:149`)
```typescript
interface FMContext {
  // ... existing fields ...
  fileContents?: Record<string, string>;  // NEW: workspace file contents
}
```

**Step 2: Read files in buildFMContext** (`map-orchestrator.ts:165`)
```typescript
function buildFMContext(
  task: TerminalBenchTask,
  decomposition: TaskDecomposition,
  state: ExecutionState,
  workspacePath: string,  // NEW parameter
): FMContext {
  // Read modified files to pass context forward
  const fileContents: Record<string, string> = {};
  for (const filePath of state.modifiedFiles) {
    const fullPath = path.join(workspacePath, filePath.replace(/^\/app\//, ""));
    if (fs.existsSync(fullPath)) {
      fileContents[filePath] = fs.readFileSync(fullPath, "utf-8");
    }
  }

  return {
    // ... existing fields ...
    fileContents,
  };
}
```

**Step 3: Include in prompt** (`map-orchestrator.ts:250`)
```typescript
// In formatFMPrompt(), add section for current file contents
if (context.fileContents && Object.keys(context.fileContents).length > 0) {
  lines.push(`## Current File Contents`);
  for (const [path, content] of Object.entries(context.fileContents)) {
    lines.push(`### ${path}`);
    lines.push("```");
    lines.push(content.trim());
    lines.push("```");
  }
}
```

---

## Part 2: Build Unit Test Suite

### Test File: `src/hillclimber/map-orchestrator.test.ts`

Create comprehensive unit tests for all discovered bugs:

```typescript
import { describe, test, expect } from "bun:test";

describe("MAP Orchestrator Unit Tests", () => {

  // ===== CONTEXT PRESERVATION =====
  describe("buildFMContext", () => {
    test("includes modified file contents in context", () => {
      // Setup: state.modifiedFiles = ["/app/regex.txt"]
      // Setup: workspace has regex.txt with "(?=.*\\d...)..."
      // Assert: context.fileContents["/app/regex.txt"] contains regex
    });

    test("passes file contents across subtask boundary", () => {
      // Setup: advance from subtask 0 to subtask 1
      // Assert: subtask 1 context includes files from subtask 0
    });
  });

  // ===== TOOL SELECTION PRIORITY =====
  describe("parseToolCalls priority", () => {
    test("prefers write_file over read_file when both present", () => {
      const response = `<tool_call>{"name":"read_file","arguments":{"path":"/app/regex.txt"}}</tool_call>
<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"test"}}</tool_call>`;
      const result = parseAndSelectTool(response);
      expect(result.name).toBe("write_file");
    });

    test("follows priority order: write > edit > verify > run > read", () => {
      // Test various combinations
    });
  });

  // ===== JSON ESCAPING =====
  describe("JSON escaping in regex", () => {
    test("preserves \\d as digit pattern not 0x0d", () => {
      const json = '{"content":"\\\\d{4}-\\\\d{2}"}';
      const parsed = JSON.parse(json);
      expect(parsed.content).toBe("\\d{4}-\\d{2}");
      expect(parsed.content.charCodeAt(0)).not.toBe(0x0d);
    });

    test("preserves \\b as word boundary not backspace", () => {
      const json = '{"content":"\\\\btest\\\\b"}';
      const parsed = JSON.parse(json);
      expect(parsed.content).toBe("\\btest\\b");
      expect(parsed.content.charCodeAt(0)).not.toBe(0x08);
    });

    test("fixInvalidEscapeSequences handles unescaped backslashes", () => {
      const broken = '{"content":"\\d{4}"}'; // FM often outputs this
      const fixed = fixInvalidEscapeSequences(broken);
      expect(() => JSON.parse(fixed)).not.toThrow();
    });
  });

  // ===== MALFORMED JSON RECOVERY =====
  describe("JSON parsing recovery", () => {
    test("handles missing closing brace", () => {
      const truncated = '<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"test"}</tool_call>';
      const result = parseToolCalls(truncated);
      expect(result).toHaveLength(1);
      expect(result[0].name).toBe("write_file");
    });

    test("handles multiple tool calls with missing tags", () => {
      const messy = `{"name":"write_file","arguments":{"path":"/app/x","content":"y"}}
{"name":"verify_progress","arguments":{}}`;
      const result = parseToolCalls(messy);
      expect(result.length).toBeGreaterThanOrEqual(1);
    });
  });

  // ===== SUBTASK TRANSITION =====
  describe("subtask advancement", () => {
    test("preserves bestProgress when advancing", () => {
      // Simulate: subtask 1 at 65%, advance to subtask 2
      // Assert: bestProgress still 65%, not reset
    });

    test("does not regress when subtask 2 produces worse result", () => {
      // Simulate: subtask 1 at 65%, subtask 2 produces 0%
      // Assert: bestProgress stays 65%
    });
  });
});
```

### Extend Existing Test: `src/bench/parse-tool-calls.test.ts`

Add edge cases:

```typescript
describe("parseToolCalls edge cases", () => {
  test("handles regex with backslashes", () => {
    const input = '<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"(?=.*\\\\d{1,3})"}}</tool_call>';
    const result = parseToolCalls(input);
    expect(result[0].arguments.content).toBe("(?=.*\\d{1,3})");
  });

  test("handles lookahead regex pattern", () => {
    const regex = "(?=.*\\d{1,3}\\.\\d{1,3}).*\\d{4}-\\d{2}-\\d{2}";
    const input = `<tool_call>{"name":"write_file","arguments":{"path":"/app/regex.txt","content":"${regex.replace(/\\/g, "\\\\")}"}}</tool_call>`;
    const result = parseToolCalls(input);
    expect(result[0].arguments.content).toBe(regex);
  });
});
```

---

## Part 3: Quick Validation Script

### File: `scripts/validate-map.ts`

Ultra-fast validation that runs unit tests + mini integration test:

```bash
bun scripts/validate-map.ts
# Runs in <30 seconds:
# 1. Unit tests (5s)
# 2. Mock FM single-turn test (10s)
# 3. Basic tool parsing (5s)
```

---

## Execution Order

```
Step 1: Fix context loss bug (10 min)
├── Add fileContents to FMContext interface
├── Read files in buildFMContext
└── Include in formatFMPrompt

Step 2: Build unit tests (15 min)
├── Create map-orchestrator.test.ts
├── Add context preservation tests
├── Add tool selection tests
├── Add JSON escaping tests
└── Add subtask transition tests

Step 3: Quick validation (5 min)
├── Create validate-map.ts script
└── Verify all tests pass

Step 4: Run quick integration test (5 min)
└── 3-turn test to confirm fix works
```

**Total: ~35 minutes**

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/hillclimber/map-orchestrator.ts` | Add fileContents to context, read workspace files |
| `src/hillclimber/map-orchestrator.test.ts` | **NEW** - Unit tests for all bugs |
| `src/bench/parse-tool-calls.test.ts` | Add regex escaping edge cases |
| `scripts/validate-map.ts` | **NEW** - Quick validation script |

---

## Success Criteria

- [ ] Unit tests catch all 4 discovered bugs
- [ ] `bun test src/hillclimber/` passes in <10 seconds
- [ ] Context loss bug fixed (subtask 2 sees regex from subtask 1)
- [ ] Quick 3-turn test achieves >60% (proves fix works)
- [ ] Future bugs caught in tests, not during 10+ minute runs
