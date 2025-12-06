



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

> Now, speculate how our system can reliably code, given the tightly-constrained context window. Like, how can we chunk things up as small as possible so that FM can do coding?

At a high level: you treat FM like a tiny, hyper-focused mechanic who only ever sees one bolt at a time and interacts with the rest of the car through tools, summaries, and handles—not raw text.

Let me break down how that can actually be made reliable.

---

## 1. Accept the constraint: FM is a **local coder**, not a global brain

Given an ~1100-char safe window, FM can’t “understand the repo” in one shot. So:

* **FM’s job:** make **small, local code edits** and run short reasoning loops.
* **Your job (orchestrator, skills, memory):** maintain global understanding, decide which slice FM should see next, and encode that slice + the minimum necessary context into the prompt.

So think in terms of “micro-steps” instead of “do the whole feature”:

> “Change this function’s return type and update its test to match”
> instead of
> “Implement OAuth login across the app.”

---

## 2. Represent code with handles, not bulk text

Instead of always stuffing raw files into FM’s context, you make the **filesystem and codebase navigable via tools** and **identifiers**:

* Tools you already have / can refine:

  * `find`/`glob`: locate files by pattern, module, or symbol.
  * `grep`: show **only matching lines** for a symbol/usage.
  * `read_file` with line ranges: `read_file(path, startLine, endLine)`.
  * `write_file`/`edit`: apply small patches.

In FM prompts, you don’t say “here’s src/app.ts (2000 lines)”. You say:

* “We’re editing `src/auth/login.ts`, function `validateCredentials`.”
* “Here is the current implementation (40–60 lines around it).”
* “Here is the signature of the function you cannot see but must respect: `src/auth/types.ts: AuthResult` (summarized).”

So the **unit of work** becomes:

* **A single function** or **small component**, plus:

  * a tiny snippet of related types,
  * maybe 1–2 small call sites,
  * or a summary for each.

Everything else is reachable via tools, **not** pre-injected into the prompt.

---

## 3. Use tools + orchestrator to walk the code on FM’s behalf

The orchestrator (or a higher-level process) drives the navigation:

1. FM says (via tools):

   * “Search for `validateCredentials` definition.”
   * “Show me 40 lines around it.”
   * “Show its return type definition.”
   * “Show where it’s called in tests.”

2. Orchestrator executes these as **separate tool calls**, builds a local “scratchpad” (outside FM’s context), and for each micro-step:

   * Rebuilds a **fresh tiny prompt**:

     * System: coding instructions.
     * “Here’s the function you’re editing.”
     * “Here’s the specific change we’re doing.”
     * “Here’s any type or call-site info you requested.”
   * Asks FM: “Produce a patch for this one region.”

3. The orchestrator applies that patch (with your existing edit tools) and repeats.

The key: **FM never holds the whole repo in its head.** It just does **one small hop at a time**. The orchestrator + tools traverse the graph of files for it.

---

## 4. Always work in **micro-tasks** that fit inside the window

You already have task/subtask decomposition (orchestrator + subtasks, plus TB tasks). For FM, push that decomposition further:

* Each subtask FM sees should be something like:

  * “Add this missing import and fix the TypeScript error at line 47.”
  * “Adjust the return type to match `AuthResult` and update the unit test accordingly.”
  * “Extract function `foo` from lines 30–45 into a new helper.”

Where each subtask:

* Uses at most:

  * 1 function’s definition (40–60 lines),
  * 1 test (another 20–40 lines),
  * 1 type/interface snippet,
* plus a few lines of instructions.

That’s easily within 1100 chars *if you’re ruthless about trimming*.

Your skill library and memory system are very helpful here:

* A **skill** might encode the micro-pattern:

  * “When TypeScript complains about missing property, find type definition, add the property, update all call sites.”
* That skill can be referenced in the prompt as a **short recipe** rather than full-blown code context.

---

## 5. Summaries, not history: keep y/z outside the prompt

In TRM terms (y = candidate, z = reasoning), or your own skills/memory:

* You **don’t** keep full conversation history in FM; you persist state in:

  * `progress.md`
  * skills library
  * memory store
  * ATIF trajectories

Then each FM call sees:

* `x` — current task fragment (function + immediate types/tests).

* A **short, distilled summary** of prior relevant work, e.g.:

  > Summary: Previously we decided:
  >
  > * `AuthResult` must include `userId` and `roles`.
  > * `validateCredentials` must throw `AuthError` on invalid passwords.
  >   We already updated `src/auth/types.ts`.

* Optionally 1–2 **relevant reflections** (“last time this failed, we forgot to update the tests”).

This keeps the FM message tiny:

* You never send 20 K of prior transcripts.
* You send maybe 2–5 lines of “lessons learned + constraints”.

All the heavy “state” lives in your JSON logs, skills, and memory, not in the FM context.

---

## 6. Make FM operate via a tiny, rigid tool protocol

The more constrained the tool protocol, the less context you need to explain it every time.

