At the highest level, you’ve basically built a physics lab for agents:
you drop them into a controlled environment, grow a set of smart probes (tests) around that environment, then watch how well the agents survive and adapt under those probes.

Let me walk it as one coherent mental model.

---

## 1. The “world”: tasks + environments

**TB task = a little universe:**

* A **task spec**: description, goal, constraints.
* A concrete **environment**:

  * Docker image / platform
  * Installed languages and tools
  * Prohibited tools (anti-cheat)
  * File tree + key files (previews, not full dumps)

You then have a **builder** that turns “TB task folder” into a structured `EnvironmentInfo`:

* `platform`, `languages[]`, `prohibitedTools[]`
* `files[]` + small snippets of content
* Any task-specific knobs (e.g. tolerances, data schema, etc.)

> Mental model: every TB task is a tiny OS sandbox with a contract:
> “If you behave *within* these constraints and transform these files / data correctly, you win.”

---

## 2. The TestGen agent: growing probes around the world

Given **(task description + EnvironmentInfo)**, you spin up a *test-generation agent*.

This agent doesn’t solve the task; its job is to **design experiments** that would detect good vs bad solutions:

* It works **by category**:

  * `anti_cheat` – ensure forbidden shortcuts don’t work (e.g., `which R` must fail).
  * `existence` – expected files/results exist.
  * `correctness` – core behavior is right on canonical cases.
  * `boundary` – edge cases, limits, weird inputs.
  * `integration` – end-to-end flows.

* It runs **iteratively**:

  1. Generate a few tests for a category.
  2. Reflect: “What’s missing? What edge cases did I ignore?”
  3. Generate more tests if there are gaps.
  4. Move to the next category.
  5. At the end, do a **global self-assessment** (“comprehensiveness 1–10, what’s still weak?”) and maybe add more tests.

* Each test is a concrete little program:

  * **Input** (often a shell command)
  * **Expected output** or condition
  * **Reasoning** (why this test matters)
  * **Confidence score**

All of this streams out as a **trajectory**:
`start → progress → test → reflection → test → … → complete`.

You surface that trajectory in the TBCC TestGen tab as a **chronological thread**: progress ticks, tests, reflections, final summary.

> Mental model: TestGen is the “lab scientist” who walks around the environment and sets up measuring devices.

---

## 3. Trajectories + DB: turning runs into memory

Every TestGen run is now **snapshotted as a trajectory**:

* `sessionId`, `taskId`, `modelName`, timestamps
* All generated **tests**, **reflections**, **environment** snapshot
* Final stats: total tests, rounds, comprehensiveness, tokens, duration, uncertainties

This lands in SQLite in two layers:

* **Generic** `trajectories` table: “this was an agent run; here’s the full JSON trace.”
* **Testgen-specific** `testgen_trajectories`: “this run was test generation; here are the fields we care about” (tests, reflections, scores, env, etc.).

That’s what makes the *next* layer possible: you can now do analysis and evolution across many runs, not just eyeball a single session.

> Mental model: every generation run is a lab notebook entry, not a one-off printout.

---

## 4. The execution loop: agents under test

Separate from TestGen, you have the **coding agent loop** (HillClimber side):

1. Pick a TB task.
2. Drop a coding agent (Claude Code, etc.) into the task’s environment.
3. Let it read:

   * Task spec
   * Some subset of tests (self-generated ones, or earlier baseline tests).
4. The agent iterates:

   * Writes code / edits files.
   * Runs its own checks (subset of tests, custom printouts).
   * Refines until it thinks it’s right.
5. **Evaluator** then runs:

   * A battery of tests (some known, some hidden / blind).
   * Produces a score: pass/fail, partial, plus maybe “turns used” as a kind of cost.

HillClimber then uses those scores to tune **config knobs**:

* Max turns
* Hint usage
* Skill/memory injection
* Model choice, etc.

> Mental model: TestGen builds the probes; the evaluator uses those probes to judge the coding agent; HillClimber tunes the knobs that control how the coding agent behaves in that environment.

---

## 5. Meta on top of meta: evolving testgen itself

Because TestGen runs are persisted, you can now treat **test generation itself** as a TB/HillClimber task:

* Define a **TestGenConfig**:

  * Temperatures, rounds, per-category test counts
  * Prompt templates
  * Weights for environment vs description, anti-cheat emphasis, etc.

* For each TestGen run:

  1. Generate tests with that config.
  2. Analyze the trajectory:

     * Category balance
     * Anti-cheat coverage
     * Parameter discovery
     * Token efficiency
     * Self-reported comprehensiveness.
  3. Collapse that into a **quality score**.
  4. Store it as a `testgen_runs` row tied to that config.

* A **meta-reasoner** looks at recent runs + their scores and proposes slow, controlled changes to the config:

  * “Increase min tests for boundary from 2 → 3.”
  * “Turn down temperature for anti_cheat.”
  * “Adjust prompt to ask explicitly about forbidden tools.”

* Over time you get:

  * A *global* best config.
  * Optional per-task-type overrides (e.g. “conversion tasks need heavier anti-cheat emphasis”).

> Mental model: you now have a *HillClimber for tests* that sits alongside HillClimber for tasks. One optimizes how we **grow probes**, the other optimizes how we **solve under those probes**.

---

## 6. Big-picture mental model

If you squint, the whole architecture looks like this:

1. **Worlds** – TB tasks define little universes (codebase + environment + goal).
2. **Probes** – TestGen agents grow environment-aware tests around those worlds.
3. **Subjects** – Coding agents live inside those worlds and adapt to the probes.
4. **Judges** – Evaluators run the probes and assign scores.
5. **Scientists** – HillClimber and TestGen-HillClimber watch the outcomes and update the knobs that shape both probes and subjects.
6. **Memory** – SQLite trajectories + analyses turn all of this into a dataset you can mine, replay, and learn from.

That’s “what makes it all possible”:

* Strong **environment introspection** to give TestGen real context.
* A **streaming protocol** so long-running processes feel alive and incremental.
* A **trajectory-first mindset** (everything is a log of events, not just a final score).
* A **persistent store** so you can treat “how we generate tests” and “how we solve tasks” as things that themselves can be optimized over time.

If you’d like, next step we can literally sketch this as one diagram (worlds → probes → subjects → judges → scientists → memory) and align it with file boundaries so new contributors can grok it in one shot.
