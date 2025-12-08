# 0050 FM Terminal-Bench Implementation Guide

**For:** Coding agent implementation  
**Based on:** 0045-fm-tb-fix-plan.md + reviewer feedback  
**Goal:** Fix 5 systemic issues causing FM TB2 failure

---

## Overview

Implement these changes in order. Run `bun run typecheck` and `bun test` after each section.

**Files to create:**
- `src/fm/hints.ts`
- `src/fm/step-summary.ts`

**Files to modify:**
- `src/fm/worker.ts`
- `src/fm/orchestrator.ts`
- `src/bench/model-adapter.ts`

---

## Section 1: Suite-Aware Hints

### 1.1 Create `src/fm/hints.ts`

```typescript
/**
 * Suite-aware hint system for FM micro-tasks.
 * Hints are disabled for TB2 to avoid fm-mini heuristics polluting real benchmarks.
 */

export type SuiteMode = "fm-mini" | "tb2" | "unknown";

/**
 * Determine suite mode from suite path or name.
 */
export function getSuiteMode(suitePath: string | undefined): SuiteMode {
  if (!suitePath) return "unknown";
  
  const lower = suitePath.toLowerCase();
  if (lower.includes("terminal-bench-mini") || lower.includes("fm-mini")) {
    return "fm-mini";
  }
  if (lower.includes("terminal-bench-2") || lower.includes("tb2")) {
    return "tb2";
  }
  return "unknown";
}

/**
 * Build a hint for the current task context.
 * Returns undefined if no hint should be shown.
 * 
 * IMPORTANT: TB2 gets NO hints by default. Only fm-mini uses hints.
 */
export function buildHint(
  taskDescription: string,
  previousActions: string[],
  mode: SuiteMode
): string | undefined {
  // TB2: No hints until we add task-specific ones
  if (mode === "tb2") {
    return undefined;
  }
  
  // Unknown mode: be conservative, no hints
  if (mode === "unknown") {
    return undefined;
  }
  
  // fm-mini: Keep existing hint logic
  const descLower = taskDescription.toLowerCase();
  const prevJoined = previousActions.join(" ").toLowerCase();
  
  // Hint: read before write (only for fm-mini)
  if (
    (descLower.includes("read") || descLower.includes("copy") || descLower.includes("duplicate")) &&
    !prevJoined.includes("read_file")
  ) {
    return "Hint: This task requires reading a file first. Use read_file before writing.";
  }
  
  // Hint: after reading, write exactly (only for fm-mini)
  if (prevJoined.includes("read_file") && !prevJoined.includes("write_file")) {
    return "Hint: You just read file content. Write it EXACTLY to the target file using write_file.";
  }
  
  // Hint: word count (only for fm-mini)
  if (descLower.includes("count") && descLower.includes("word")) {
    return "Hint: Use shell tools like 'wc -w' for counting words.";
  }
  
  return undefined;
}
```

### 1.2 Update `src/fm/orchestrator.ts`

Add to `OrchestratorOptions` interface:
```typescript
export interface OrchestratorOptions {
  // ... existing fields
  suiteMode?: SuiteMode;
}
```

Import and use in orchestrator:
```typescript
import { buildHint, type SuiteMode } from "./hints.js";

// In the execution loop, when building worker input:
const hint = buildHint(
  task.description,
  history.map(h => h.tool),
  options.suiteMode ?? "unknown"
);
```

### 1.3 Update `src/bench/model-adapter.ts`

Pass suite mode when calling FM:
```typescript
import { getSuiteMode } from "../fm/hints.js";

// In runMicroTaskPlan or wherever FM is invoked:
const suiteMode = getSuiteMode(options.suitePath);

await orchestrator.run(task, {
  ...options,
  suiteMode,
});
```

### 1.4 Tests

Create `src/fm/hints.test.ts`:
```typescript
import { describe, expect, it } from "bun:test";
import { buildHint, getSuiteMode } from "./hints.js";

describe("getSuiteMode", () => {
  it("detects fm-mini from path", () => {
    expect(getSuiteMode("tasks/terminal-bench-mini.json")).toBe("fm-mini");
  });
  
  it("detects tb2 from path", () => {
    expect(getSuiteMode("tasks/terminal-bench-2.json")).toBe("tb2");
  });
  
  it("returns unknown for other paths", () => {
    expect(getSuiteMode("tasks/custom-suite.json")).toBe("unknown");
  });
});

describe("buildHint", () => {
  it("returns undefined for tb2 mode", () => {
    const hint = buildHint("read file foo.txt and copy to bar.txt", [], "tb2");
    expect(hint).toBeUndefined();
  });
  
  it("returns hint for fm-mini read task", () => {
    const hint = buildHint("read file foo.txt and copy to bar.txt", [], "fm-mini");
    expect(hint).toContain("read_file");
  });
  
  it("returns undefined for unknown mode", () => {
    const hint = buildHint("read file foo.txt", [], "unknown");
    expect(hint).toBeUndefined();
  });
});
```

