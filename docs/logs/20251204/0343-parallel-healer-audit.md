# 0343 Parallel Agent Healer Audit

**Date:** 2025-12-04
**Time:** 03:43 AM Central
**Session:** parallel-1764841065611
**Analysis:** Agent 2 Healer behavior during parallel run

## Executive Summary

✅ **Healer behaved correctly** - did NOT trigger because both agents started from a clean, passing codebase. This is the expected and desired behavior.

## Context

After fixing typecheck errors in mainview/index.ts (commit f5c6bed9), a new parallel run was launched with 2 agents to verify the fix prevented duplication of effort.

## Parallel Run Timeline

### Session Start
- **Time:** 09:37:45
- **Config:** 2 agents, 10 max tasks, Claude Code only, verbose mode
- **Ready tasks:** 75 (processing first 2)

### Agent Assignments

**Agent 1:**
- Task: oa-91f147 - "E2E: Error handling and resilience tests"
- Worktree: `.worktrees/oa-91f147`
- Session: session-2025-12-04T09-38-05-411Z-4ayy1b

**Agent 2:**
- Task: oa-4c9c36 - "Add self-healing validation to worktree manager"
- Worktree: `.worktrees/oa-4c9c36`
- Session: session-2025-12-04T09-37-58-146Z-r0fnxe

## Agent 2 Detailed Analysis

### Phase 1: Orientation (09:37:58 - 09:38:05)

**Init Script:** Skipped (as designed - worktrees skip init script)

**Initial Typecheck:** ✅ PASSED
```
[Agent 2] [event] verification_start
[Agent 2] Running: bun run typecheck
[Agent 2] [event] verification_complete
[Agent 2] ✅ Verification: bun run typecheck [host]
```

**Healer Decision:**
```
[DEBUG] shouldRunHealer decision: {
  run: false,
  reason: "Event does not trigger Healer",
}
```

**Analysis:** ✅ CORRECT - Typecheck passed, no healing needed.

### Phase 2: Task Decomposition (09:38:05)

**Task selected:** "Add self-healing validation to worktree manager"

**Subtasks created:** 1 subtask
- oa-4c9c36-sub-001: "Add self-healing validation to worktree manager"

**No Healer involvement** - task decomposition does not trigger Healer policy.

### Phase 3: Subtask Execution (09:38:05 - 09:41:05)

**Work performed:**
1. Read existing worktree code
2. Added `validateWorktree()` function to detect corrupted worktrees
3. Added `repairWorktree()` function to auto-recreate corrupted worktrees
4. Added `ensureValidWorktree()` main entry point
5. Integrated validation into `worktree-runner.ts`
6. Added comprehensive tests

**No Healer involvement during implementation** - Agent 2 worked cleanly without introducing errors.

### Phase 4: Post-Work Verification (09:41:05 - 09:41:44)

**Typecheck verification:** ✅ PASSED
```
[Agent 2] [event] verification_start
[Agent 2] Running: bun run typecheck
[Agent 2] [event] verification_complete
[Agent 2] ✅ Verification: bun run typecheck [host]
```

**Test verification:** ✅ PASSED (All 17 worktree tests passed)
```
[Agent 2] [event] verification_start
[Agent 2] Running: bun test
[Agent 2] [event] verification_complete
[Agent 2] ✅ Verification: bun test [host]
```

**Healer involvement:** NONE - All verification passed.

### Phase 5: Final Verification (09:41:46)

**Second typecheck:** ✅ PASSED
**Second test run:** ✅ PASSED

**Healer involvement:** NONE

### Completion (09:41:46)

**Status:** ✅ SUCCESS
**Duration:** 178 seconds (~3 minutes)
**Cost:** $1.23 USD
**Turns:** 25
**Result:** Subtask completed successfully

## Healer Policy Analysis

### Expected Healer Trigger Points

The Healer has 3 potential trigger points in the orchestrator:

1. **Init Script Failures** - Line 280-307 of orchestrator.ts
   - **Triggered:** NO - Init script was skipped in worktree (by design)
   - **Expected:** Correct - worktrees skip init script

2. **Subtask Failures** - Line 568-586 of orchestrator.ts
   - **Triggered:** NO - Subtask completed successfully without errors
   - **Expected:** Correct - no subtask failures occurred

3. **Verification Failures** - Line 662-728 of orchestrator.ts
   - **Triggered:** NO - All typecheck and test verification passed
   - **Expected:** Correct - no verification failures occurred

### Healer Decision Logic

The `shouldRunHealer` function evaluated:

```json
{
  "run": false,
  "reason": "Event does not trigger Healer"
}
```

