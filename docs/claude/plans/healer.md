# Healer Subagent Implementation Plan

A self-healing subagent that wakes up automatically when agent trajectories go off the rails, diagnoses what went wrong, and tries to repair or safely contain the damage.

## Summary

- **Scope**: Full Healer subagent implementation with spells, ATIF integration, and HUD events
- **Triggers**: Init script failure, verification failure, subtask failure, runtime errors
- **Spells**: Controlled recovery operations (rewind, fix typecheck, mark blocked, update progress)
- **Migration**: **Replaces existing safe-mode** in orchestrator.ts (not parallel opt-in)
- **Tasks**: Create all 10 tasks upfront (Phases H1, H2, H3)

---

## Module Structure

```
src/healer/
├── types.ts          # HealerConfig, HealerScenario, HealerOutcome, HealerContext
├── policy.ts         # HealerPolicy: when to run, limits, scenario mapping
├── context.ts        # buildHealerContext() using ATIF, sessions, logs, git
├── spells/
│   ├── index.ts      # Spell registry and execution
│   ├── rewind.ts     # rewind_uncommitted_changes
│   ├── typecheck.ts  # fix_typecheck_errors (from safe-mode)
│   ├── blocked.ts    # mark_task_blocked_with_followup
│   └── progress.ts   # update_progress_with_guidance
├── planner.ts        # Given heuristics + config, choose spell plan(s)
├── service.ts        # HealerService.maybeRun()/handleEvent()
├── atif.ts           # createHealerAgent(), ATIF helpers
└── __tests__/
    ├── policy.test.ts
    ├── spells.test.ts
    ├── planner.test.ts
    └── service.test.ts
```

---

## Phase H1: Core Infrastructure (P1)

### Task 1: Healer Types & Config Schema

**File: `src/healer/types.ts`**

```typescript
// Healer scenarios that trigger invocation
type HealerScenario =
  | "InitScriptTypecheckFailure"
  | "InitScriptTestFailure"
  | "InitScriptEnvironmentFailure"
  | "VerificationFailed"
  | "SubtaskFailed"
  | "SubtaskStuck"
  | "RuntimeError";

// Spell identifiers
type HealerSpellId =
  | "rewind_uncommitted_changes"
  | "rewind_to_last_green_commit"
  | "mark_task_blocked_with_followup"
  | "retry_with_minimal_subagent"
  | "retry_with_claude_code_resume"
  | "fix_typecheck_errors"
  | "update_progress_with_guidance"
  | "run_tasks_doctor_like_checks";

// Spell result
interface HealerSpellResult {
  success: boolean;
  changesApplied?: boolean;
  summary: string;
}

// Overall outcome
interface HealerOutcome {
  scenario: HealerScenario;
  status: "resolved" | "contained" | "unresolved" | "skipped";
  spellsTried: HealerSpellId[];
  summary: string;
}

// Context passed to Healer
interface HealerContext {
  projectRoot: string;
  projectConfig: ProjectConfig;
  task?: Task;
  subtask?: Subtask;
  sessionId: string;
  runId?: string;
  trajectory?: Trajectory;
  relatedTrajectories: Trajectory[];
  progressMd: string | null;
  gitStatus: GitStatus;
  heuristics: HealerHeuristics;
}
```

**File: `src/tasks/schema.ts` (extend ProjectConfig)**

```typescript
const HealerConfig = S.Struct({
  enabled: S.optionalWith(S.Boolean, { default: () => true }),
  maxInvocationsPerSession: S.optionalWith(S.Number, { default: () => 2 }),
  maxInvocationsPerSubtask: S.optionalWith(S.Number, { default: () => 1 }),
  scenarios: S.optionalWith(S.Struct({
    onInitFailure: S.optionalWith(S.Boolean, { default: () => true }),
    onVerificationFailure: S.optionalWith(S.Boolean, { default: () => true }),
    onSubtaskFailure: S.optionalWith(S.Boolean, { default: () => true }),
    onRuntimeError: S.optionalWith(S.Boolean, { default: () => true }),
    onStuckSubtask: S.optionalWith(S.Boolean, { default: () => false }),
  }), { default: () => ({}) }),
  spells: S.optionalWith(S.Struct({
    allowed: S.optionalWith(S.Array(S.String), { default: () => [] }),
    forbidden: S.optionalWith(S.Array(S.String), { default: () => [] }),
  }), { default: () => ({}) }),
  mode: S.optionalWith(S.Literal("conservative", "aggressive"), { default: () => "conservative" }),
});
```