---

## Section 2: StepSummary (Context Truncation)

### 2.1 Create `src/fm/step-summary.ts`

```typescript
/**
 * StepSummary: Compact representation of tool execution for context management.
 * Prevents context overflow by summarizing tool outputs intelligently.
 */

export interface StepSummary {
  step: number;
  tool: string;
  success: boolean;
  message: string; // Always <= MAX_MESSAGE_CHARS
}

const MAX_MESSAGE_CHARS = 100;
const MAX_SUMMARIES = 3;

/**
 * Create a compact summary of a tool execution result.
 * Tool-aware: produces readable summaries instead of truncated blobs.
 */
export function summarizeToolResult(
  step: number,
  tool: string,
  success: boolean,
  rawOutput: string,
  args?: Record<string, unknown>
): StepSummary {
  let message: string;
  
  switch (tool) {
    case "read_file": {
      const path = args?.path ?? "file";
      const lines = rawOutput.split("\n").length;
      const chars = rawOutput.length;
      message = success 
        ? `Read ${path} (${lines} lines, ${chars} chars)`
        : `Failed to read ${path}: ${rawOutput.slice(0, 50)}`;
      break;
    }
    
    case "write_file": {
      const path = args?.path ?? "file";
      const bytes = typeof args?.content === "string" ? args.content.length : 0;
      message = success
        ? `Wrote ${bytes} bytes to ${path}`
        : `Failed to write ${path}: ${rawOutput.slice(0, 50)}`;
      break;
    }
    
    case "run_command": {
      const cmd = String(args?.command ?? "").slice(0, 40);
      const cmdDisplay = cmd.length < 40 ? cmd : cmd + "...";
      if (success) {
        const hasOutput = rawOutput.trim().length > 0;
        message = hasOutput 
          ? `Ran: ${cmdDisplay} (ok, output)`
          : `Ran: ${cmdDisplay} (ok, no output)`;
      } else {
        const errorSnippet = rawOutput.slice(0, 30).replace(/\n/g, " ");
        message = `Ran: ${cmdDisplay} (failed: ${errorSnippet})`;
      }
      break;
    }
    
    case "edit_file": {
      const path = args?.path ?? "file";
      message = success
        ? `Edited ${path}`
        : `Failed to edit ${path}: ${rawOutput.slice(0, 50)}`;
      break;
    }
    
    case "task_complete": {
      message = "Signaled task complete";
      break;
    }
    
    case "verification": {
      message = success
        ? "Verification passed"
        : "Verification failed: output does not meet spec";
      break;
    }
    
    default: {
      // Fallback: truncate raw output
      message = rawOutput.slice(0, MAX_MESSAGE_CHARS);
      if (rawOutput.length > MAX_MESSAGE_CHARS) {
        message = message.slice(0, MAX_MESSAGE_CHARS - 3) + "...";
      }
    }
  }
  
  // Final safety cap
  if (message.length > MAX_MESSAGE_CHARS) {
    message = message.slice(0, MAX_MESSAGE_CHARS - 3) + "...";
  }
  
  return { step, tool, success, message };
}

/**
 * Build the Previous field from step history.
 * Keeps only the last MAX_SUMMARIES entries.
 */
export function buildPreviousField(history: StepSummary[]): string {
  if (history.length === 0) return "none";
  
  const recent = history.slice(-MAX_SUMMARIES);
  return recent
    .map(h => `Step ${h.step} (${h.tool}): ${h.message}`)
    .join("; ");
}

/**
 * Constants for external use
 */
export const STEP_SUMMARY_LIMITS = {
  maxMessageChars: MAX_MESSAGE_CHARS,
  maxSummaries: MAX_SUMMARIES,
} as const;
```

### 2.2 Update `src/fm/orchestrator.ts`

Replace raw output tracking with StepSummary:

```typescript
import { 
  summarizeToolResult, 
  buildPreviousField, 
  type StepSummary 
} from "./step-summary.js";

// In the orchestrator class/function:
const history: StepSummary[] = [];

// After each tool execution:
const summary = summarizeToolResult(
  currentStep,
  result.tool,
  result.success,
  result.output,
  result.args // Pass args for tool-aware summaries
);
history.push(summary);

// When building worker prompt:
const previous = buildPreviousField(history);
// Use `previous` in the WorkerPromptInput
```

### 2.3 Update `src/fm/worker.ts`

Ensure WorkerPromptInput accepts the previous field and uses it:

```typescript
export interface WorkerPromptInput {
  // ... existing fields
  previous: string; // Now always comes from buildPreviousField()
}
```

### 2.4 Tests

