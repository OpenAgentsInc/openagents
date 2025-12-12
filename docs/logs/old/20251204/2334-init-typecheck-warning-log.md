# 2334 Init Script Typecheck Non-Fatal Fix

## Session Context

Continued from MC assign button implementation. User reported MechaCoder spawns were failing during init script typecheck phase with "Operation aborted" errors from Claude Code SDK.

## Problem

Init script `.openagents/init.sh` was treating typecheck failures as **fatal** (exit 1), causing MechaCoder sessions to abort immediately during preflight checks.

User feedback:
> "I'm happy for it to do a type check. I just don't want it to fail there. I want it to report it to the agent so the agent knows about it and can fix it if it's relevant."

## Solution

Changed `.openagents/init.sh` line 52-55 to treat typecheck failures as **warnings** (exit 2) instead of fatal errors:

### Before
```bash
# 2. Smoke test: typecheck
log "Running smoke test (typecheck)..."
if ! bun run typecheck >> "$LOG_FILE" 2>&1; then
    fatal "Typecheck failed at preflight. Fix errors before running agent."
fi
log "Typecheck passed."
```

### After
```bash
# 2. Smoke test: typecheck
log "Running smoke test (typecheck)..."
if ! bun run typecheck >> "$LOG_FILE" 2>&1; then
    warn "Typecheck failed at preflight. Agent will be notified to fix type errors."
else
    log "Typecheck passed."
fi
```

## How It Works

Exit code semantics in init script:
- **0** = All checks passed (success)
- **1** = Fatal error (abort session)
- **2** = Warnings only (continue with caution)

When typecheck fails:
1. `warn()` increments `WARNINGS` counter
2. Script exits with code 2 at end
3. Orchestrator treats exit code 2 as `success=true, hasWarnings=true`
4. Init script output (including typecheck errors) is captured in log file
5. Agent receives init script result via `init_script_complete` event
6. Agent can see and fix type errors if relevant to its task

## Validation

```bash
$ bash .openagents/init.sh; echo "Exit code: $?"
[2025-12-04T23:34:30-06:00] === Golden Loop v2 Preflight Checklist ===
[2025-12-04T23:34:30-06:00] Project: openagents
[2025-12-04T23:34:30-06:00] Checking git status...
[2025-12-04T23:34:30-06:00] WARNING: Uncommitted changes detected:
[2025-12-04T23:34:30-06:00] Running smoke test (typecheck)...
[2025-12-04T23:34:31-06:00] Typecheck passed.
[2025-12-04T23:34:32-06:00] === Preflight Complete ===
[2025-12-04T23:34:32-06:00] Completed with 1 warning(s). Review docs/logs/20251204/2334-preflight.log for details.
Exit code: 2
```

Exit code 2 indicates warnings (uncommitted changes in this case). If typecheck had failed, it would also contribute to the warnings count and exit code 2.

## Architecture

The orchestrator already has full support for init script warnings:

**src/agent/orchestrator/init-script.ts:121-122**
```typescript
const success = exitCode === 0 || exitCode === 2;
const hasWarnings = exitCode === 2;
```

**src/agent/orchestrator/orchestrator.ts:567-571**
```typescript
const healerOutcome = yield* config.healerService.maybeRun(
  { type: "init_script_complete", result: initScriptResult },
  state,
  config.projectConfig,
  config.healerCounters ?? createHealerCounters()
);
```

The Healer service can inspect `initScriptResult.hasWarnings`, `initScriptResult.output`, and `initScriptResult.failureType` to determine if it should spawn Claude Code to fix type errors.

## Files Modified

1. `.openagents/init.sh` - Changed typecheck failure from `fatal` to `warn`

## Design Decisions

1. **Warnings over fatal**: Typecheck failures are development issues, not environment issues - agent should attempt to fix them
2. **Keep running typecheck**: Still valuable to catch type errors early, just don't abort
3. **Full output captured**: Typecheck output still logged to preflight log file for agent inspection
4. **Reuse existing mechanism**: Exit code 2 already fully supported by orchestrator and Healer service

## Impact

MechaCoder can now:
- Start successfully even when repo has type errors
- See type errors in init script output
- Fix type errors if they're relevant to current task
- Proceed with task if type errors are unrelated

This aligns with the "self-healing" philosophy in Golden Loop v2 - don't fail fast on fixable issues, let the agent assess and repair.

## Session Duration

- Start: 23:34 CT
- End: 23:34 CT
- Duration: ~1 minute

## Next Steps

Test manually by:
1. Introducing a type error in the codebase
2. Running `bun dev` to spawn desktop app
3. Clicking "Assign" on a ready task
4. Verify MechaCoder starts and receives init script warnings
5. Check if agent attempts to fix type errors or proceeds with task