---

### Task 2: Healer Policy & Event Hook

**File: `src/healer/policy.ts`**

- Track per-session/per-subtask invocation counters
- Decide if Healer should run based on config and limits
- Map OrchestratorEvents to HealerScenarios

```typescript
export const shouldRunHealer = (
  event: OrchestratorEvent,
  config: HealerConfig,
  counters: HealerCounters
): { run: boolean; scenario?: HealerScenario; reason?: string };

export const mapEventToScenario = (
  event: OrchestratorEvent
): HealerScenario | null;
```

---

### Task 3: Context Builder

**File: `src/healer/context.ts`**

Build HealerContext from available data sources:
- Read `progress.md` for session state
- Get current Task/Subtask from orchestrator state
- Load ATIF trajectory if available
- Capture git status
- Compute heuristics (failure patterns, repetition count)

```typescript
export const buildHealerContext = (
  trigger: { scenario: HealerScenario; event: OrchestratorEvent },
  state: OrchestratorState,
  config: ProjectConfig
): Effect.Effect<HealerContext, Error, FileSystem>;
```

---

### Task 4: Core Spells (Phase 1 Set)

**File: `src/healer/spells/rewind.ts`**

```typescript
export const rewindUncommittedChanges: HealerSpell = {
  id: "rewind_uncommitted_changes",
  description: "Restore repo to clean state since last commit",
  apply: (ctx) => Effect.gen(function* () {
    // git status → if dirty → git restore . + git clean -fd
    // Return success/failure summary
  }),
};
```

**File: `src/healer/spells/typecheck.ts`**

Refactor existing safe-mode logic from orchestrator.ts:280-370:
- Create emergency subtask with typecheck fix instructions
- Invoke Claude Code subagent
- Re-run verification
- Return result

**File: `src/healer/spells/blocked.ts`**

```typescript
export const markTaskBlockedWithFollowup: HealerSpell = {
  id: "mark_task_blocked_with_followup",
  description: "Stop thrashing; surface problem clearly",
  apply: (ctx) => Effect.gen(function* () {
    // Update Task status to "blocked" with reason
    // Create follow-up task with details + trajectory pointers
    // Cross-link via deps
  }),
};
```

**File: `src/healer/spells/progress.ts`**

```typescript
export const updateProgressWithGuidance: HealerSpell = {
  id: "update_progress_with_guidance",
  description: "Leave crisp explanation + next steps in progress.md",
  apply: (ctx) => Effect.gen(function* () {
    // Append "Healer Summary" section with:
    // - Failure summary
    // - What Healer tried
    // - Recommended next steps
  }),
};
```

---

### Task 5: Spell Planner

**File: `src/healer/planner.ts`**

Rule-based planner that selects spells based on scenario and heuristics:

```typescript
export const planSpells = (
  context: HealerContext,
  config: HealerConfig
): HealerSpellId[] => {
  switch (context.heuristics.scenario) {
    case "InitScriptTypecheckFailure":
      return ["fix_typecheck_errors", "update_progress_with_guidance"];

    case "VerificationFailed":
      if (context.heuristics.isFlaky) {
        return ["retry_with_claude_code_resume"];
      }
      return ["rewind_uncommitted_changes", "mark_task_blocked_with_followup"];

    case "SubtaskFailed":
      if (context.heuristics.failureCount >= 3) {
        return ["mark_task_blocked_with_followup", "update_progress_with_guidance"];
      }
      return ["fix_typecheck_errors"];

    // ... other scenarios
  }
};
```

---

### Task 6: Healer Service

**File: `src/healer/service.ts`**

Main entry point for orchestrator integration:

