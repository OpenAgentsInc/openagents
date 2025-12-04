# Healer Synchronous Integration Fix

**Status:** Healer fully implemented (168 tests passing) but integration is broken
**Problem:** Healer runs asynchronously AFTER orchestrator fails, so it never gets to act
**Solution:** Move Healer invocation to synchronous Effect.gen blocks BEFORE failure decisions

---

## Problem Analysis

### Current Broken Integration

**Location:** `src/agent/do-one-task.ts:860-882`

```typescript
// BROKEN - Runs asynchronously in emit() callback
const emit = (event: OrchestratorEvent) => {
  // ... logging ...

  Effect.runPromise(
    healerService.maybeRun(event, currentState, projectConfig, healerCounters)
  ).then((outcome) => {  // ← .then() = async!
    // Orchestrator has ALREADY decided to fail by this point
    // Healer never gets a chance to fix the issue
  });
};
```

**Timeline of Events:**
1. Init script runs and fails with typecheck errors
2. Orchestrator emits `init_script_complete` event
3. Orchestrator checks `!initScriptResult.success` → decides to fail immediately
4. Orchestrator aborts session
5. **THEN** Healer callback fires asynchronously (too late!)

### Why It Happens

The `emit()` function is used for logging/HUD updates, not for decision-making. By the time the async `.then()` callback executes, the orchestrator's `Effect.gen` function has already:
- Set `state.phase = "failed"`
- Emitted `session_complete`
- Returned from the Effect

### Root Cause

**Healer invocation must be SYNCHRONOUS** within the orchestrator's Effect workflow, not async in a callback.

---

## Solution Architecture

### The Pattern: `yield*` for Synchronous Effects

**Old Safe-Mode Pattern (Working):**

```typescript
// From orchestrator.ts:280-370 - SYNCHRONOUS with yield*
if (initScriptResult.ran && !initScriptResult.success && config.safeMode) {
  const healingSubtask = createEmergencySubtask();

  // ✅ SYNCHRONOUS - Waits for completion
  const healResult = yield* Effect.tryPromise({
    try: () => runClaudeCodeSubagent(healingSubtask, opts),
    catch: (e) => new Error(`Self-healing failed: ${e.message}`),
  });

  if (healResult.success) {
    // Re-run init script to verify
    const retryResult = yield* runInitScript(openagentsDir, config.cwd, emit);

    if (retryResult.success) {
      // Update state and CONTINUE
      initScriptResult = retryResult;
    }
  }
}

// This check only runs AFTER healing completes
if (initScriptResult.ran && !initScriptResult.success) {
  state.phase = "failed";  // Only fail if healing didn't fix it
  return state;
}
```

**New Healer Pattern (Same Approach):**

```typescript
// Replace safe-mode with Healer using identical pattern
if (initScriptResult.ran && !initScriptResult.success && config.healer?.enabled) {
  // ✅ SYNCHRONOUS - Waits for completion
  const healerOutcome = yield* healerService.maybeRun(
    { type: "init_script_complete", result: initScriptResult },
    state,
    projectConfig,
    healerCounters
  );

  if (healerOutcome?.status === "resolved") {
    // Re-run init script to verify
    const retryResult = yield* runInitScript(openagentsDir, config.cwd, emit);

    if (retryResult.success) {
      // Update state and CONTINUE
      initScriptResult = retryResult;
    }
  }
}

// Same check - only fails if healing didn't work
if (initScriptResult.ran && !initScriptResult.success) {
  state.phase = "failed";
  return state;
}
```

### Key Insight: No BunContext Needed!

```typescript
// Healer Effect signature
maybeRun(...): Effect.Effect<HealerOutcome | null, Error, never>
//                                                      ^^^^^ No context dependencies!
```

The `never` in the third type parameter means Healer requires NO Effect context layers. You can `yield*` it directly without providing BunContext.

---

## Recommended Approach: Phased Integration

### Phase 1: Init Script Integration (P0 - Critical)

**Goal:** Get Healer working for init script failures (most common case)

**Changes:**
1. Create full Healer service with ClaudeCodeInvoker and VerificationRunner
2. Thread service to orchestrator via config
3. Replace old safe-mode (lines 280-370) with Healer invocation
4. Remove broken async callback (lines 860-882)