Create `src/fm/step-summary.test.ts`:
```typescript
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
```

---

## Section 3: Verification-Gated Completion

### 3.1 Update `src/fm/orchestrator.ts`

Add verification callback to options:

```typescript
export interface OrchestratorOptions {
  // ... existing fields
  suiteMode?: SuiteMode;
  verifyTask?: () => Promise<boolean>;
  maxRetryAfterFailedVerify?: number; // Default: 2
}
```

Update the main execution loop:

```typescript
// Track verification retry count
let verifyRetryCount = 0;
const maxVerifyRetries = options.maxRetryAfterFailedVerify ?? 2;

// In the loop, when FM signals completion:
if (toolCall.name === "task_complete" || repeatedSameAction >= 3) {
  // If we have a verifier, use it
  if (options.verifyTask) {
    const passed = await options.verifyTask();
    
    if (passed) {
      return { 
        success: true, 
        message: "Task completed and verified",
        turns: currentStep 
      };
    }
    
    // Verification failed
    verifyRetryCount++;
    
    if (verifyRetryCount >= maxVerifyRetries) {
      return { 
        success: false, 
        message: `Verification failed after ${verifyRetryCount} attempts`,
        turns: currentStep 
      };
    }
    
    // Add feedback to history and continue
    history.push(summarizeToolResult(
      currentStep,
      "verification",
      false,
      "Verification failed: your output does not meet the spec. Fix and try again.",
      {}
    ));
    
    // Reset repeat counter since we're giving FM another chance
    repeatedSameAction = 0;
    continue; // Don't exit the loop
  }
  
  // No verifier: trust the completion signal (backward compat)
  return { 
    success: true, 
    message: "FM signaled completion",
    turns: currentStep 
  };
}
```

### 3.2 Update `src/bench/model-adapter.ts`

Wire verification from TB runner:

```typescript
// Import verification utilities
import { runVerificationScript } from "./verification.js"; // or wherever this lives

// In the function that runs FM for a task:
async function runFMTask(task: TBTask, workspace: string, options: RunOptions) {
  const suiteMode = getSuiteMode(options.suitePath);
  
  // Create verification callback
  const verifyTask = async (): Promise<boolean> => {
    if (!task.verify) return true; // No verification script = pass
    
    try {
      const result = await runVerificationScript(task.verify, workspace);
      return result.exitCode === 0;
    } catch {
      return false;
    }
  };
  
  return orchestrator.run(task, {
    ...options,
    suiteMode,
    verifyTask,
    maxRetryAfterFailedVerify: 2,
  });
}
```

### 3.3 Tests

Add to orchestrator tests:
```typescript
describe("verification-gated completion", () => {
  it("continues after failed verification", async () => {
    let verifyCallCount = 0;
    const verifyTask = async () => {
      verifyCallCount++;
      return verifyCallCount >= 2; // Pass on second try
    };
    
    // Mock FM that always calls task_complete
    const mockFM = createMockFM(() => ({ name: "task_complete", arguments: {} }));
    
    const result = await orchestrator.run(task, {
      client: mockFM,
      verifyTask,
      maxRetryAfterFailedVerify: 3,
    });
    
    expect(verifyCallCount).toBe(2);
    expect(result.success).toBe(true);
  });
  
  it("fails after max verify retries", async () => {
    const verifyTask = async () => false; // Always fail
    
    const mockFM = createMockFM(() => ({ name: "task_complete", arguments: {} }));
    
    const result = await orchestrator.run(task, {
      client: mockFM,
      verifyTask,
      maxRetryAfterFailedVerify: 2,
    });
    
    expect(result.success).toBe(false);
    expect(result.message).toContain("Verification failed");
  });
});
```

---

## Section 4: Path Normalization

### 4.1 Update `src/fm/worker.ts` System Prompt

Add to the system prompt template:

```typescript
const SYSTEM_PROMPT = `You are a coding assistant. Respond ONLY with a tool call...

PATH RULES:
- Your workspace is the current directory (.)
- When the task mentions "/app/foo", use "foo" or "./foo" (relative path)
- Never use absolute /app/ paths in commands or file operations
- Example: "/app/output.txt" → "output.txt" or "./output.txt"

Available tools:
...`;
```

### 4.2 Add Command Normalization in Tool Executor

In the file that executes `run_command` (likely `src/fm/tools.ts` or similar):

```typescript
/**
 * Normalize /app/ paths in shell commands to relative paths.
 */
function normalizeCommand(command: string): string {
  let cmd = command;
  
  // Replace /app/ with ./
  cmd = cmd.replace(/\/app\//g, "./");
  
  // Strip "cd /app && " prefix entirely
  cmd = cmd.replace(/^cd\s+\/app\s*&&\s*/i, "");
  
  // Strip standalone "cd /app;" 
  cmd = cmd.replace(/^cd\s+\/app\s*;\s*/i, "");
  
  return cmd;
}

// In run_command executor:
async function executeRunCommand(args: { command: string }, workspace: string) {
  const normalizedCommand = normalizeCommand(args.command);
  // ... execute normalizedCommand in workspace
}
```