```typescript
export const HealerService = {
  maybeRun: (
    trigger: { scenario: HealerScenario; event: OrchestratorEvent; state: OrchestratorState },
    config: ProjectConfig
  ): Effect.Effect<HealerOutcome | null, Error, ...> => {
    // 1. Check policy (enabled, limits)
    // 2. Build context
    // 3. Plan spells
    // 4. Execute spells sequentially
    // 5. Re-verify if needed
    // 6. Return outcome
  },

  handleEvent: (
    event: OrchestratorEvent,
    state: OrchestratorState
  ): void => {
    // Lightweight hook for event-driven triggering
    // Calls maybeRun internally
  },
};
```

---

### Task 7: Wire into Orchestrator

**File: `src/agent/orchestrator/orchestrator.ts`**

Integration points (3 locations):

**A. After init script failure (replace existing safe-mode, ~line 280):**
```typescript
if (initScriptResult.ran && !initScriptResult.success && config.healer?.enabled) {
  const healerOutcome = yield* HealerService.maybeRun(
    { scenario: mapInitFailure(initScriptResult), event, state },
    config
  );
  if (healerOutcome?.status === "resolved") {
    // Re-run init script to verify
    initScriptResult = yield* runInitScript(...);
  }
}
```

**B. After subtask failure (~line 629):**
```typescript
if (!result.success && config.healer?.enabled) {
  const healerOutcome = yield* HealerService.maybeRun(
    { scenario: "SubtaskFailed", event, state },
    config
  );
  // Handle outcome...
}
```

**C. After verification failure (~line 704):**
```typescript
if (!verifyResult.passed && config.healer?.enabled) {
  const healerOutcome = yield* HealerService.maybeRun(
    { scenario: "VerificationFailed", event, state },
    config
  );
  // Handle outcome...
}
```

---

## Phase H2: ATIF & Subagent Integration (P1/P2)

### Task 8: ATIF Helpers for Healer

**File: `src/healer/atif.ts`**

```typescript
import { StandaloneTrajectoryCollector } from "../atif/index.js";

export const createHealerAgent = (modelName: string): Agent => ({
  name: "healer",
  version: "1.0.0",
  model_name: modelName,
  extra: { type: "healing-agent" },
});

export const createHealerTrajectory = (
  parentSessionId: string,
  scenario: HealerScenario
): StandaloneTrajectoryCollector => {
  const collector = new StandaloneTrajectoryCollector();
  collector.startTrajectory({
    agent: createHealerAgent("grok-4.1-fast"),
    parentSessionId,
  });
  return collector;
};
```

**File: `src/atif/adapter.ts` (add Healer agent factory)**

```typescript
export const createHealerAgent = (
  modelName: string,
  version = "1.0.0",
): Agent => ({
  name: "healer",
  version,
  model_name: modelName,
  extra: { type: "healing-agent" },
});
```

---

### Task 9: HUD Events for Healer

**File: `src/hud/protocol.ts` (extend HudMessage union)**

```typescript
interface HealerInvocationStartMessage {
  type: "healer_invocation_start";
  scenario: HealerScenario;
  sessionId: string;
  ts: string;
}

interface HealerInvocationCompleteMessage {
  type: "healer_invocation_complete";
  scenario: HealerScenario;
  status: "resolved" | "contained" | "unresolved";
  spellsTried: string[];
  ts: string;
}

interface HealerSpellAppliedMessage {
  type: "healer_spell_applied";
  spell: HealerSpellId;
  success: boolean;
  summary: string;
  ts: string;
}
```

---

## Phase H3: Advanced Features (P2)

### Task 10: Stuck Detection

- Look across ATIF trajectories + sessions for repeated failure patterns
- Trigger Healer for in_progress tasks stuck > N hours
- CLI command: `bun run healer:scan`

### Task 11: Additional Spells

- `rewind_to_last_green_commit` - Find last passing commit via run logs
- `retry_with_minimal_subagent` - Fallback when Claude Code fails
- `retry_with_claude_code_resume` - Session resume for recovery
- `run_tasks_doctor_like_checks` - Structural validation of tasks.jsonl

### Task 12: E2E Tests

**File: `src/healer/__tests__/healer.e2e.test.ts`**

Test scenarios:
1. Init typecheck failure → Healer fixes → continue
2. Verification failure → Healer rewinds → marks blocked
3. Subtask failure × 3 → Healer creates follow-up task
4. Healer respects invocation limits

---

## Files to Create

