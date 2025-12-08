# 0135 FM TB Round 2 Fixes Implementation Log

**Date:** 2025-12-08
**Task:** Implement FM TB Round 2 fixes from 0127-fm-tb-round2-fixes.md
**Status:** ✅ Complete (with follow-up improvements)

---

## Summary

Implemented all 4 sections from the round 2 fixes guide to address remaining scaffolding issues:

1. ✅ Hard-Cap Task Description (truncate to 600 chars)
2. ✅ Unify All Completion Paths Through Verification (finalizeIfDone)
3. ✅ Make parseToolCalls More Forgiving (JSON salvage logic)
4. ✅ Graceful Handling of Unavailable Tools (checkUnavailableTool)

---

## Section 1: Hard-Cap Task Description

### Problem
Full TB2 task descriptions (~3.5k chars) were causing:
```
Error: Foundation Models request failed: Exceeded model context window size
```

### Solution
- Added `truncateTaskDescription()` function with `MAX_TASK_CHARS = 600`
- Applied truncation in `buildWorkerPrompt()` before injecting into prompt
- Added `...[truncated]` suffix when description is cut
- Updated logging to show prompt length

### Modified Files
- `src/fm/worker.ts`:
  - Added `truncateTaskDescription()` function
  - Updated `buildWorkerPrompt()` to use truncated description
  - Improved prompt length logging

### Key Changes
- Task descriptions now capped at 600 chars
- Preserves first 600 chars (most important context)
- Logs prompt length for debugging

---

## Section 2: Unify All Completion Paths Through Verification

### Problem
The "3 consecutive failures after success" heuristic was bypassing `verifyTask`:
```
[Orchestrator] 3 consecutive failures after success - task likely complete
```
No verification ran, causing false positives.

### Solution
- Created `finalizeIfDone()` function that ALL completion paths use
- Unified three completion paths:
  1. `task_complete` signal
  2. `repeat_same_action` (same tool+args repeated 3x)
  3. `repeat_failures` (3 consecutive failures after success)
- All paths now go through verification when `verifyTask` exists

### Modified Files
- `src/fm/orchestrator.ts`:
  - Added `CompletionReason` type
  - Created `finalizeIfDone()` function
  - Refactored all completion paths to use `finalizeIfDone()`
  - Fixed state management: reset counters after failed verification

### Key Changes
- All completion paths now verify before returning success
- Failed verification resets counters and continues loop
- Backward compatible: when `verifyTask` is undefined, trusts completion signals
- State management: `consecutiveFailures` reset on success, incremented on failure

---

## Section 3: Make parseToolCalls More Forgiving

### Problem
FM emits tool calls with long content that breaks JSON parsing:
```
No tool call parsed, raw: <tool_call>{"name":"write_file","arguments":{"path":"image.c","content":"#include <stdio.h>...
```
JSON was truncated or had unescaped characters.

### Solution
- Added `attemptJSONSalvage()` function
- Improved regex patterns to capture JSON more reliably
- Tries progressively shorter substrings ending at `}` to find valid JSON
- Added JSON rules to system prompt

### Modified Files
- `src/bench/model-adapter.ts`:
  - Added `attemptJSONSalvage()` function
  - Improved `parseToolCalls()` with salvage logic
  - Better handling of truncated JSON
- `src/fm/worker.ts`:
  - Added JSON RULES section to system prompt

### Key Changes
- More forgiving JSON parsing for truncated tool calls
- Tries multiple `}` positions to find valid JSON
- System prompt now warns FM about JSON formatting

---

## Section 4: Graceful Handling of Unavailable Tools

### Problem
FM tries to run tools like `primer3`, `python` that aren't available locally:
```
sh: primer3: command not found
```
It then loops trying the same command repeatedly.

### Solution
- Added `checkUnavailableTool()` function
- Detects common unavailable tools before execution
- Returns helpful error message instead of letting command fail

### Modified Files
- `src/bench/model-adapter.ts`:
  - Added `checkUnavailableTool()` function
  - Applied check in FM runner's `run_command` executor
  - Detects: `primer3`, `python`, `oligotm`

### Key Changes
- Early detection of unavailable tools
- Clear error message explaining tool needs container
- Prevents infinite retry loops

---

## Validation

### Type Check
```bash
bun run typecheck
```
✅ Passed with zero errors

### Unit Tests
```bash
bun test src/fm/ src/bench/normalize-command.test.ts
```
✅ All 54 tests pass (9 skipped - require macOS bridge)

---

## Files Modified

1. `src/fm/worker.ts` - Task truncation, JSON rules in prompt
2. `src/fm/orchestrator.ts` - `finalizeIfDone()`, unified completion logic
3. `src/bench/model-adapter.ts` - Improved `parseToolCalls()`, `checkUnavailableTool()`

---

## Next Steps

According to the guide, integration tests should be run:

```bash
bun run tbench:local -- --suite tasks/terminal-bench-2.json --model fm --tasks path-tracing,dna-assembly,regex-log
```

**Check logs for:**
- [ ] No `Exceeded model context window size` errors
- [ ] All completion paths show `[Orchestrator] Verification...` logs (including "3 consecutive failures")
- [ ] Fewer `No tool call parsed` errors (some are expected, but less than before)
- [ ] Task descriptions appear truncated in prompts (`...[truncated]`)

---

## Success Metrics

| Metric | Before | After (Expected) |
|--------|--------|------------------|
| Context window errors | ~30% of turns | 0 |
| Unverified "likely complete" exits | Yes (regex-log, dna-assembly) | No |
| Parse error rate | High (~50% on path-tracing) | Lower (some are unavoidable) |

---

## Follow-Up Improvements (Post-Review)

### Parse Error Metrics
- Added `parseErrorCount` counter in orchestrator
- Logs parse error count at end of task run
- Helps track whether `attemptJSONSalvage` is actually helping
- Logged in all exit paths (timeout, max turns, completion)

### ParseToolCalls Tests
- Created `src/bench/parse-tool-calls.test.ts`
- 7 tests covering:
  - Valid tool calls (with/without closing tag)
  - Truncated JSON salvage
  - Multiple tool calls
  - Invalid input handling
- All tests pass ✅

---

## Notes

- All changes are backward compatible (fm-mini behavior preserved)
- Verification is now mandatory for all completion paths when `verifyTask` exists
- JSON parsing is more robust but some edge cases may still fail
- Unavailable tool detection prevents infinite retry loops
- Parse error metrics help track parsing improvements over time
