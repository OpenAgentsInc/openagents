



Here’s a spec you can hand straight to MechaCoder / Claude Code.

---

# Spec: FM-Driven Task Selection & Planning

## Goal

Enable the **FM-driven agent loop** to:

1. **Pick the next task to work on** from `.openagents/tasks.jsonl`,

   * respecting the existing task system and dependencies,
   * **optionally weighting** user/agent priorities.

2. **If no acceptable task is available** (or only low-priority tasks and FM decides to plan instead),

   * run a **“plan session”** that:

     * proposes new work,
     * **optionally weights** user/agent priorities,
     * and can create new tasks via TaskService.

The implementation must use the existing `.openagents` task system and **must not** manually edit `tasks.jsonl`.

---

## Scope & Non-Goals

**In scope:**

* New **TaskSelection** utility that:

  * lists ready tasks,
  * applies deterministic sorting,
  * exposes a compact “candidate set” to FM.

* New **FM selection step** that:

  * lets FM choose between:

    * `work_on_task(taskId)`
    * `run_plan_session(reason)`
  * uses structured JSON output.

* New **PlanSession** flow that:

  * uses FM to produce a plan (tasks/epics),
  * optionally creates `.openagents` tasks via TaskService.

**Not in scope (for this change):**

* Changing the underlying Task schema.
* Changing existing `tasks:next` CLI behavior.
* Integrating with parallel worktrees / overnight (this should be orchestrator-level but not required to be wired into `overnight` in the first iteration).

---

## Existing Constraints & Conventions

You must respect the following rules:

* **Single task system**: `.openagents/` is the **only** task system in this repo.

  * Use `TaskService` / `ProjectService` in `src/tasks/`.
  * **Never** manually edit `.openagents/tasks.jsonl`.

* **Task schema:**

  * `status`: `open | in_progress | blocked | closed`
  * `priority`: `0..4` (0=P0 critical, 4=backlog)
  * `type`: `bug | feature | task | epic | chore`

* **Task readiness**:

  * “Ready” means:

    * `status` in `["open", "in_progress"]`
    * No open `blocks` or `parent-child` deps blocking it.

* **Standard “Next task” flow** currently:

  * Load `ProjectConfig` (`.openagents/project.json`),
  * Find ready tasks,
  * Pick the **highest-priority, oldest** task,
  * Mark `in_progress`,
  * Run Golden Loop.

You must **build on top of this**, not replace it.

---

## High-Level Behavior

### 1. FM Task Selection State Machine

When the FM-driven agent starts, the flow should look like this:

1. **Load config and tasks**

   * Read `.openagents/project.json` for project-level settings.
   * Load all tasks via `TaskService`.

2. **Compute candidate tasks**

   * Filter to **ready** tasks.
   * Sort by:

     1. `priority` ascending (0 first),
     2. `status` (`in_progress` before `open`),
     3. `createdAt` ascending.

3. **Build a small candidate summary** (for FM)

   * Truncate to e.g. top N = 10 candidates.
   * For each candidate include:

     * `id`, `title`, `priority`, `type`, `status`, `labels`, high-level `deps` summary.

4. **Call FM with a task-selection prompt**

   * Provide:

     * Project context (from AGENTS / project.json if needed).
     * `TaskSelectionPreferences` (see below) if provided.
     * List of candidate tasks (max 10).
   * Ask FM to choose **one of**:

     ```jsonc
     {
       "action": "work_on_task" | "run_plan_session",
       "taskId": "oa-123456",            // required if action = work_on_task
       "reason": "string explaining choice",
       "priorityThreshold": 2            // optional hint for min priority
     }
     ```

5. **Interpret FM answer:**

   * If `action == "work_on_task"` and `taskId` is a valid ready task:

     * Mark that task `in_progress`.
     * Proceed into Golden Loop for that task.

   * If:

     * No ready tasks exist, **or**
     * FM returns `run_plan_session`, **or**
     * FM selects a task below a configurable `minPriorityForWork` threshold,
     * Then run a **Plan Session** (see below).

### 2. TaskSelectionPreferences