| File | Purpose | Lines (est) |
|------|---------|-------------|
| `src/healer/types.ts` | Type definitions | ~100 |
| `src/healer/policy.ts` | Policy and event mapping | ~80 |
| `src/healer/context.ts` | Context builder | ~120 |
| `src/healer/spells/index.ts` | Spell registry | ~50 |
| `src/healer/spells/rewind.ts` | Rewind spell | ~60 |
| `src/healer/spells/typecheck.ts` | Typecheck fix spell | ~150 |
| `src/healer/spells/blocked.ts` | Block task spell | ~80 |
| `src/healer/spells/progress.ts` | Progress update spell | ~60 |
| `src/healer/planner.ts` | Spell planner | ~100 |
| `src/healer/service.ts` | Main service | ~200 |
| `src/healer/atif.ts` | ATIF helpers | ~60 |
| `src/healer/__tests__/*.test.ts` | Tests (4 files) | ~400 |

## Files to Modify

| File | Changes |
|------|---------|
| `src/tasks/schema.ts` | Add HealerConfig to ProjectConfig |
| `src/agent/orchestrator/orchestrator.ts` | Wire Healer at 3 integration points |
| `src/agent/orchestrator/types.ts` | Add Healer-related event types (optional) |
| `src/hud/protocol.ts` | Add Healer HUD message types |
| `src/atif/adapter.ts` | Add createHealerAgent factory |

---

## Critical Files to Read Before Implementation

1. `src/agent/orchestrator/orchestrator.ts:280-370` - Existing safe-mode pattern
2. `src/agent/orchestrator/orchestrator.ts:619-685` - Subtask failure handling
3. `src/agent/orchestrator/init-script.ts:10-75` - detectFailureType()
4. `src/agent/orchestrator/types.ts` - OrchestratorEvent, InitScriptResult
5. `src/atif/integration.ts` - ATIF integration patterns
6. `src/agent/orchestrator/progress.ts` - Progress file management

---

## Tasks to Create in .openagents/tasks.jsonl

### Phase H1 (P1 - Core)

1. **oa-healer-01** - Design & scaffold Healer module
   - Create `src/healer/*` skeleton
   - Add types.ts with all interfaces

2. **oa-healer-02** - Add HealerConfig to ProjectConfig
   - Extend schema in `src/tasks/schema.ts`
   - Add defaults and validation

3. **oa-healer-03** - Implement Healer policy & event hook
   - Create policy.ts
   - Map OrchestratorEvents to scenarios
   - Track invocation limits

4. **oa-healer-04** - Implement core spells
   - rewind_uncommitted_changes
   - mark_task_blocked_with_followup
   - update_progress_with_guidance

5. **oa-healer-05** - Generalize safe-mode into fix_typecheck_errors spell
   - Refactor orchestrator.ts:280-370 into spell
   - Route through Healer service

6. **oa-healer-06** - Wire Healer into orchestrator
   - Init failure integration
   - Subtask failure integration
   - Verification failure integration

### Phase H2 (P1/P2 - ATIF + HUD)

7. **oa-healer-07** - Integrate Healer with ATIF
   - Add createHealerAgent to adapter.ts
   - Record Healer invocations as trajectories
   - Link via subagent_trajectory_ref

8. **oa-healer-08** - Add HUD Healer events
   - Extend protocol.ts with Healer message types
   - Emit events during Healer execution

### Phase H3 (P2 - Advanced)

9. **oa-healer-09** - Stuck task/subtask detection
   - Implement heuristics for repeated failures
   - CLI command for scanning

10. **oa-healer-10** - E2E tests for Healer scenarios
    - Test all trigger scenarios
    - Verify repo remains in safe state

---

## Dependency Graph

```
oa-healer-01 (scaffold)
    └── oa-healer-02 (config)
        └── oa-healer-03 (policy)
            └── oa-healer-04 (core spells)
                ├── oa-healer-05 (typecheck spell)
                └── oa-healer-06 (wire into orchestrator)
                    ├── oa-healer-07 (ATIF) [parallel]
                    └── oa-healer-08 (HUD) [parallel]
                        └── oa-healer-09 (stuck detection)
                            └── oa-healer-10 (E2E tests)
```