**Risk:** MEDIUM (proven pattern + 168 passing tests)
**Impact:** HIGH (unblocks overnight runs)

### Phase 2: Complete Integration (P1 - Important)

**Goal:** Add Healer at subtask and verification failure points

**Changes:**
1. Add Healer after subtask failure (line 629+)
2. Add Healer after verification failure (line 704+)
3. Consistent outcome handling across all 3 points

**Risk:** LOW (reusing Phase 1 pattern)
**Impact:** MEDIUM (catches more failure scenarios)

### Phase 3: Cleanup (P2 - Nice to Have)

**Goal:** Remove old code and polish

**Changes:**
1. Delete old safe-mode code entirely
2. Deprecate `safeMode` config field
3. Update tests to use Healer
4. Clean up debug logging

---

## Implementation Plan: Phase 1 (Init Script)

### Step 1: Add Healer Service to Orchestrator Config

**File:** `src/agent/orchestrator/types.ts`

```typescript
export interface OrchestratorConfig {
  // ... existing fields ...

  // NEW: Optional Healer integration
  healerService?: {
    maybeRun: (
      event: OrchestratorEvent,
      state: OrchestratorState,
      config: ProjectConfig,
      counters: HealerCounters
    ) => Effect.Effect<HealerOutcome | null, Error, never>;
  };
  healerCounters?: HealerCounters;
  projectConfig?: ProjectConfig;
}
```

**Why optional?** Backward compatibility - old code still works without Healer.

---

### Step 2: Create Full Healer Service

**File:** `src/agent/do-one-task.ts`

**Imports to add:**

```typescript
import {
  createHealerService,
  createHealerCounters,
  type HealerOutcome,
  type HealerCounters,
} from "../healer/index.js";
import type { ProjectConfig } from "../tasks/schema.js";
```

**Service creation (around line 781):**

```typescript
// Initialize Healer service with LLM capabilities
const healerCounters = createHealerCounters();
const healerEvents: Array<{ type: string; data: unknown }> = [];

// Adapter: Wrap runClaudeCodeSubagent for Healer
const claudeCodeInvoker = (subtask: any, opts: any) => {
  return runClaudeCodeSubagent(subtask, {
    ...opts,
    cwd: config.workDir,
    openagentsDir,
  });
};

// Adapter: Wrap verification for Healer
const verificationRunner = (commands: string[], cwd: string) => {
  return Effect.runPromise(
    runVerification(commands, cwd, (event) => {
      // Forward to HUD
    }, sandboxRunnerConfig).pipe(
      Effect.catchAll(() => Effect.succeed({ passed: false, outputs: [] }))
    )
  );
};

// Create full Healer service (not Basic)
const healerService = createHealerService({
  claudeCodeInvoker,
  verificationRunner,
  onOutput: config.onOutput,
  signal: config.signal,
  openagentsDir,
});
```

---

### Step 3: Pass Healer to Orchestrator

**File:** `src/agent/do-one-task.ts` (around line 895)

```typescript
const finalState = yield* runOrchestrator(
  {
    cwd: config.workDir,
    openagentsDir,
    testCommands: projectConfig.testCommands ?? [],
    allowPush: projectConfig.allowPush ?? false,
    skipInitScript: config.skipInit,
    safeMode: false, // Deprecated, use healer instead
    claudeCode: {
      enabled: claudeCodeConfig.enabled,
      preferForComplexTasks: claudeCodeConfig.preferForComplexTasks ?? false,
      fallbackToMinimal: claudeCodeConfig.fallbackToMinimal ?? false,
    },

    // NEW: Pass Healer service
    healerService,
    healerCounters,
    projectConfig,
  },
  emit,
  {
    pickNextTask: (tasksPath: string) => pickNextTask(tasksPath, config.taskId),
    runClaudeCodeSubagent: (subtask, opts) => runClaudeCodeSubagent(subtask, opts),
    runMinimalSubagent: (subtask, opts) => runMinimalSubagent(subtask, opts),
    runVerification: (commands, cwd, emitVerif) =>
      runVerification(commands, cwd, emitVerif, sandboxRunnerConfig),
  }
);
```

---

### Step 4: Replace Safe-Mode with Healer

**File:** `src/agent/orchestrator/orchestrator.ts`

**Imports to add (top of file):**

```typescript
import type { HealerOutcome, HealerCounters } from "../healer/index.js";
import type { ProjectConfig } from "../../tasks/schema.js";
```

