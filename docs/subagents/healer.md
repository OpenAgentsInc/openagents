Healer fits *perfectly* with what you’ve already built: ATIF, orchestrator events, sessions, safe-mode, sandbox, HUD, etc. Let’s define it as a first-class, orchestrator-managed **self-healing subagent**.

Below is a concrete spec you can hand to MechaCoder / Claude Code as tasks.

---

# Healer Subagent Spec

> A system subagent that wakes up automatically when agent trajectories go off the rails, diagnoses what went wrong, and tries to repair or safely contain the damage.

---

## 1. Goals & Non-Goals

### Goals

1. **Automatic recovery on errors**

   * When init, verification, subtask, or runtime errors occur, Healer is invoked automatically (configurable).
   * Healer either **fixes the issue**, **rewinds** to a safe state, or **marks things clearly blocked** instead of leaving repo/tasks in a weird half-broken state.

2. **Trajectory-aware diagnosis**

   * Healer sees the **full ATIF trajectory** for the current run (plus related subagent trajectories) so it can reason about:

     * Which tool calls failed
     * Which tests failed and why
     * Whether this is a repeated pattern for this task/subtask

3. **Controlled “spell” execution**

   * Healer has a limited set of **“spells”** (safe, well-defined operations) to:

     * Rewind/git-reset
     * Restart with minimal subagent
     * Mark/reshape tasks & subtasks
     * Update `progress.md` / logs with guidance
   * Spells are **tools**, not arbitrary shell sequences.

4. **Never make things worse**

   * Respect Golden Loop invariants: **no “healing” commit** if tests are still failing.
   * Never push broken code.
   * Upper bound on Healer invocations per session/subtask to avoid infinite loops.

### Non-Goals (for v1)

* Healer is **not** a general “auto-tune my whole repo” wizard.
* Healer is **not** a long-running optimization agent; it’s focused on **recovery, containment, and guidance**.
* No cross-project global scheduler yet; v1 is scoped per project/session.

---

## 2. When Healer Runs (Triggers)

Healer hooks into **OrchestratorEvents** *and* ATIF.

### 2.1 Orchestrator triggers (Phase 1 – P1)

Healer runs synchronously inside the orchestrator loop under these conditions (configurable):

1. **Init script failure (preflight)**

   * `init_script_failed` / safe-mode path (already implemented in `oa-safe01`).
   * Scenario: `InitScriptTypecheckFailure`, `InitScriptTestFailure`, `InitScriptEnvironmentFailure`.

2. **Verification failure**

   * `verification_complete` with `passed: false`.
   * Scenario: `VerificationFailed` (tests or typecheck fail after subtask).

3. **Subtask failure**

   * `subtask_failed` with `error_max_turns`, repeated tool failures, or explicit error.
   * Scenario: `SubtaskFailed` / `SubtaskStuck`.

4. **Hard runtime error**

   * `error` event in orchestrator with severity `fatal` or `retriable` exceeding retry budget.
   * Scenario: `RuntimeError`.

### 2.2 “Stuck” detection (Phase 2 – P2)

Additional triggers once ATIF + sessions are live:

* **Stuck in-progress subtask**

  * Same subtask retried N times with similar failures.
  * ATIF + sessions show repeating failure messages.

* **Stale in_progress tasks**

  * Task left in `in_progress` for > X hours/days.
  * Ties in with `tasks:stale` / `tasks:doctor` (future).

These can either run:

* **Inline** in orchestrator when it detects the pattern, or
* Via a periodic CLI job: `bun run healer:scan` that inspects ATIF + tasks + sessions and fires Healer for long-running issues.

---

## 3. Inputs Healer Sees

For each invocation, Healer receives a **HealerContext**:

```ts
interface HealerContext {
  projectRoot: string;
  projectConfig: ProjectConfig;
  task?: Task;                 // current main task
  subtask?: Subtask;           // current subtask, if any
  sessionId: string;
  runId?: string;              // overnight / do-one-task run ID
  trajectory?: Trajectory;     // ATIF trajectory for this run or subagent
  relatedTrajectories: Trajectory[]; // parent/children trajectories
  sessions: SessionSummary[];  // recent sessions from SessionManager
  runLogs: RunLogSummary[];    // docs/logs + run-logs
  progressMd: string | null;
  gitStatus: GitStatus;        // output from git status, current branch, etc.
  heuristics: HealerHeuristics; // precomputed: failure type, repetition, etc.
}
```

Healer doesn’t have to fetch everything itself; **HealerService** builds this context using:

* `TrajectoryService` (`.openagents/trajectories/...`)
* `SessionManager` (~/.openagents/agent/sessions or project sessions)
* `Run-log` pipeline (`.openagents/run-logs/`)
* `progress.ts`
* `TaskService`
* `git` helper in orchestrator

---

## 4. Healer Spells

Spells are **typed tools** exposed to the Healer subagent. Think of them as very constrained “capabilities” it can chain together.

### 4.1 Spell catalog (v1)

1. **`spell.rewind_uncommitted_changes`**

   * Goal: restore repo to a clean state since last commit.
   * Impl: `git status`, if dirty then `git restore` / `git checkout .`, optionally `git clean -fd` (config-gated).

2. **`spell.rewind_to_last_green_commit`**

   * Goal: revert to last commit where tests passed.
   * Impl:

     * Use run logs / SessionManager to find last session with `testsPassed = true`.
     * Record that commit SHA in `progress.md`.
     * Optionally `git reset --hard <sha>` (config-gated / needing permission).

3. **`spell.mark_task_blocked_with_followup`**

   * Goal: stop thrashing; surface problem clearly.
   * Impl:

     * Update current Task status to `blocked`, with `closeReason`/`blockedReason` summarizing failure.
     * Create a new “healer follow-up” task with details + pointers to trajectories/sessions.
     * Cross-link via `deps`.

4. **`spell.retry_with_minimal_subagent`**

   * Goal: when Claude Code keeps failing, try the minimal subagent once.
   * Impl:

     * Update subtask metadata to indicate fallback attempted.
     * Call orchestrator’s `runBestAvailableSubagent` with override `forceMinimal`.

5. **`spell.retry_with_claude_code_resume`**

   * Goal: use CC’s session resume to try recovery once.
   * Impl:

     * Use session metadata (already stored in `SubagentResult.sessionMetadata`) to resume CC at previous session instead of starting new.

6. **`spell.fix_typecheck_errors`**

   * Goal: the existing safe-mode behavior generalized.
   * Impl:

     * Collect typecheck errors from init script or verification logs.
     * Spawn subagent with a constrained prompt: “Fix these type errors without changing behavior” + references.

7. **`spell.update_progress_with_guidance`**

   * Goal: leave a crisp explanation + next steps.
   * Impl:

     * Append a “Healer summary” section in `progress.md` with:

       * Summary of failure.
       * What Healer tried.
       * Recommended next manual steps if not fully resolved.

8. **`spell.run_tasks_doctor_like_checks` (stub)**

   * Goal: run structural checks on `.openagents/tasks.jsonl` and `.openagents/project.json`.
   * Impl:

     * Call TaskService doctor/validate once those commands exist (`oa-65d5fe` / `oa-a4eb48`).

You don’t need to build all spells on day one; Phase 1 can focus on:

* `rewind_uncommitted_changes`
* `fix_typecheck_errors`
* `mark_task_blocked_with_followup`
* `update_progress_with_guidance`

### 4.2 Spell interface

```ts
interface HealerSpellContext extends HealerContext {
  // plus helper functions: runBash, runTasksCli, writeProgress, etc.
}

interface HealerSpellResult {
  success: boolean;
  changesApplied?: boolean;
  summary: string;
}

type HealerSpellId =
  | "rewind_uncommitted_changes"
  | "rewind_to_last_green_commit"
  | "mark_task_blocked_with_followup"
  | "retry_with_minimal_subagent"
  | "retry_with_claude_code_resume"
  | "fix_typecheck_errors"
  | "update_progress_with_guidance"
  | "run_tasks_doctor_like_checks";

interface HealerSpell {
  id: HealerSpellId;
  description: string;
  apply: (ctx: HealerSpellContext) => Effect.Effect<HealerSpellResult, HealerError>;
}
```

These spells can be:

* Used directly from TS in **“Healer service mode”**, and/or
* Exposed as **LLM tools** for a “Healer subagent” (preferred so the model chooses which spells to invoke).

---

## 5. Healer Execution Flow

### 5.1 High-level state machine (per invocation)

1. **Detection**

   * Orchestrator emits an event that matches a trigger.
   * `HealerService.maybeRunHealer(trigger, sessionState)` checks policy:

     * Is Healer enabled?
     * Are we under `maxInvocationsPerSession` / `perSubtask`?
     * Is this scenario configured to auto-heal?

2. **Context assembly**

   * `buildHealerContext(trigger)` collects ATIF, sessions, run logs, git status, etc.