Create a simple preference structure that can be passed in by user or orchestrator and included in the FM prompt:

```ts
// src/tasks/selection.ts
export interface TaskSelectionPreferences {
  // Optional: prefer tasks with these labels (e.g. "fm", "tbench", "p0-fix")
  preferredLabels?: string[];

  // Optional: de-prioritize these labels (e.g. "backlog", "docs-only")
  avoidLabels?: string[];

  // Optional: minimum priority to consider for direct work
  // e.g. 2 = ignore P3/P4 unless nothing else is available
  minPriorityForWork?: number;

  // Optional: per-label weights (positive boosts, negative penalties)
  labelWeights?: Record<string, number>;

  // Optional: per-type weights (e.g. bug > feature > chore)
  typeWeights?: Record<"bug" | "feature" | "task" | "epic" | "chore", number>;
}
```

**Internally**:

* Compute a **base score** for each ready task:

  ```text
  base = -priority (0 highest)
       + labelWeights[label] sum
       + typeWeights[type]
  ```

* Use base score + age for deterministic ordering **before** FM sees them, then FM makes the final choice among the top few.

Preferences should be **optional**; if omitted, fall back to pure priority + age.

---

## Implementation Details

### A. New Task Selection Module

**File:** `src/tasks/selection.ts`

**Exports:**

```ts
export interface TaskCandidateSummary {
  id: string;
  title: string;
  priority: number;
  type: string;
  status: string;
  labels: string[];
  createdAt: string;
  depsSummary: {
    blocks: string[];
    blockedBy: string[];
  };
}

export interface FMSelectionContext {
  project: ProjectConfig;             // from ProjectService
  preferences?: TaskSelectionPreferences;
  candidates: TaskCandidateSummary[];
}

export interface FMSelectionDecision {
  action: "work_on_task" | "run_plan_session";
  taskId?: string;
  reason?: string;
  priorityThreshold?: number;
}

export function getReadyTaskCandidates(
  tasks: Task[]
): TaskCandidateSummary[];

// scoring using TaskSelectionPreferences (pure/deterministic)
export function scoreAndSortCandidates(
  candidates: TaskCandidateSummary[],
  prefs?: TaskSelectionPreferences
): TaskCandidateSummary[];
```

**Responsibilities:**

* `getReadyTaskCandidates()`:

  * Filter `.openagents` tasks to “ready”.
  * Build `TaskCandidateSummary` structs.

* `scoreAndSortCandidates()`:

  * Apply deterministic scoring using `TaskSelectionPreferences`.
  * Return sorted array, highest score first.

No FM calls here. This is pure data preparation.

---

### B. FM Selection Call

**Where:** in the FM-oriented orchestrator / entrypoint (e.g. `src/cli/tbench-iterate.ts` or a new FM orchestration module you already use for TB runs and FM work).

**Add:**

```ts
// Pseudocode – in whichever FM agent orchestrator is appropriate
async function fmChooseNextAction(
  context: FMSelectionContext
): Promise<FMSelectionDecision> {
  const prompt = buildFMSelectionPrompt(context); // include prefs + top N candidates

  const fm = FMService.FMService; // existing Effect service
  const response = await runFMSelectionPrompt(fm, prompt);

  // Parse JSON-only answer into FMSelectionDecision; fail gracefully on errors
}
```

**Prompt requirements:**

* Instruct FM to **output only JSON**, shaped as:

  ```jsonc
  {
    "action": "work_on_task" | "run_plan_session",
    "taskId": "oa-...",
    "reason": "short explanation",
    "priorityThreshold": 2
  }
  ```

* Remind FM:

  * It must **only choose a `taskId` from the provided candidate list**.
  * If it truly believes no candidate should be worked on (e.g. all low priority, missing info), it should choose `run_plan_session`.

Handle parse failures by:

* Logging the raw FM response.
* Falling back to a deterministic “pick top candidate” behavior.

---

### C. Plan Session Module

**Files:**

* `src/agent/planner/fm-plan-session.ts` (new)
* Optional: `docs/mechacoder/PLANNING.md` (doc stub – optional but nice)

**Types:**