**REPLACE lines 280-370** (old safe-mode) with:

```typescript
// Healer: Attempt self-healing for recoverable failures
if (
  initScriptResult.ran &&
  !initScriptResult.success &&
  initScriptResult.canSelfHeal &&
  config.healerService &&
  config.healerCounters &&
  config.projectConfig
) {
  const ts = new Date().toISOString();
  emit({
    type: "error",
    phase: "orienting",
    error: `Init script failed with ${initScriptResult.failureType}, invoking Healer...`,
  });

  // Build event for Healer
  const healerEvent: OrchestratorEvent = {
    type: "init_script_complete",
    result: initScriptResult,
  };

  // ✅ SYNCHRONOUS Healer invocation with yield*
  const healerOutcome = yield* config.healerService.maybeRun(
    healerEvent,
    state,
    config.projectConfig,
    config.healerCounters
  ).pipe(
    Effect.catchAll((error) => {
      emit({
        type: "error",
        phase: "orienting",
        error: `Healer invocation failed: ${error.message}`,
      });
      return Effect.succeed(null);
    })
  );

  if (healerOutcome) {
    console.log(`[${ts}] [HEALER] Outcome:`, healerOutcome.status);
    console.log(`[${ts}] [HEALER] Spells tried:`, healerOutcome.spellsTried.join(", "));
    console.log(`[${ts}] [HEALER] Summary:`, healerOutcome.summary);

    // Handle different outcome statuses
    if (healerOutcome.status === "resolved") {
      // Healer fixed the issue - re-run init script to verify
      emit({
        type: "error",
        phase: "orienting",
        error: `Healer claims to have resolved ${initScriptResult.failureType}, verifying...`,
      });

      const retryResult = yield* runInitScript(openagentsDir, config.cwd, emit);

      if (retryResult.success) {
        // Success! Update state and continue
        initScriptResult = retryResult;
        progress.orientation.initScript = retryResult;
        emit({
          type: "error",
          phase: "orienting",
          error: `Healer successfully resolved ${initScriptResult.failureType}`,
        });
      } else {
        emit({
          type: "error",
          phase: "orienting",
          error: `Healer ran but init script still fails`,
        });
      }
    } else if (healerOutcome.status === "contained") {
      // Healer contained the issue (e.g., marked task blocked)
      emit({
        type: "error",
        phase: "orienting",
        error: `Healer contained the issue: ${healerOutcome.summary}`,
      });
    } else {
      // unresolved, failed, or skipped - fall through to normal failure
      emit({
        type: "error",
        phase: "orienting",
        error: `Healer could not resolve: ${healerOutcome.summary}`,
      });
    }
  }
}
```

**Keep existing failure check (lines 373-402) UNCHANGED:**

```typescript
// Check if init script still failed after potential Healer intervention
if (initScriptResult.ran && !initScriptResult.success) {
  state.phase = "failed";
  const failureInfo = initScriptResult.failureType
    ? ` (${initScriptResult.failureType}${initScriptResult.canSelfHeal ? ", self-heal attempted" : ""})`
    : "";
  state.error = `Init script failed${failureInfo}`;
  // ... abort logic ...
  return state;
}
```

---

### Step 5: Remove Broken Async Integration

**File:** `src/agent/do-one-task.ts`

**DELETE lines 860-882** (the broken async callback):

```typescript
// DELETE THIS ENTIRE BLOCK
const emit = (event: OrchestratorEvent) => {
  logOrchestratorEvent(event);
  hudEmit(event);

  // ... DEBUG logs ...

  // Invoke Healer for self-healing on errors
  if (currentState && "healer" in projectConfig && projectConfig.healer?.enabled !== false) {
    Effect.runPromise(
      healerService.maybeRun(event, currentState, projectConfig as any, healerCounters).pipe(
        Effect.provide(BunContext.layer)
      )
    ).then((outcome: HealerOutcome | null) => {
      // ... outcome handling ...
    }).catch((err) => {
      // ... error handling ...
    });
  }
};
```

**REPLACE with simple emit:**

```typescript
const emit = (event: OrchestratorEvent) => {
  logOrchestratorEvent(event);
  hudEmit(event);
};
```

---

### Step 6: Remove Debug Logging

**File:** `src/agent/do-one-task.ts`