3. **Diagnosis & plan**

   * Either:

     * A simple rule-based planner picks a plan (list of spells) based on `HealerHeuristics`, or
     * A Healer subagent model sees a summary + list of spells as tools and chooses.

   Example heuristics:

   * If **init typecheck failure** → `fix_typecheck_errors` → `update_progress_with_guidance`.
   * If **verification failure** but tests flaked → possibly `retry_with_claude_code_resume` or mark blocked.
   * If **repo dirty + tests failing + no successful commit yet** → `rewind_uncommitted_changes`, then mark blocked.

4. **Spell execution**

   * Execute each spell sequentially, collecting results.

5. **Evaluation**

   * Re-run verification subset:

     * At least typecheck, optionally full `testCommands`.
   * If success:

     * Mark Healer outcome `resolved`.
   * If still failing but repo clean and task blocked:

     * Mark `contained`.
   * If still failing + repo dirty and no safe rewinds:

     * Mark `unresolved`.

6. **Outcome reporting**

   * Record a `HealerInvocation` object:

     ```ts
     interface HealerOutcome {
       scenario: HealerScenario;
       status: "resolved" | "contained" | "unresolved" | "skipped";
       spellsTried: HealerSpellId[];
       summary: string;
     }
     ```
   * Append to:

     * `progress.md` (Healer summary section).
     * Session logs / ATIF metrics.
   * Emit HUD messages.

7. **Control back to orchestrator**

   * If `resolved` → orchestrator may **continue** normal flow.
   * If `contained` or `unresolved` → orchestrator:

     * Stops further subtask work.
     * Leaves task blocked / to human.

---

## 6. Integration Points

### 6.1 Project config

Extend `ProjectConfig` (`src/tasks/project.ts`) with:

```ts
interface HealerConfig {
  enabled: boolean;                         // default true
  maxInvocationsPerSession: number;         // default 2
  maxInvocationsPerSubtask: number;         // default 1
  scenarios: {
    onInitFailure: boolean;                 // default true
    onVerificationFailure: boolean;         // default true
    onSubtaskFailure: boolean;              // default true
    onRuntimeError: boolean;                // default true
    onStuckSubtask: boolean;                // default false (Phase 2)
  };
  spells?: {
    allowed?: HealerSpellId[];              // optional whitelist
    forbidden?: HealerSpellId[];            // optional blacklist
  };
  mode: "conservative" | "aggressive";      // influences planner heuristics
}
```

Add to `ProjectConfig`:

```ts
healer?: HealerConfig;
```

### 6.2 Orchestrator

Files:

* `src/agent/orchestrator/orchestrator.ts`
* `src/agent/overnight.ts`
* `src/agent/do-one-task.ts`

Changes:

* Wire `HealerService` into the `emit` & error handling flow:

```ts
import { HealerService } from "../healer/service.js";

const emit = (event: OrchestratorEvent) => {
  logOrchestratorEvent(event);
  hudClient.send(mapOrchestratorEvent(event));
  HealerService.handleEvent(event, sessionState).catch(logError);
};
```

* At key points (init failure, verification failure, subtask failure), call:

```ts
const healerOutcome = await HealerService.maybeRun(
  { scenario: "VerificationFailed", event, sessionState },
  projectConfig
);

// Outcome decides whether we continue, stop, or mark blocked.
```

### 6.3 ATIF

Use existing open ATIF tasks:

* `oa-668316` – schema
* `oa-6d4f43` – validation
* `oa-349d5e` – collector
* etc.

Healer integration:

* Each Healer invocation is either:

  * A **subagent ATIF trajectory** with `agent.kind = "healer"`, or
  * A **segment** in existing orchestrator trajectory with a distinct “phase”.

Add helper in `src/atif/adapter.ts`:

```ts
function makeHealerAgent(version: string): Agent { ... }
function makeHealerStepFromInvocation(invocation: HealerInvocation): Step { ... }
```

This allows you to:

* Link Healer trajectories via `subagent_trajectory_ref` from the failing subtask.

### 6.4 HUD

Extend `HudMessage` union (`src/hud/protocol.ts`) with Healer events:

```ts
| { type: "healer_invocation_start"; scenario: string; sessionId: string; ts: string }
| { type: "healer_invocation_complete"; scenario: string; status: "resolved" | "contained" | "unresolved"; ts: string }
| { type: "healer_spell_applied"; spell: HealerSpellId; success: boolean; ts: string }
```

Map via `mapOrchestratorEvent` or new mapping helpers in `src/hud/emit.ts`.

Electrobun HUD can then show:

* A “Healer” node branching off from the failing node in the flow graph.
* Live updates like “Healer: rewinding uncommitted changes…” etc.

---

