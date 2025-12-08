# 0127 FM TB Round 2 Fixes

**For:** Coding agent implementation  
**Based on:** 0126-chatgpt-response.md + tb2-test-20251208-011347 results  
**Goal:** Fix remaining scaffolding issues after initial StepSummary/hints implementation

---

## What's Working Now

- Hints correctly disabled for TB2 (no "read first" nonsense)
- PATH rules & `/app` normalization are honored
- StepSummary is giving compact `Previous:` entries
- Skills are disabled for TB2
- Verification gating works for `task_complete` and "same tool+args repeated"

## What's Still Broken

1. **Context window exceeded** - Full TB2 task descriptions (~3.5k chars) still blow FM's tiny context
2. **"3 consecutive failures after success" bypasses verification** - This heuristic returns "likely complete" without calling `verifyTask`
3. **parseToolCalls too brittle** - Long `write_file` content (C code, regexes) fails JSON parsing

---

## Section 1: Hard-Cap Task Description

### 1.1 Problem

Even with StepSummary, injecting the full TB2 task description on every turn causes:
```
Error: Foundation Models request failed: Exceeded model context window size
```

Seen in: dna-assembly, path-tracing

### 1.2 Solution

Truncate the task description before injecting into the prompt.

**Where:** `src/fm/worker.ts` (or wherever the FM prompt is built)

**Implementation:**

```typescript
const MAX_TASK_CHARS = 600; // Conservative limit for FM's tiny context

/**
 * Truncate task description to fit in FM's context window.
 * The full description is too verbose for FM; it just needs the gist.
 */
function truncateTaskDescription(description: string): string {
  if (description.length <= MAX_TASK_CHARS) {
    return description;
  }
  return description.slice(0, MAX_TASK_CHARS) + "\n...[truncated]";
}

// In the prompt builder:
const taskSnippet = truncateTaskDescription(input.taskDescription ?? "");

// Use taskSnippet instead of the full description in the prompt
```

### 1.3 Where to Apply

Find where the prompt is built (likely `buildWorkerPrompt()` or similar) and replace:
```typescript
// Before:
Original Task: ${input.taskDescription}

// After:
Original Task: ${truncateTaskDescription(input.taskDescription)}
```

### 1.4 Optional: Log Prompt Length

Add logging to verify total prompt size stays under ~2.5-3.0k chars:
```typescript
const prompt = buildPrompt(...);
console.log(`[FM] Prompt length: ${prompt.length} chars`);
```

### 1.5 Test

Run dna-assembly:
```bash
bun run tbench -- --suite tasks/terminal-bench-2.json --model fm --tasks dna-assembly
```

Confirm NO `Exceeded model context window size` errors in logs.

---

## Section 2: Unify All Completion Paths Through Verification

### 2.1 Problem

The "3 consecutive failures after success" heuristic bypasses `verifyTask`:
```
[Orchestrator] 3 consecutive failures after success - task likely complete
```
No verification runs. This path is used in dna-assembly and regex-log.

### 2.2 Solution

Create a single `finalizeIfDone()` function that ALL completion paths use.

**Where:** `src/fm/orchestrator.ts`

**Implementation:**

```typescript
type CompletionReason = 
  | "task_complete" 
  | "repeat_same_action" 
  | "repeat_failures";

/**
 * Check if task is done. If verifier exists, run it.
 * Returns result if done, undefined if should continue.
 */
async function finalizeIfDone(
  reason: CompletionReason,
  options: OrchestratorOptions,
  state: {
    step: number;
    history: StepSummary[];
    verifyRetryCount: number;
    maxVerifyRetries: number;
  }
): Promise<OrchestratorResult | undefined> {
  // If no verifier, trust the signal (backward compat for fm-mini)
  if (!options.verifyTask) {
    return {
      success: true,
      message: `FM signaled completion (${reason})`,
      turns: state.step,
    };
  }

  // Run verification
  const passed = await options.verifyTask();
  
  if (passed) {
    return {
      success: true,
      message: `Task completed and verified (${reason})`,
      turns: state.step,
    };
  }

  // Verification failed
  state.verifyRetryCount++;
  
  if (state.verifyRetryCount >= state.maxVerifyRetries) {
    return {
      success: false,
      message: `Verification failed after ${state.verifyRetryCount} attempts`,
      turns: state.step,
    };
  }

  // Add feedback to history and continue
  state.history.push(summarizeToolResult(
    state.step,
    "verification",
    false,
    "Verification failed: output does not meet spec. Fix and try again.",
    {}
  ));

  // Return undefined = not done, keep looping
  return undefined;
}
```