**DELETE debug console.logs (lines 756-759):**

```typescript
// DELETE THESE
console.log("[DEBUG] projectConfig keys:", Object.keys(projectConfig));
console.log("[DEBUG] projectConfig.healer:", projectConfig.healer);
console.log("[DEBUG] 'healer' in projectConfig:", "healer" in projectConfig);
```

**File:** `src/healer/service.ts`

**DELETE debug console.logs:**

```typescript
// DELETE THESE
console.log("[DEBUG] shouldRunHealer decision:", decision);
console.log("[DEBUG] Healer will run for scenario:", scenario);
console.log("[DEBUG] Building Healer context...");
console.log("[DEBUG] buildHealerContext ERROR:", error);
console.log("[DEBUG] Healer context built:", Object.keys(ctx));
```

---

## Testing Strategy

### Unit Tests

**File:** `src/agent/orchestrator/__tests__/orchestrator.healer.test.ts` (NEW)

```typescript
test("init script typecheck failure triggers Healer", async () => {
  // Mock Healer service
  const healerMock = {
    maybeRun: vi.fn().mockResolvedValue({
      status: "resolved",
      scenario: "InitScriptTypecheckFailure",
      spellsTried: ["fix_typecheck_errors"],
      spellsSucceeded: ["fix_typecheck_errors"],
      summary: "Fixed 2 typecheck errors",
    }),
  };

  // Run orchestrator with failing init script
  const state = await runOrchestrator(
    { healerService: healerMock, ... },
    emit,
    runners
  );

  // Verify Healer was invoked
  expect(healerMock.maybeRun).toHaveBeenCalledWith(
    expect.objectContaining({ type: "init_script_complete" }),
    expect.any(Object), // state
    expect.any(Object), // config
    expect.any(Object)  // counters
  );

  // Verify session continued (not failed)
  expect(state.phase).not.toBe("failed");
});

test("init script failure with Healer disabled falls back to old behavior", async () => {
  // No Healer service provided
  const state = await runOrchestrator(
    { healerService: undefined, ... },
    emit,
    runners
  );

  // Verify session failed as expected
  expect(state.phase).toBe("failed");
  expect(state.error).toContain("Init script failed");
});
```

### Integration Test

**File:** `src/healer/__tests__/healer.e2e.test.ts` (already exists)

Verify end-to-end with real Healer:

```typescript
test("Healer fixes real typecheck error in orchestrator flow", async () => {
  // Create test repo with broken file
  const testDir = createTestRepo();
  writeFile(`${testDir}/broken.ts`, 'const x: number = "string";');

  // Run orchestrator with real Healer
  const state = await runOrchestrator(
    {
      cwd: testDir,
      healerService: createHealerService({ ... }),
      healerCounters: createHealerCounters(),
      ...
    },
    emit,
    runners
  );

  // Verify Healer fixed the error
  expect(state.phase).not.toBe("failed");

  // Verify file was fixed
  const fixed = readFile(`${testDir}/broken.ts`);
  expect(fixed).not.toContain('const x: number = "string"');
});
```

### Manual Test

```bash
# 1. Create test file with typecheck error
echo 'const count: number = "wrong";' > src/healer-test-trigger.ts

# 2. Run orchestrator
bun run mechacoder --verbose --cc-only

# Expected output:
# [timestamp] Init script failed with typecheck_failed, invoking Healer...
# [timestamp] [HEALER] Outcome: resolved
# [timestamp] [HEALER] Spells tried: fix_typecheck_errors
# [timestamp] Healer successfully resolved typecheck_failed
# [timestamp] Session SUCCESS: Completed task ...

# 3. Verify file was fixed
cat src/healer-test-trigger.ts
# Should show corrected code
```

---

## Files to Modify

### Primary Changes

| File | Lines | Change Type | Description |
|------|-------|-------------|-------------|
| `src/agent/orchestrator/types.ts` | ~172 | ADD | Add `healerService`, `healerCounters`, `projectConfig` to OrchestratorConfig |
| `src/agent/orchestrator/orchestrator.ts` | ~1-10 | ADD | Import Healer types |
| `src/agent/orchestrator/orchestrator.ts` | 280-370 | REPLACE | Replace safe-mode with Healer invocation |
| `src/agent/do-one-task.ts` | ~770-790 | ADD | Create full Healer service with adapters |
| `src/agent/do-one-task.ts` | ~895-908 | MODIFY | Pass Healer to orchestrator config |
| `src/agent/do-one-task.ts` | 860-882 | DELETE | Remove broken async callback |
| `src/agent/do-one-task.ts` | 756-759 | DELETE | Remove debug logging |
| `src/healer/service.ts` | various | DELETE | Remove debug console.logs |