This indicates the event type did not match any of the configured Healer scenarios:
- `onInitFailure`: false (init skipped)
- `onVerificationFailure`: false (verification passed)
- `onSubtaskFailure`: false (subtask succeeded)
- `onRuntimeError`: false (no runtime errors)
- `onStuckSubtask`: false (subtask completed in 3 minutes)

**Verdict:** ✅ CORRECT - Healer policy working as designed.

## Comparison to Previous Run

### Previous Run (Before typecheck fix)
- **Main repo state:** 7+ typecheck errors in mainview/index.ts
- **Agent behavior:** Both agents saw same errors, created duplicate fix-typecheck subtasks
- **Healer behavior:** Not triggered (agents attempted fixes before Healer could intervene)
- **Outcome:** Wasted effort, duplicate work

### This Run (After typecheck fix f5c6bed9)
- **Main repo state:** Clean, 0 typecheck errors
- **Agent behavior:** Both agents started clean, worked on assigned tasks
- **Healer behavior:** Not triggered (no failures to heal)
- **Outcome:** ✅ Success - both agents completed independent tasks efficiently

## Key Findings

### ✅ What Worked

1. **Pre-fixed main repo:** Manual typecheck fix (f5c6bed9) prevented the duplication issue
2. **Healer policy:** Correctly identified no healing was needed
3. **Worktree isolation:** Both agents worked independently without conflicts
4. **Skip init script:** Worktrees correctly skipped init script validation
5. **Clean execution:** Both agents completed tasks without introducing errors

### ⚠️ What's Still Missing

**Task oa-1a6f31 not yet implemented:** "Add pre-flight typecheck validation to overnight-parallel"

The parallel runner (`overnight-parallel.ts`) does NOT yet run pre-flight typecheck on the main repo before creating worktrees. This run succeeded because:
- Main repo was manually fixed before starting parallel run
- Both agents started from clean state

**Without pre-flight validation:**
- If main repo has typecheck errors when parallel run starts
- Both agents will see same errors and attempt same fixes
- Duplication of effort will occur again

**Required fix:** Modify `overnight-parallel.ts` to:
1. Run `bun run typecheck` on main repo BEFORE creating any worktrees
2. If typecheck fails, optionally invoke Healer to fix automatically
3. Only proceed with worktree creation once main repo is clean
4. Log pre-flight results to parallel session log

## Agent 1 Comparison

**Agent 1 behavior:**
- Also started with clean typecheck (✅ PASSED)
- Also had no Healer involvement
- Completed task oa-91f147 successfully in ~4 minutes
- No duplication with Agent 2 (different tasks)

**Verdict:** Both agents behaved identically with respect to Healer - neither needed it because main repo was clean.

## Conclusion

### Healer Behavior: ✅ CORRECT

The Healer behaved **exactly as designed**:
- Did not trigger when typecheck passed
- Did not trigger when subtasks succeeded
- Did not trigger when verification passed
- Respected policy configuration

### System Status

**Current state:** ✅ Working correctly when main repo is pre-fixed
**Missing protection:** ⚠️ Pre-flight validation (task oa-1a6f31)
**Risk:** If main repo has errors at parallel start, duplication will occur

### Recommendations

1. **Implement oa-1a6f31 BEFORE next overnight run** - Add pre-flight typecheck to overnight-parallel.ts
2. **Consider Healer pre-flight integration** - Option to auto-heal main repo before spawning agents
3. **Monitor next parallel run** - Verify pre-flight validation prevents duplication
4. **Document pattern** - Add "pre-flight validation" as standard practice for parallel systems

## Cost Analysis

**Agent 2 session cost:** $1.23 USD
- Input tokens: 583
- Output tokens: 9,891
- Cache read: 1,099,627 tokens
- Duration: 178 seconds (~3 minutes)

**No Healer cost:** $0.00 (Healer did not run)

**Total efficiency:** ✅ Maximum - no wasted Healer invocations, no duplicate work

## Artifacts

**Files modified by Agent 2:**
- `src/agent/worktree/worktree.ts` - Added validation/repair functions
- `src/agent/worktree/worktree-runner.ts` - Integrated validation
- `src/agent/worktree/worktree.test.ts` - Added validation/repair tests

**Tests added:** 17 total worktree tests (all passing)

**No Healer artifacts:** No Healer sessions, trajectories, or logs created (as expected).

## Status: 🟢 HEALER WORKING AS DESIGNED

The Healer integration is functioning correctly. The absence of Healer invocation in this run is the **correct behavior** when the codebase is clean. The next step is implementing pre-flight validation (oa-1a6f31) to ensure the codebase is always clean before parallel agents start.
