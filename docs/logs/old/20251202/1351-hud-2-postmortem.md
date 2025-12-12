# 1351 HUD-2 Postmortem

## Task
- **ID**: oa-138548
- **Title**: HUD-2: Flow layout engine (calculateLayout)
- **Priority**: P1

## MechaCoder Run Summary

**Run time**: 2025-12-02 19:39:56 - 19:49:37 (about 10 minutes)
**Turns used**: 6
**Retries triggered**: 0
**Final status**: INCOMPLETE_NO_TASK_COMPLETED

### What MechaCoder Did

1. **Exploration Phase** (turns 1-2):
   - Looked for src/flow/layout.ts (not found)
   - Searched for flow-related files
   - Found src/flow/model.ts and sample-data.ts from HUD-1
   - Read git log to understand recent changes

2. **Implementation Phase** (turns 3-4):
   - Read model.ts and sample-data.ts to understand types
   - Ran bun test and bun run typecheck (both passed)
   - Created src/flow/layout.ts with:
     - LayoutConfig, LayoutInput, LayoutOutput interfaces
     - calculateLayout() function with:
       - Cycle/duplicate ID detection
       - Size validation
       - Horizontal/vertical layout support
       - Connection waypoint generation
   - Created src/flow/layout.test.ts with 9 test cases

3. **Where It Stopped** (turn 5-6):
   - Did NOT run typecheck after writing files
   - Did NOT run tests after writing files
   - Did NOT commit/push
   - Did NOT close task via CLI
   - Final message was literally `git status` output (garbage)

### Why Retry Logic Didn't Kick In

The hardened retry logic detects typecheck/test failures **from tool results**. But MechaCoder:
- Ran typecheck/tests BEFORE writing the new files (they passed trivially)
- Did NOT run them AFTER writing the new files
- So there were no typecheck failures in the tool results to detect

The agent simply stopped with a non-tool-call message that wasn't TASK_COMPLETED.

### Issues Found in Generated Code

1. **Test file used `vitest` instead of `bun:test`**
   - Project uses bun:test throughout
   - MechaCoder apparently knows vitest patterns but not the local convention

2. **Duplicate variable declaration in layout.ts**
   - `let curX` declared twice in same scope (lines 70 and 83 originally)
   - Classic copy-paste error from comments

3. **Unused import**
   - `LayoutInput` imported but only used as type (not an error but flagged)
   - `vi` imported from vitest but never used

### Manual Fixes Applied

1. Changed `import { describe, it, expect, vi } from 'vitest'` to `import { describe, it, expect } from 'bun:test'`
2. Removed duplicate `let curX` declaration and cleaned up dead comment code
3. Removed unused `LayoutInput` from import

### Verification

After fixes:
- `bun run typecheck` - passed
- `bun test src/flow/` - 9 tests passed
- `bun test` - 105 tests passed (96 existing + 9 new)

### Commits

- **29e84b30**: oa-138548: HUD-2 Flow layout engine (calculateLayout)

## Analysis: Why Did MechaCoder Stop Early?

### Root Cause

The LLM (grok-4.1-fast) appears to have a pattern of:
1. Doing exploratory work (reading files, running commands)
2. Writing implementation code
3. Then emitting a message that looks like it's "checking status" but isn't a tool call

This manifests as the final message being raw `git status` output instead of either:
- Another tool call to actually run typecheck/tests
- `TASK_COMPLETED: oa-138548 - ...`

### Observations

1. **6 turns is too few**: The agent used all exploration + implementation turns but had no turns left for verification/commit
2. **No typecheck-after-write**: The agent ran typecheck before writing, not after
3. **Final message is noise**: `On branch main...` is clearly not intentional

### Potential Fixes

1. **Increase maxTurns per loop iteration**: Currently 30, but agent burned them on exploration
2. **Add explicit "verify after write" phase**: After detecting write tool calls, inject a user message forcing typecheck/test run
3. **Detect garbage final messages**: If final message looks like command output (starts with common patterns), treat as incomplete
4. **Track typecheck timing**: Detect if typecheck was run BEFORE the last write, and force re-run

## Files Created

- `src/flow/layout.ts` - Layout engine with calculateLayout function
- `src/flow/layout.test.ts` - 9 unit tests

## Artifacts

- Task run log: `docs/logs/20251202/133956-task-run.md`
- Session JSONL: `.openagents/sessions/session-20251202-193956-rox5.jsonl`
- Run metadata: `.openagents/run-logs/20251202/134937-oa-138548.json`

## Conclusion

The hardening work (retry on typecheck failure) helped somewhat - the infrastructure correctly detected INCOMPLETE_NO_TASK_COMPLETED. However, the retry logic didn't trigger because MechaCoder didn't run typecheck AFTER writing files.

**Next improvement needed**: Force typecheck/test verification after any write/edit tool calls before accepting a final message.