### 2.3 Update All Completion Paths

Replace current completion logic with calls to `finalizeIfDone()`:

```typescript
// Path 1: FM calls task_complete
if (toolCall.name === "task_complete") {
  const result = await finalizeIfDone("task_complete", options, state);
  if (result) return result;
  // Reset counters and continue loop
  resetRepeatCounters();
  continue;
}

// Path 2: Same action repeated too many times
if (repeatedSameAction >= 3) {
  const result = await finalizeIfDone("repeat_same_action", options, state);
  if (result) return result;
  resetRepeatCounters();
  continue;
}

// Path 3: 3 consecutive failures after a success (NEW - was unverified before)
if (consecutiveFailures >= 3 && hadPriorSuccess) {
  const result = await finalizeIfDone("repeat_failures", options, state);
  if (result) return result;
  resetRepeatCounters();
  continue;
}
```

### 2.4 Important State Management

Make sure to:
1. Track `consecutiveFailures` counter (reset on success)
2. Track `hadPriorSuccess` boolean (set true after first successful tool execution)
3. Reset all counters after failed verification to give FM fresh chances

```typescript
// After successful tool execution:
consecutiveFailures = 0;
hadPriorSuccess = true;

// After failed tool execution (parse_error, context_error, tool failure):
consecutiveFailures++;

// After failed verification:
consecutiveFailures = 0;
repeatedSameAction = 0;
```

### 2.5 Test

Run regex-log:
```bash
bun run tbench -- --suite tasks/terminal-bench-2.json --model fm --tasks regex-log
```

Confirm you see `[Orchestrator] Verification...` logs even when the "3 consecutive failures" path triggers.

---

## Section 3: Make parseToolCalls More Forgiving

### 3.1 Problem

FM emits tool calls with long content that breaks JSON parsing:
```
No tool call parsed, raw: <tool_call>{"name":"write_file","arguments":{"path":"image.c","content":"#include <stdio.h>...
```

The JSON is often truncated or has unescaped characters.

### 3.2 Solution (Minimal)

Improve the regex to capture JSON more reliably.

**Where:** Wherever `parseToolCalls` is defined (likely `src/fm/worker.ts` or `src/bench/model-adapter.ts`)

**Implementation:**

```typescript
/**
 * Parse tool calls from FM output.
 * More forgiving: handles trailing junk after JSON.
 */
function parseToolCalls(output: string): ToolCall | null {
  // Look for <tool_call> followed by JSON object
  // Capture everything up to the last } before </tool_call> or end
  const patterns = [
    // Pattern 1: Proper closing tag
    /<tool_call>\s*(\{[\s\S]*?\})\s*<\/tool_call>/,
    // Pattern 2: No closing tag, find balanced braces
    /<tool_call>\s*(\{[\s\S]*\})/,
  ];

  for (const pattern of patterns) {
    const match = output.match(pattern);
    if (match && match[1]) {
      try {
        const parsed = JSON.parse(match[1]);
        if (parsed.name && parsed.arguments) {
          return parsed as ToolCall;
        }
      } catch {
        // Try to salvage truncated JSON
        const salvaged = attemptJSONSalvage(match[1]);
        if (salvaged) return salvaged;
      }
    }
  }

  return null;
}

/**
 * Attempt to fix common JSON issues from FM output.
 */
function attemptJSONSalvage(jsonStr: string): ToolCall | null {
  // Try progressively shorter substrings ending at }
  let str = jsonStr;
  
  // Find all } positions and try parsing from longest to shortest
  const bracePositions: number[] = [];
  for (let i = str.length - 1; i >= 0; i--) {
    if (str[i] === '}') bracePositions.push(i);
  }

  for (const pos of bracePositions) {
    const candidate = str.slice(0, pos + 1);
    try {
      const parsed = JSON.parse(candidate);
      if (parsed.name && parsed.arguments) {
        return parsed as ToolCall;
      }
    } catch {
      continue;
    }
  }

  return null;
}
```

### 3.3 Add System Prompt Guidance

In the FM system prompt, add a gentle nudge:

```typescript
const SYSTEM_PROMPT = `...

JSON RULES:
- Keep file content reasonably short when possible
- Ensure valid JSON: escape newlines (\\n) and quotes (\\" )
- If writing large files, consider breaking into smaller chunks

...`;
```

### 3.4 Test

The improved parser should handle more edge cases. Test with path-tracing which produces long C code.

---

## Section 4: Graceful Handling of Unavailable Tools (Optional)

### 4.1 Problem

FM tries to run `primer3`, `python` etc. that aren't available locally:
```
sh: primer3: command not found
```
It then loops trying the same command repeatedly.

### 4.2 Solution

Detect common unavailable tools and return a helpful error once.

**Where:** FM `run_command` executor in `src/bench/model-adapter.ts`

**Implementation:**

```typescript
const UNAVAILABLE_LOCAL_TOOLS = [
  /^\s*primer3\b/,
  /^\s*python\b/,  // python3 might work, but python often doesn't
  /^\s*oligotm\b/,
];

function checkUnavailableTool(command: string): string | null {
  for (const pattern of UNAVAILABLE_LOCAL_TOOLS) {
    if (pattern.test(command)) {
      const toolName = command.match(/^\s*(\S+)/)?.[1] ?? "tool";
      return `${toolName} is not available in local environment. This task expects a container with ${toolName} installed.`;
    }
  }
  return null;
}

// In run_command executor:
async function executeRunCommand(args: { command: string }, workspace: string) {
  const unavailableMsg = checkUnavailableTool(args.command);
  if (unavailableMsg) {
    return {
      success: false,
      output: unavailableMsg,
    };
  }
  
  // ... normal execution
}
```

### 4.3 Priority

This is nice-to-have. Implement after Sections 1-3.

---

## Section 5: Validation Checklist

After implementing all sections:

### 5.1 Type Check
```bash
bun run typecheck
```

### 5.2 Unit Tests
```bash
bun test src/fm/
```

### 5.3 Integration Test

Run the same 3 tasks that failed:
```bash
bun run tbench -- --suite tasks/terminal-bench-2.json --model fm --tasks path-tracing,dna-assembly,regex-log
```

**Check logs for:**
- [ ] No `Exceeded model context window size` errors
- [ ] All completion paths show `[Orchestrator] Verification...` logs (including "3 consecutive failures")
- [ ] Fewer `No tool call parsed` errors (some are expected, but less than before)
- [ ] Task descriptions appear truncated in prompts (`...[truncated]`)

---

## File Summary

### Modified Files
| File | Changes |
|------|---------|
| `src/fm/worker.ts` | `truncateTaskDescription()`, JSON rules in system prompt |
| `src/fm/orchestrator.ts` | `finalizeIfDone()`, unified completion logic, state tracking |
| `src/bench/model-adapter.ts` or parser location | Improved `parseToolCalls()`, `attemptJSONSalvage()` |
| (Optional) FM run_command executor | `checkUnavailableTool()` |

---

## Notes for Implementer

1. **Find the prompt builder first**: Read current `worker.ts` to find where the full prompt is assembled. That's where `truncateTaskDescription()` goes.

2. **Find the completion logic**: Read current `orchestrator.ts` to find all the places where it returns early (task_complete, repeat heuristics). Refactor them to use `finalizeIfDone()`.

3. **Find parseToolCalls**: Search for "parseToolCalls" or "tool_call" regex. It might be in worker.ts or model-adapter.ts.

4. **Preserve fm-mini behavior**: All changes should be backward compatible. When `verifyTask` is undefined, trust completion signals.

5. **Log liberally during testing**: Add `console.log` statements to verify each fix is working, then remove them.

---

## Success Metrics

| Metric | Before | After |
|--------|--------|-------|
| Context window errors | ~30% of turns | 0 |
| Unverified "likely complete" exits | Yes (regex-log, dna-assembly) | No |
| Parse error rate | High (~50% on path-tracing) | Lower (some are unavoidable) |

---

## Future Work (Not This Batch)

1. **Per-task hints for TB2**: Once scaffolding is stable, add targeted hints like:
   > "regex-log: You don't need to read the log file. Just write a regex to /app/regex.txt."

2. **Streaming content for large files**: If FM can't write large files in one shot, consider a chunked approach.

3. **Tool availability detection**: Auto-detect what tools are available in the environment and tell FM.
