# 0055 FM Terminal-Bench Implementation Log

**Date:** 2025-12-08
**Task:** Implement FM TB2 fixes from 0050-fm-tb-implementation-guide.md
**Status:** ✅ Complete

---

## Summary

Implemented all 5 sections from the implementation guide to fix systemic issues causing FM TB2 failure:

1. ✅ Suite-Aware Hints
2. ✅ StepSummary (Context Truncation)
3. ✅ Verification-Gated Completion
4. ✅ Path Normalization
5. ✅ Skills Presentation

---

## Section 1: Suite-Aware Hints

### Created Files
- `src/fm/hints.ts` - Suite-aware hint system with `getSuiteMode()` and `buildHint()`
- `src/fm/hints.test.ts` - Tests for hint system

### Modified Files
- `src/fm/orchestrator.ts`:
  - Added `suiteMode` to `OrchestratorOptions`
  - Imported `buildHint` and `SuiteMode`
  - Added `toolHistory` array to track tool names
  - Integrated hint building in worker input construction

- `src/fm/worker.ts`:
  - Added `hint` field to `WorkerPromptInput`
  - Replaced inline hint logic with suite-aware hint from orchestrator

- `src/bench/model-adapter.ts`:
  - Added `suitePath` to `RunTaskOptions`
  - Imported `getSuiteMode`
  - Computed `suiteMode` and passed to orchestrator

- `src/cli/tbench-local.ts`:
  - Added `suitePath` to `runTask` options
  - Threaded `args.suite` through to model runner

### Key Changes
- TB2 gets NO hints by default (returns `undefined`)
- Only fm-mini uses hints
- Tool names tracked separately for hint system (not full outputs)

### Tests
All tests pass:
- `getSuiteMode` detects fm-mini, tb2, and unknown modes
- `buildHint` returns undefined for tb2/unknown, hints for fm-mini

---

## Section 2: StepSummary (Context Truncation)

### Created Files
- `src/fm/step-summary.ts` - StepSummary type and builders
- `src/fm/step-summary.test.ts` - Tests for StepSummary

### Modified Files
- `src/fm/orchestrator.ts`:
  - Imported `summarizeToolResult`, `buildPreviousField`, `StepSummary`
  - Added `stepHistory: StepSummary[]` array
  - Replaced `previous` field construction to use `buildPreviousField(stepHistory)`
  - Added `summarizeToolResult()` calls after tool execution
  - Passed `toolCall.arguments` to `summarizeToolResult` for tool-aware summaries
  - Added summaries for parse errors and general errors

### Key Changes
- Tool-aware summaries: "Wrote 123 bytes to foo.txt" instead of truncated raw output
- Keeps only last 3 entries in previous field (MAX_SUMMARIES = 3)
- Each message capped at 100 chars (MAX_MESSAGE_CHARS = 100)
- Previous field format: "Step N (tool): message; Step N+1 (tool): message"

### Tests
All tests pass:
- `summarizeToolResult` produces tool-aware summaries
- `buildPreviousField` keeps only last 3 entries
- Truncation works for unknown tools

---

## Section 3: Verification-Gated Completion

### Modified Files
- `src/fm/orchestrator.ts`:
  - Added `verifyTask?: () => Promise<boolean>` to `OrchestratorOptions`
  - Added `maxRetryAfterFailedVerify?: number` (default: 2)
  - Added `verifyRetryCount` tracking
  - Updated completion logic to check verification when FM signals `task_complete` OR `repeatCount >= MAX_REPEAT_ACTIONS`
  - On verification failure: add feedback to history, reset repeat counter, continue loop
  - On max retries: return failure with error message

- `src/bench/model-adapter.ts`:
  - Imported `runTaskVerification` and `BunContext`
  - Created `verifyTask` callback that wraps existing TB verification
  - Passed `verifyTask` and `maxRetryAfterFailedVerify: 2` to orchestrator

