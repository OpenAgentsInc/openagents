What you’re asking for is basically:

> “Wrap the FM TB runner in a tiny evolution loop that *remembers* what happened last time, nudges the config a bit, and keeps grinding on one task until it gets better.”

Yes, that’s absolutely doable within the constraints you’ve already set up (micro-task FM, small prompts, skills, hints, TB verification, Bun script).

Let me turn your scribbles into a concrete spec for **TBHillClimber**.

---

## 1. What TBHillClimber *is*

A **Bun CLI script** that:

1. Picks a **single TB task** (start with an “easy” one).
2. Runs that task with the FM micro-task runner using a **current config** (hints, skill usage, some knobs).
3. Reads the results (pass/fail, turns, logs, StepSummary).
4. Proposes **one tiny change** to the config for that task.
5. Saves the new config.
6. Loops.

Overnight it keeps doing:

> `config₀ → run → score₀ → tweak → config₁ → run → score₁ → …`

so it’s literally a hill climber: always trying to step to a slightly better config.

---

## 2. What “learning / thinking” means here

Within the constraints you have:

* FM **worker**: does the actual TB task (micro-task supervisor).
* TBHillClimber **meta-agent**: doesn’t need deep “reasoning”, but it:

  * Looks at previous run summaries (StepSummary, verification result).
  * Decides *one* change to try (e.g. add a hint, modify a hint, toggle a knob).
  * Records that as experience.

So “learning” is:

* **Persistent per-task config** that improves over iterations.
* **Skill/hint library updates** based on success/failure.
* Optionally, storing successful trajectories as new skills.

No magic; just a very small search over a very constrained space.

---

## 3. State & storage

Have a small JSON state file, e.g.:

`/.openagents/tb-hillclimber/state.json`

Structure something like:

```json
{
  "tasks": {
    "regex-log": {
      "currentConfig": {
        "hint": "Use a Python regex with lookbehind to capture the last date per line; write to /app/regex.txt.",
        "useSkills": false,
        "maxTurnsOverride": 30
      },
      "bestScore": {
        "passed": false,
        "turns": 5,
        "lastUpdatedAt": "2025-12-09T02:30:00Z"
      },
      "history": [
        {
          "runId": "tb-...",
          "configHash": "abc123",
          "passed": false,
          "turns": 4,
          "notes": "Timed out; no regex.txt created."
        }
      ]
    }
  }
}
```

TBHillClimber always:

* Loads this file at start.
* Chooses the task + config to work on.
* Appends a new entry to `history` after each run.
* Updates `currentConfig` if it found a better config.

---

## 4. Where FM fits in

There are two levels:

1. **Inner FM** (already built)

   * Micro-task worker doing TB tasks via tools.
   * Already has hints, StepSummary, verification, etc.

2. **Optional FM “meta” use**

   * You *can* use FM to suggest config tweaks, but given the tiny context, you might instead:

     * Let Bun/TypeScript encode simple heuristics (“if hint is empty, add one; if repeated same error, change hint text”).
     * Or call a *bigger* model (Claude Code) for the meta suggestions.

Given your constraints on FM and context, I’d start with **heuristic hill climbing in TypeScript** and only later experiment with FM as the meta thinker.

---

## 5. What knobs TBHillClimber can tweak

Keep the search space small so it’s learnable and safe:

### Per-task knobs:

* `hint`: one or two sentences that get injected into the prompt for that task.
* `useSkills`: boolean (once you re-enable skills for TB2).
* `topKSkills`: 0–5 (when skills are on).
* `maxTurnsOverride`: maybe 20–60.
* **Task-specific mini-rules**, e.g.:

  * For regex tasks: “Do / don’t read log file.”
  * For file transformation tasks: “Remember to write to the exact output path.”

At the start, I’d *only* let it tweak `hint` and maybe `useSkills`, and keep everything else fixed.

---

## 6. Hill-climbing algorithm (outer loop)

Pseudocode in Bun/TS:

```ts
while (true) {
  const state = loadState();
  const taskId = pickTask(state); // start with a single "easy" TB task
  const config = state.tasks[taskId].currentConfig;

  // 1. Run TB for this task with currentConfig
  const runResult = await runSingleTBTask(taskId, config);

  // 2. Score it
  const score = {
    passed: runResult.passed,
    turns: runResult.turns,
  };

  // 3. Decide whether and how to tweak
  const newConfig = proposeConfigChange(taskId, config, runResult);

  // 4. If newConfig equals old config and we failed, maybe fallback / restart
  // For now, always accept the new config if it's different
  if (newConfig !== config) {
    state.tasks[taskId].currentConfig = newConfig;
  }

  // 5. Update bestScore if improved
  if (score.passed && !state.tasks[taskId].bestScore.passed) {
    state.tasks[taskId].bestScore = { ...score, lastUpdatedAt: new Date().toISOString() };
  } else if (score.passed === state.tasks[taskId].bestScore.passed &&
             score.turns < state.tasks[taskId].bestScore.turns) {
    state.tasks[taskId].bestScore = { ...score, lastUpdatedAt: new Date().toISOString() };
  }

  // 6. Append history
  state.tasks[taskId].history.push({
    runId: runResult.runId,
    configHash: hashConfig(config),
    passed: score.passed,
    turns: score.turns,
    notes: summarizeRun(runResult), // we can use StepSummary here
  });

  saveState(state);

  // Sleep a bit so it doesn't hammer your machine
  await sleep(30_000); // 30s or longer for overnight
}
```

### `proposeConfigChange(...)`

For the first iteration, do something very simple:

* If **never passed**:

  * If `hint` is empty:

    * Add a very basic hint relevant to that task.
  * Else if last run failed with a *specific error* (e.g. “file not found”, “verification failed” string in StepSummary):

    * Slightly rewrite the hint in code based on heuristics:

      * For “file not found”: add path clarification.
      * For “no output file”: add success criteria reminder.
* If **passed** but too many turns:

  * Add or tweak a hint that encourages more direct behavior, like “Skip reading log file, just write regex to regex.txt.”

Later, once the scaffolding is stable, you can involve FM or Claude Code:

* Pass in:

  * Task description,
  * Last few StepSummary lines,
  * Current hint,
  * Whether it passed,
* Ask: “Propose a single small tweak to the hint that might help,” and update config with that.

---

## 7. How it interacts with TB runner

You already added `suitePath` and `suiteMode` and the hint plumbing.

For TBHillClimber, you need a way to:

* Run **one** task instead of the full suite.
* Inject a **task-specific hint** or config for that run.

So add a lightweight wrapper around TB runner:

```ts
async function runSingleTBTask(taskId: string, config: TaskConfig): Promise<RunResult> {
  // a) Build CLI args with --suite and --tasks=taskId
  // b) Set env vars or extend options so FM runner sees this task's hint
  //    e.g., export TB_HINT_TASKID=... or pass through RunTaskOptions

  // Option 1: modify model-adapter so RunTaskOptions has taskHint
  // Option 2: keep a small JSON config file that model-adapter reads.

  // c) Spawn tbench-local with those options and parse the summary JSON it writes.
}
```

You already have the plumbing for `verifyTask` etc., so TBHillClimber only needs to:

* Feed in `taskHint` / `useSkills` per task.
* Read the resulting JSON summary you’re already writing to `.openagents/tb-runs/...`.

---

## 8. Starting with an “easy” task

Pick one “easy” TB2 task that:

* Has a single-file output requirement.
* Doesn’t need external packages or heavy sysadmin.

Good candidates from your earlier thinking:

* **regex-log** (medium, but conceptually simple)
* Or one of the simpler file-ops / text-processing tasks if there are TB2 tasks rated “easy”.

Design TBHillClimber so **only that task** is active at first:

```json
"tasks": {
  "regex-log": { ... }
}
```

Once you see it stabilizing (e.g. config converges to passing, fewer turns), you can:

* Add another task id to the state file,
* Change `pickTask` to round-robin among configured tasks.

---

## 9. “Can it think about stuff?”

Given FM’s constraints, the “thinking” has to be:

* **Distributed** between:

  * Hard logic in Bun (hill climbing, state, scoring).
  * Short, targeted hints and skills.
  * TB verification as the authority.

If you want FM to also be the *meta* thinker later, you can:

* Use the FM client again with a **different prompt** like:

  > “You are a tiny tuning agent. Here is the task description, current hint, and a summary of the last run (pass/fail, a few StepSummary lines). Suggest a single revised hint that might improve performance.”

* That prompt is small and fits within FM’s context.

* The Bun hill climber then tests that new hint and keeps it or discards it based on verification.

That’s about as much “thinking” as you can safely expect FM to do, and it fits neatly into your micro-task architecture.

---

If you’d like, next step we can:

* Pick a specific TB2 task id (e.g. `regex-log`) and
* Draft the exact Bun script skeleton + the per-task `TaskConfig` type and how it plugs into your existing `model-adapter`.