## 7. Module Structure

Create a new package namespace:

```text
src/healer/
├── types.ts          # HealerConfig, HealerScenario, HealerOutcome, HealerContext
├── policy.ts         # HealerPolicy: when to run, limits, scenario mapping
├── context.ts        # buildHealerContext(trigger) using ATIF, sessions, logs, git
├── spells.ts         # Spell definitions and implementations
├── planner.ts        # Given heuristics + config, choose spell plan(s)
├── service.ts        # HealerService.maybeRun()/handleEvent()
├── atif.ts           # Helpers for ATIF-based analysis (last error, patterns)
└── __tests__/...     # Unit tests for policy, spells, planner, service
```

---

## 8. Implementation Phases

### Phase H1 – Core Healer infrastructure (P1)

1. **Healer types & config**

   * `src/healer/types.ts`
   * `healer` section in `ProjectConfig`.

2. **Healer policy + event hook**

   * `policy.ts` + `service.ts::handleEvent` to track per-session/subtask invocation counters.

3. **Context builder (minimal)**

   * `context.ts` with:

     * `progress.md`
     * current Task/Subtask
     * basic ATIF data (if available)
     * git status
     * recent run logs

4. **Minimal spell set**

   * `rewind_uncommitted_changes`
   * `fix_typecheck_errors` (wrap existing safe-mode logic)
   * `mark_task_blocked_with_followup`
   * `update_progress_with_guidance`

5. **Wire into orchestrator**

   * Run Healer on:

     * init failure
     * verification failure
     * subtask failure

### Phase H2 – ATIF + subagent integration (P1/P2)

1. **ATIF helpers**

   * `healer/atif.ts` uses TrajectoryService to find:

     * last failing step
     * pattern of repeated tool failures
     * which subagent produced the failure.

2. **Healer as subagent**

   * `healer-subagent.ts`: define a minimal Healer prompt and map spells as tools (via existing tool schema + provider abstractions).
   * Use `runBestAvailableSubagent` under a “healer” mode with constrained tools.

3. **HUD integration**

   * Healer events -> HUD messages.
   * Visualize in flow HUD.

### Phase H3 – Advanced heuristics & stuck detection (P2)

1. **Stuck subtask detection**

   * Look across ATIF trajectories + sessions to detect repeated failure patterns.
   * Trigger Healer in background or at next run start.

2. **Cross-session healing**

   * Healer can operate at beginning of an overnight run to clean up dirty state left by previous runs (e.g., run `rewind` + `mark_blocked` before new work).

3. **More spells**

   * `rewind_to_last_green_commit`
   * `run_tasks_doctor_like_checks`
   * Additional repo-level spells as needed.

---

## 9. Tasks to Add to `.openagents/tasks.jsonl`

You can mint them however you like, but something like:

1. **oa-healer-01 – Design & scaffold Healer module**

   * Create `src/healer/*` skeleton + docs stub `docs/healer.md`.

2. **oa-healer-02 – Add HealerConfig to ProjectConfig**

   * Extend schema + defaults, update tests.

3. **oa-healer-03 – Implement Healer policy & event hook**

   * `HealerPolicy`, per-session/per-subtask limits, integrate with orchestrator events (no spells yet).

4. **oa-healer-04 – Implement core spells (rewind/mark_blocked/progress)**

   * `rewind_uncommitted_changes`, `mark_task_blocked_with_followup`, `update_progress_with_guidance`.

5. **oa-healer-05 – Generalize safe-mode typecheck fixer into `spell.fix_typecheck_errors`**

   * Refactor existing safe-mode logic into a spell and route via Healer.

6. **oa-healer-06 – Wire Healer into init / verification / subtask failure paths**

   * Orchestrator + overnight/do-one-task integration.

7. **oa-healer-07 – Integrate Healer with ATIF trajectories**

   * Use TrajectoryService; record Healer invocations and link via `subagent_trajectory_ref`.

8. **oa-healer-08 – Add HUD Healer events + basic visualization**

   * HUD protocol + Electrobun view updates.

9. **oa-healer-09 – Stuck task/subtask detection (v1)**

   * Simple heuristics based on repeated failures + stale in_progress tasks; run Healer accordingly.

10. **oa-healer-10 – E2E tests for Healer scenarios**

    * Fixtures for: init typecheck failure, verification failure, subtask failure; assert Healer leaves repo in safe state and tasks correctly marked.

---

If you want, next step could be: I write the concrete HealerConfig schema + a `HealerService` signature + a minimal “InitScriptFailure” flow you can drop straight into `orchestrator.ts`.