### Key Changes
- Verification only called on "done" signals (task_complete or repeated actions)
- NOT on every turn (avoids overhead)
- After failed verification: reset `repeatCount = 0` to give FM fresh chances
- Reuses existing `runTaskVerification` from TB harness (no new verification system)

---

## Section 4: Path Normalization

### Modified Files
- `src/fm/worker.ts`:
  - Updated `WORKER_SYSTEM` prompt with PATH RULES section
  - Added explicit tool list warning: "The ONLY tools you may call are: ..."
  - Added warning: "If you put any other name in the 'name' field, it will fail"

- `src/bench/model-adapter.ts`:
  - Added `normalizeCommand()` function (FM runner only)
  - Applied normalization in FM runner's `run_command` executor
  - Normalizes: `/app/` → `./`, strips `cd /app &&` and `cd /app;` prefixes
  - Logs normalization in output when command is changed

### Key Changes
- Normalization ONLY in FM runner (not Ollama/Claude Code - they may have real /app paths)
- System prompt warns FM about /app/ paths upfront
- Command normalization happens at execution time (not in prompt)

---

## Section 5: Skills Presentation

### Modified Files
- `src/fm/worker.ts`:
  - Updated skills formatting: "Example approaches (for reference only, NOT callable tools)"
  - Changed format: description only (no skill name), bullet points
  - Truncated to 80 chars per skill

- `src/bench/model-adapter.ts`:
  - Added suite-aware skills toggle: `useSkillsForTask = suiteMode === "fm-mini"`
  - TB2 gets `skills = []` (no skill retrieval call)
  - Only calls `getRelevantSkills()` when `useSkillsForTask === true`
  - Logs when skills are disabled for suite mode

### Key Changes
- TB2: Skills disabled initially (clean baseline)
- fm-mini: Skills enabled (existing behavior preserved)
- Skills clearly marked as "NOT callable tools" in prompt
- Explicit tool list in system prompt prevents FM from calling skill names

---

## Validation

### Type Check
```bash
bun run typecheck
```
✅ Passed with zero errors

### Unit Tests
```bash
bun test src/fm/hints.test.ts src/fm/step-summary.test.ts
```
✅ All 12 tests pass:
- 6 hint tests (getSuiteMode, buildHint)
- 6 step-summary tests (summarizeToolResult, buildPreviousField)

---

## Files Created

1. `src/fm/hints.ts` - Suite-aware hint system
2. `src/fm/hints.test.ts` - Hint tests
3. `src/fm/step-summary.ts` - StepSummary type and builders
4. `src/fm/step-summary.test.ts` - StepSummary tests

## Files Modified

1. `src/fm/orchestrator.ts` - Hints, StepSummary, verification-gated completion
2. `src/fm/worker.ts` - System prompt updates, skills formatting, hint integration
3. `src/bench/model-adapter.ts` - SuiteMode, verification callback, path normalization, skills toggle
4. `src/cli/tbench-local.ts` - Thread suitePath through to model runner

---

## Next Steps

According to the guide, integration tests should be run:

1. **fm-mini test:**
   ```bash
   bun run tbench -- --suite tasks/terminal-bench-mini.json --model fm
   ```
   Should maintain current pass rate (hints still work for fm-mini).

2. **TB2 sample test:**
   ```bash
   bun run tbench -- --suite tasks/terminal-bench-2.json --model fm --tasks path-tracing,regex-log,dna-assembly
   ```
   Check logs for:
   - [ ] No hint appears for TB2 tasks
   - [ ] Previous field is <300 chars
   - [ ] FM gets "verification failed" feedback when task_complete is premature
   - [ ] No `/app/` path errors in run_command
   - [ ] No `Exceeded model context window size` errors

---

## Notes

- All changes are backward compatible (fm-mini behavior preserved)
- Verification reuses existing TB mechanism (no new verification system)
- Path normalization is FM-runner-specific (doesn't affect other models)
- Skills are disabled for TB2 to provide clean baseline for measuring other fixes