```ts
export interface PlanSessionPreferences {
  // Re-use TaskSelectionPreferences for weighting, plus:
  maxNewTasks?: number;      // safety cap
  allowedTypes?: string[];   // e.g. ["epic", "task"]
}

export interface PlanSessionRequest {
  project: ProjectConfig;
  existingTasks: TaskCandidateSummary[];  // snapshot from TaskService
  preferences?: PlanSessionPreferences;
  reason?: string;                        // from FMSelectionDecision
}

export interface ProposedPlanTask {
  title: string;
  description: string;
  type: "bug" | "feature" | "task" | "epic" | "chore";
  priority: number;               // 0..4
  labels?: string[];
  deps?: { id: string; type: string }[];
}

export interface PlanSessionResult {
  planSummary: string;
  createdTaskIds: string[];        // tasks that were actually written
  proposedTasks: ProposedPlanTask[]; // raw proposals (whether created or not)
}
```

**Behavior:**

1. **Input:**

   * `PlanSessionRequest` with:

     * project config,
     * snapshot of tasks,
     * optional preferences & FM reason.

2. **FM prompt:**

   * Provide:

     * High-level project context (AGENTS, existing epics, etc.),
     * A summary of current tasks (at least high-priority open tasks),
     * Preferences (`PlanSessionPreferences`),
     * Reason for planning (`FMSelectionDecision.reason`).

   * Ask FM to output **only JSON**:

     ```jsonc
     {
       "planSummary": "short plan",
       "tasks": [
         {
           "title": "...",
           "description": "...",
           "type": "task",
           "priority": 1,
           "labels": ["fm","tbench"],
           "deps": [{ "id":"oa-xxxx", "type":"parent-child" }]
         }
       ]
     }
     ```

3. **Task creation:**

   * Validate each proposed task:

     * Enforce priority within 0–4.
     * Limit to `maxNewTasks` (default e.g. 10).
   * Use **TaskService** to create tasks; for each successful creation:

     * Add a `discovered-from` dep back to the current session task (if any) or to a “planning” meta-task.
   * Collect `createdTaskIds`.

4. **Output:**

   * Return `PlanSessionResult` with:

     * `planSummary` from FM,
     * `createdTaskIds`,
     * `proposedTasks` (for logging/ATIF).

5. **Logging:**

   * Write a planning log entry into `docs/logs/YYYYMMDD/HHMM-fm-plan-session-log.md`.
   * Include:

     * `planSummary`,
     * `createdTaskIds`,
     * high-level justification.

---

## Integration into Golden Loop / Agent Startup

When the FM-driven agent starts (or when user says “Next task (FM)”):

1. **Load config + tasks.**

2. **Compute ready candidates.**

3. **Score & sort using `TaskSelectionPreferences` (if provided).**

4. **Truncate to top N, build `FMSelectionContext`.**

5. **Call `fmChooseNextAction(context)`.**

6. **If `work_on_task`:**

   * Mark task `in_progress`.
   * Proceed with Golden Loop for that task.

7. **If `run_plan_session`:**

   * Call `runPlanSession(request)` (PlanSession module).
   * Optionally, after planning:

     * Re-run selection to see if new tasks change the decision.

In all cases, you must:

* Continue to follow `GOLDEN-LOOP-v2.md` for implementation + verification.
* Use `.openagents/tasks.jsonl` via TaskService only.
* Log each session in `docs/logs/YYYYMMDD/*.md`.

---

## Acceptance Criteria

* A **new unit-tested** `TaskSelectionPreferences` + scoring implementation.

* `fmChooseNextAction` correctly handles:

  * Work vs plan decision.
  * Bad FM JSON output (falls back deterministically).

* Plan Session module can:

  * Generate a plan via FM,
  * Create `.openagents` tasks via TaskService,
  * Respect `maxNewTasks` / allowed types,
  * Log its activity.

* From the user point of view:

  * When they say “Next task (FM)” (or when FM mode starts),

    * The agent either:

      * Starts work on a reasonable next task, **or**
      * Runs a planning session and logs new planned tasks,
    * and both paths respect user/agent priorities when provided.