### 4.3 Tests

```typescript
describe("normalizeCommand", () => {
  it("replaces /app/ with ./", () => {
    expect(normalizeCommand("touch /app/file.txt")).toBe("touch ./file.txt");
  });
  
  it("strips cd /app && prefix", () => {
    expect(normalizeCommand("cd /app && ./configure")).toBe("./configure");
  });
  
  it("strips cd /app; prefix", () => {
    expect(normalizeCommand("cd /app; make")).toBe("make");
  });
  
  it("handles multiple /app/ occurrences", () => {
    expect(normalizeCommand("cp /app/a.txt /app/b.txt")).toBe("cp ./a.txt ./b.txt");
  });
  
  it("leaves relative paths unchanged", () => {
    expect(normalizeCommand("touch ./file.txt")).toBe("touch ./file.txt");
  });
});
```

---

## Section 5: Skills Presentation

### 5.1 Update `src/fm/worker.ts` Skills Section

Change how skills are formatted in the prompt:

```typescript
// Current format (confusing - looks like tools):
// Relevant Skills:
// - Setup Bun Project: Initialize a Bun project...

// New format (clearly not callable):
function formatSkillsSection(skills: Skill[] | undefined): string {
  if (!skills || skills.length === 0) return "";
  
  const approaches = skills.map(s => {
    // Use description only, without the skill name
    const desc = s.description.slice(0, 80);
    return `  • ${desc}`;
  }).join("\n");
  
  return `
Example approaches (for reference only, NOT callable tools):
${approaches}
`;
}
```

### 5.2 Add Explicit Tool List in System Prompt

Make it crystal clear what the only valid tools are:

```typescript
const SYSTEM_PROMPT = `...

IMPORTANT: The ONLY tools you may call are:
- write_file
- read_file  
- run_command
- edit_file
- task_complete

Any other name in the tool call will fail. Skills/approaches listed above are for inspiration only.

...`;
```

### 5.3 Suite-Aware Skills Toggle

In `src/bench/model-adapter.ts`:

```typescript
// For TB2, disable skills initially until we stabilize other fixes
const useSkills = suiteMode === "fm-mini"; // TB2 gets false

await orchestrator.run(task, {
  ...options,
  suiteMode,
  useSkills, // Pass through to worker
});
```

---

## Section 6: Validation Checklist

After implementing all sections, run these checks:

### 6.1 Type Check
```bash
bun run typecheck
```
Must pass with zero errors.

### 6.2 Unit Tests
```bash
bun test src/fm/
```
All new tests must pass.

### 6.3 Integration Test: fm-mini
```bash
bun run tbench -- --suite tasks/terminal-bench-mini.json --model fm
```
Should maintain current pass rate (hints still work for fm-mini).

### 6.4 Integration Test: TB2 Sample
```bash
# Run just 3 tasks to verify fixes
bun run tbench -- --suite tasks/terminal-bench-2.json --model fm --tasks path-tracing,regex-log,dna-assembly
```

Check logs for:
- [ ] No hint appears for TB2 tasks
- [ ] Previous field is <300 chars
- [ ] FM gets "verification failed" feedback when task_complete is premature
- [ ] No `/app/` path errors in run_command

---

## File Summary

### New Files
| File | Purpose |
|------|---------|
| `src/fm/hints.ts` | Suite-aware hint system |
| `src/fm/hints.test.ts` | Hint tests |
| `src/fm/step-summary.ts` | StepSummary type and builders |
| `src/fm/step-summary.test.ts` | StepSummary tests |

### Modified Files
| File | Changes |
|------|---------|
| `src/fm/worker.ts` | System prompt updates, skills formatting, use buildHint() |
| `src/fm/orchestrator.ts` | StepSummary integration, verification-gated completion, suiteMode |
| `src/bench/model-adapter.ts` | Wire suiteMode, verifyTask, useSkills toggle |
| Tool executor (varies) | normalizeCommand() for /app/ paths |

---

## Notes for Implementer

1. **Don't break fm-mini**: All existing fm-mini behavior should continue to work. The changes are additive/gated.

2. **Test incrementally**: Implement one section, run typecheck + tests, then move to next.

3. **Check existing code first**: Read the current `worker.ts` and `orchestrator.ts` to understand exact structure before editing.

4. **StepSummary args**: Make sure the tool executor passes `args` to `summarizeToolResult` so tool-aware summaries work.

5. **Verification script path**: The TB task schema has a `verify` field - check how it's currently used and wire `verifyTask` appropriately.