### Test Files

| File | Change Type | Description |
|------|-------------|-------------|
| `src/agent/orchestrator/__tests__/orchestrator.healer.test.ts` | CREATE | Unit tests for Healer integration |
| `src/healer/__tests__/healer.e2e.test.ts` | MODIFY | Add orchestrator flow test |

---

## Success Criteria

1. ✅ Init script typecheck failures invoke Healer synchronously
2. ✅ Orchestrator waits for Healer outcome before deciding to fail
3. ✅ Successful healing → session continues
4. ✅ Failed healing → session aborts with clear message
5. ✅ All 168 Healer tests still pass
6. ✅ All orchestrator tests still pass
7. ✅ Manual test with real typecheck error succeeds
8. ✅ Healer logging shows in output
9. ✅ No BunContext errors
10. ✅ Backward compatible (works without Healer)

---

## Rollback Plan

If integration causes issues:

1. **Immediate rollback:** Revert `orchestrator.ts` to old safe-mode pattern (lines 280-370)
2. **Keep Healer service:** Leave service creation in place for future retry
3. **Disable async callback:** Keep lines 860-882 deleted (they were broken anyway)
4. **Document issues:** Note what failed for next attempt

---

## Future Work: Phase 2 (Complete Integration)

After Phase 1 is stable, add Healer at remaining failure points:

### Subtask Failure Integration

**Location:** `orchestrator.ts:629+` (after subtask execution)

```typescript
if (!result.success && config.healerService) {
  const healerOutcome = yield* config.healerService.maybeRun(
    { type: "subtask_failed", subtask, error: result.error },
    state,
    config.projectConfig!,
    config.healerCounters!
  );

  if (healerOutcome?.status === "resolved") {
    // Reset failure count, exit for retry next cycle
    failures[subtask.id] = 0;
    // TODO: Restructure loop to retry immediately
  }
}
```

### Verification Failure Integration

**Location:** `orchestrator.ts:704+` (after verification)

```typescript
if (!verifyResult.passed && config.healerService) {
  const healerOutcome = yield* config.healerService.maybeRun(
    { type: "verification_complete", output: verifyResult.outputs.join("\n") },
    state,
    config.projectConfig!,
    config.healerCounters!
  );

  if (healerOutcome?.status === "resolved") {
    // Re-run verification
    const retryResult = yield* runVerification(...);
    if (retryResult.passed) {
      // Continue to commit phase
    }
  }
}
```

---

## Critical Files to Read Before Implementation

1. **src/agent/orchestrator/orchestrator.ts:280-370** - Old safe-mode pattern to replace
2. **src/agent/orchestrator/types.ts:172** - OrchestratorConfig to extend
3. **src/healer/service.ts:82-200** - Healer service API and patterns
4. **src/healer/spells/typecheck.ts** - Typecheck fix spell implementation
5. **src/agent/orchestrator/init-script.ts:10-75** - Failure type detection

---

## Task to Create

```bash
bun run tasks:create \
  --title "Fix Healer synchronous integration in orchestrator" \
  --type bug \
  --priority 0 \
  --labels "healer,orchestrator,p0" \
  --description "## Problem

Healer is fully implemented (168 tests passing) but integration is broken. Healer runs asynchronously AFTER orchestrator decides to fail, so it never gets a chance to act.

## Solution

Move Healer invocation from async emit() callback to synchronous Effect.gen block in orchestrator's orientation phase, using yield* pattern.

## Implementation

Phase 1: Init Script Integration
- Create full Healer service with ClaudeCodeInvoker
- Thread service to orchestrator via config
- Replace safe-mode (lines 280-370) with Healer
- Remove async callback (lines 860-882)

See: docs/healer/INTEGRATION-FIX.md (this plan)

## Success Criteria

- Init typecheck failures invoke Healer synchronously
- Orchestrator continues after successful healing
- All tests pass
- Manual test with typecheck error succeeds"
```