For example, your FM coding prompt can be:

> You are a coding micro-agent operating with these tools:
>
> * `grep(pattern, pathGlob)`
> * `read_file(path, startLine, endLine)`
> * `write_file(path, content, startLine, endLine)`
> * `run_tests()`
>
> Rules:
>
> * Only do one focused change at a time.
> * Ask for the smallest file slice you need.
> * When editing, always propose a patch for a concrete line range.
> * After editing, request `run_tests()` if tests are cheap.

Because the tool set is small and stable, you don't need to re-explain a ton; just refer to an example once and keep the rest as stable conventions.

You’ve already started this with `<tool_call>` style outputs + custom parsing; the next step is to make the **operation vocabulary** tiny and predictable.

---

## 7. Verify aggressively, not thoroughly

Since FM can’t see everything:

* **Don’t rely on FM to “reason about global invariants”**.
* Let FM produce minimal edits.
* Then your orchestrator:

  * Runs `bun test` / `bun typecheck` / `e2eCommands` (your existing verification pipeline).
  * Uses the results as the truth.
  * Feeds only the **error messages** plus a tiny bit of local context back into FM for the next micro-step.

Typical loop:

1. FM patches `validateCredentials`.

2. Orchestrator runs `bun test`.

3. Test fails with 1–2 TS errors or assertion errors.

4. You present to FM:

   * The failing error lines (10–20 lines around each),
   * The relevant function snippet again,
   * A reflection (“last time we forgot to update `AuthResult`”).

5. FM does another small patch.

This is how you can do **global** work using a **local** agent: tests + typechecker enforce global constraints; FM just keeps patching until the constraints are met.

---

## 8. Chunk the repo structurally, not by raw size

On top of line ranges, you can impose a **code segmentation scheme** so FM always deals in well-known chunks:

* Per file, maintain an index such as:

  ```json
  {
    "src/auth/login.ts": [
      { "id": "func:validateCredentials", "start": 10, "end": 52 },
      { "id": "func:loginHandler", "start": 54, "end": 120 }
    ],
    "src/auth/types.ts": [
      { "id": "type:AuthResult", "start": 5, "end": 20 },
      ...
    ]
  }
  ```

Then FM can say:

* “Show `func:validateCredentials` and `type:AuthResult`.”

and the orchestrator converts that to precise `read_file` calls and slices.

Now the unit you feed into FM is not “lines 10–52” by accident; it’s “this named function or type chunk.” That makes the prompts stable, easier to reason about, and easier to log/replay.

You can generate this index once via a code-map tool (AST-based or regex-based) and keep it cached.

---

## 9. Let TB shape the chunking discipline

Terminal-Bench is actually perfect as a forcing function:

* Many TB tasks are “single-file” or “small neighborhood” edits.
* You can design your FM TB integration such that:

  * Each TB task is by construction:

    * a small repo or a tiny subset of files,
    * with explicit instructions to only touch those files.

That **forces** FM to operate in the “micro-slice” mode and gives you a training playground:

* If FM succeeds at TB within these constraints, the same patterns can generalize to your real repo, just with more aggressive planning and slicing.

---

## 10. Learning to improve chunking over time

Finally, your learning stack can help FM **get better at choosing minimal context**:

* Skills:

  * “To fix TS error X, you usually only need Y and Z.”
  * So the orchestrator/scheduler reduces the number and size of `read_file`/`grep` calls.

* Reflexion:

  * When a run fails because “FM changed two files at once and broke unrelated code,” you record a reflection + skill: “In this pattern, only touch the test file; don’t touch implementation.”

* Episode learner:

  * Looks across successful episodes and learns heuristics like:

    * “For assertion mismatch, you only need the test and the function body.”
    * “For import errors, you only need a small header region and the module resolution info.”

So over time, the **average bytes per FM call shrink** and reliability goes up:

* Early runs: FM needs 3–4 chunks to understand a change.
* Later: it learns to request exactly 1–2 chunks and patch correctly the first time.

---

## Putting it all together

A realistic FM coding loop in your system looks like:

1. **Orchestrator** (or FM planner) picks a micro-subtask from tasks / TB.
2. It builds a **tiny context**:

   * target function or test (40–60 lines),
   * a type or summary,
   * 2–5 lines of “what we learned before” (memory/reflexion).
3. FM:

   * issues minimal navigation tool calls (`grep`, `read_file` small slice),
   * proposes a patch on a specific range.
4. Orchestrator applies patch, runs tests/typecheck.
5. Any errors are **fed back in tiny excerpts**, plus a new reflection.
6. Repeat until tests pass or a Healer/Archivist path kicks in.

The repo, the history, the TB suite, the skills, the ATIF trajectories—all of that is **offloaded** so FM never sees more than a few hundred characters at a time.

That’s how you get a “small context” FM to behave like a real coding agent: it becomes a **local stepper** in a much larger, stateful, test-driven control system.
