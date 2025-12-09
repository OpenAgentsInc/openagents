# Next Steps: Get HillClimber Solving regex-log

- **Date:** 2025-12-08
- **Time:** 20:01 CT
- **Context:** Previous agent implemented FM integration, error handling, and fixes. Now we need to actually run it and make it work.

---

## Current State Summary

**What's Been Implemented:**
1. ✅ FM connected to MAP orchestrator (replaced placeholder `getNextAction()`)
2. ✅ Evaluator JSON.parse bugs fixed with try/catch
3. ✅ Integration test created: `src/hillclimber/e2e-regex-log.test.ts`
4. ✅ Monitor rejection feedback added to state
5. ✅ Prompt clarified: "task description is already provided, no need to read files"
6. ✅ Auto-advance subtask after 5 turns with no progress
7. ✅ FMActionError type with specific error reasons (no more "stuck")

**Known Issues (from 19:45 testing):**
- FM kept trying to read "task.md" which doesn't exist
- FM not understanding that task description is IN the prompt
- Fixes applied but NOT YET TESTED

---

## YOUR MISSION

**Goal:** Run the integration test, debug FM behavior, and get `regex-log` solving (9/9 tests passing) in < 15 turns.

---

## Step 1: Run the Integration Test

```bash
bun test src/hillclimber/e2e-regex-log.test.ts
```

Watch the output carefully. Look for:
- Does FM understand the task is in the prompt?
- Does FM try to write to `/app/regex.txt`?
- Are verification results being parsed correctly?
- Does FM iterate based on test failure feedback?

---

## Step 2: Debug FM Behavior Patterns

**If FM still tries to read task.md:**
- The prompt needs to be even more explicit
- Add to `formatFMPrompt()`: "DO NOT read any files for task information. The task description above is complete."
- Consider adding workspace file listing so FM knows what files ACTUALLY exist

**If FM doesn't write to regex.txt:**
- Check if FM understands the output path
- Decomposer hints say `/app/regex.txt` - make sure FM sees this

**If FM writes regex but doesn't iterate:**
- Check if verification feedback is being formatted correctly
- `formatForPrompt(state.lastEvaluation)` should show specific failures
- FM needs to see: "test_boundary_ip FAILED: expected [], got ['2024-01-15']"

**If tests pass but progress doesn't increase:**
- Check evaluator pytest parsing
- Verify `parsePytestOutput()` matches actual TB2 output format

---

## Step 3: Tune Prompts

The key prompt is in `map-orchestrator.ts` `formatFMPrompt()`. Current structure:

```
## Current Goal
{subtask goal}

## Success Checkpoint
{checkpoint}

## Verification Status
{pass/fail + specific failures}

## Hints
{subtask hints + suggestions}

## Recent Actions
{last 3 actions}
```

**Potential Improvements:**
1. Add "## Available Files in Workspace" section
2. Make verification feedback more actionable: "FIX THIS: IP 256.0.0.1 was incorrectly matched"
3. Include the current content of regex.txt if it exists
4. Be more specific about what "Last date" means in hints

---

## Step 4: FM Context Budget Check

FM has ~3000 token limit. If prompts are too long:
1. Compress history more aggressively
2. Drop older verification results (keep only last 2)
3. Truncate task description after first successful write

---

## Step 5: Success Criteria

Test passes when:
```typescript
expect(result.passed).toBe(true);
expect(result.turns).toBeLessThan(15);
```

Meaning:
- All 9 pytest tests for regex-log pass
- Done in under 15 turns (ideally 8-10)

---

## Files You May Need to Modify

| File | What to change |
|------|----------------|
| `src/hillclimber/map-orchestrator.ts` | `formatFMPrompt()` for better prompting |
| `src/hillclimber/evaluator.ts` | `generateSuggestion()` for regex-log specific hints |
| `src/hillclimber/decomposer.ts` | `REGEX_LOG_DECOMPOSITION` hints if FM needs more guidance |
| `src/hillclimber/e2e-regex-log.test.ts` | Test setup/assertions if needed |

---

## Debugging Commands

```bash
# Run test with verbose output
bun test src/hillclimber/e2e-regex-log.test.ts --verbose

# Check FM service is running
curl http://localhost:11435/health

# If FM not running, start it
bun run fm:start

# Check what's in the workspace after a run
ls -la .hillclimber-workspaces/regex-log/
cat .hillclimber-workspaces/regex-log/regex.txt
```

---

## Key Code Locations

- **FM Call:** `map-orchestrator.ts:663-722` `getNextAction()`
- **Prompt Building:** `map-orchestrator.ts:179-218` `formatFMPrompt()`
- **Verification Parsing:** `evaluator.ts:104-176` `parsePytestOutput()`
- **Failure Suggestions:** `evaluator.ts:233-282` `generateSuggestion()`
- **Decomposition:** `decomposer.ts:54-122` `REGEX_LOG_DECOMPOSITION`
- **Test:** `e2e-regex-log.test.ts`

---

## What Success Looks Like

```
[MAP] === Turn 1 (Subtask 1: understand-task) ===
[MAP-FM] FM returned tool: read_file  # reads existing files if any
[MAP] === Turn 2 (Subtask 2: write-initial-regex) ===
[MAP-FM] FM returned tool: write_file
[MAP] Wrote regex to /app/regex.txt
[MAP] Running verification...
[MAP] Progress: 33.3% (3/9 tests)
[MAP] === Turn 3 (Subtask 3: test-and-iterate) ===
[MAP-FM] FM sees: "test_boundary_ip FAILED: IP validation too loose"
[MAP-FM] FM returned tool: write_file  # fixes IP validation
[MAP] Progress: 55.5% (5/9 tests)
... iteration continues ...
[MAP] === Turn 8 ===
[MAP] Progress: 100% (9/9 tests)
[MAP] TASK PASSED!
```

---

## If You Get Completely Stuck

1. Read the current prompt FM is receiving (add console.log in getNextAction)
2. Check if FM response is being parsed correctly
3. Verify TB2 task files are in place for regex-log
4. Try a simpler test - just verify FM can write any file
5. Consider if FM temperature needs adjustment (too random or too deterministic?)

---

## Document Your Progress

Create a log file at `docs/logs/20251208/20XX-your-log.md` with:
- What you tried
- What worked / didn't work
- Final state (passing or not)
- Any remaining issues

Good luck. The goal is simple: make the green checkmark appear.
