Yeah, this is *extremely* informative. You just did the world’s first “FM vs full TerminalBench” crash test and it’s telling you exactly where the scaffolding is still wrong.

Let me distill what’s going on and what I’d change next.

---

## 1. Big-picture: what we’re seeing

Across **path-tracing, model-extraction, video-processing, dna-assembly, regex-log, large-scale-text-editing, sqlite-with-gcov, git-multibranch**, the pattern is:

* FM **can** follow the protocol (`<tool_call>…`) and use tools.
* The **micro-task loop is working** mechanically.
* But:

  * Hints from the mini-suite are **polluting** the behavior.
  * FM is often **misguided or under-instructed** for the TB tasks.
  * Context is blowing up again for long scientific tasks.
  * `task_complete` and repeat heuristics are **way too trusting**.
  * Skills are **only text**; FM is even trying to call them as tools (`Setup Bun Project`).

So the failures are *almost all* in the orchestrator + prompts + hint layer, not in the FM bridge itself.

---

## 2. Concrete failure modes from the logs

### 2.1 Hint bleed: mini-suite heuristics hurting TB

You can see old “mini benchmark” hints leaking into TB:

* **path-tracing**

  * Hint: `This task requires reading a file first. Use read_file before writing.`
  * But the spec *explicitly forbids* reading `/app/image.ppm`.
  * FM obediently calls `read_file("image.ppm")`, gets `File not found`, then repeatedly spams `gcc image.c` without ever writing `image.c`.

* **dna-assembly**

  * After `read_file("sequences.fasta")` you get:

    > Hint: You just read file content. Write it EXACTLY to the target file…
  * That’s the **read-and-echo** hint from fm-mini, now being applied to a Golden Gate primer design task.
  * FM happily creates an *empty* `primers.fasta` and then gives up.

* **regex-log**

  * Same “read file first” hint even though the task doesn’t actually require seeing the log contents.
  * FM tries `read_file("log.txt")` (fails) and then just calls `task_complete`.

These are super strong priors that were tuned for 7 toy tasks and now are steering the agent off a cliff on completely different tasks.

> **Conclusion 1:**
> Mini-suite hints must be **gated by suite / task type** or turned off for TB2. Right now they’re global and harmful.

---

### 2.2 Context explosion: stuffing file contents into `Previous`

Example: **dna-assembly**:

* `read_file("sequences.fasta")` returns a *huge* sequence string.
* That full `"sequences.fasta contains: >input …"` line gets shoved into `Previous:` verbatim.
* Next prompt is ~3900–4000 chars and you hit:

  > `Error: Foundation Models request failed: Exceeded model context window size`

Same pattern in sqlite-with-gcov/log: `Previous` is carrying long tool outputs (errors, long path, etc.).

We talked about this in theory—`StepSummary` and per-entry caps—but this run is screaming:

> **Conclusion 2:**
> You must actually implement the **StepSummary / truncated Previous** plan, or the micro-task loop will keep reintroducing the context problem.

---

### 2.3 Over-trusting `task_complete` + “repeat” heuristics

You see this clearly in:

* **dna-assembly**:

  * FM reads sequences.
  * Writes **empty** `primers.fasta`.
  * Context blows up → FM errors.
  * Then just shrugs and calls `task_complete`.
  * Orchestrator: “FM signaled task complete” → run verification → fails.

* **regex-log**:

  * Fails `read_file("log.txt")` once.
  * Immediately calls `task_complete`.
  * Orchestrator believes it.

And for several tasks (`path-tracing`, `video-processing`) the “same action repeated 3 times” fallback fires even though nothing correct was done.

These heuristics were okay on fm-mini where the surface area was tiny. Under TB load they are basically **“give up” buttons**.

> **Conclusion 3:**
> `task_complete` and repeat-3x should **only be accepted when verification passes**, otherwise they’re just “bail out” tokens.

---

### 2.4 Skills are visible, but not *usable*

* FM sees “Relevant Skills: Grep Code, Git Status, Setup Bun Project …”

* In **git-multibranch**, it actually tries to call:

  ```text
  tool=Setup Bun Project, args={"tool":"Setup Bun Project"}
  ```

* `executeTool` naturally says `Unknown tool: Setup Bun Project`.

So the skills being injected are:

* Semantic fodder for the FM router (good).
* But they are **not aligned with available tools** (bad: FM thinks skills are tools).
* And many are irrelevant to the TB task (`Sanitize User Input` for path-tracing, `Implement Rate Limiting` for dna-assembly).

> **Conclusion 4:**
> You need either:
>
> * a `use_skill` tool and a skill-execution layer, or
> * a **much stricter separation** between “skills in text” and “tools in the list” + better skill filtering per task.

---

### 2.5 TB task specifics the model ignores (because hints are generic)

Quick per-task snapshots:

* **path-tracing**

  * Ignores spec about not reading image.
  * Never writes `image.c`.
  * Just loops on `gcc image.c`.

* **model-extraction-relu-logits**

  * Calls `python3 forward.py` (fine).
  * Then writes several utterly bogus `/app/steal.py` variants with random torch code, ignoring the query-based extraction spec.

* **video-processing**

  * Writes placeholder scripts that never compute `jump_takeoff_frame_number` / `jump_land_frame_number` meaningfully.
  * Rewrites script multiple times; ends with comment-only script.
  * Never runs the script or checks `/app/output.toml`.

* **dna-assembly**

  * Just treats it like read-and-echo because of the hint.
  * No attempt at primer design.

* **regex-log**

  * Doesn’t even write `/app/regex.txt`.

* **large-scale-text-editing (vim macros)**

  * This one is just very hard for FM:

    * Tries various `vim` invocations.
    * Fails to respect allowed commands.
    * Tool-call parsing chokes on complex macro content (triple quotes etc.).

* **sqlite-with-gcov**

  * Uses `cd /app` where `/app` doesn’t exist in your local runner.
  * Ignores instruction to use `/app/vendor/sqlite-fossil-release.tar.gz`; tries `git clone`.

So: part task difficulty, part missing domain skills, but also lots of generic planlessness: “just run some commands, maybe set PATH, maybe clone something.”

---

## 3. What I’d change next (pragmatic patch set)

Let’s keep this practical. Here’s a prioritized patch list that directly attacks what you’re seeing **without** re-architecting everything in one go.

### 3.1 Gate or neuter the fm-mini hints for Terminal-Bench

Right now `Hint: This task requires reading a file first` and `Hint: You just read file content. Write it EXACTLY…` are global.

You want something like:

```ts
// In TB CLI / model-adapter when you call runMicroTaskPlan:
runMicroTaskPlan(client, plan, {
  ...options,
  suite: "terminal-bench-2",  // or pass a mode flag
});

// In the hint engine:
function buildHint(taskId: string, description: string, mode: "fm-mini" | "tb2"): string | undefined {
  if (mode === "fm-mini") {
    // Keep the old hints for the tiny suite
    ...
  }

  // For TB2, be *very* conservative at first:
  // maybe no hints at all, or just:
  if (description.toLowerCase().includes("count") && description.toLowerCase().includes("word")) {
    return `Hint: prefer shell tools like "wc -w" over counting manually.`;
  }

  return undefined;
}
```

Short version: **turn off the mini-suite hint hacks for TB2** until you add TB-specific ones.

---

### 3.2 Implement `StepSummary` and cap `Previous`

This is the fix for both context and interpretability. Something along the lines of what we sketched earlier:

```ts
interface StepSummary {
  step: number;
  tool: string;
  success: boolean;
  message: string; // <= 100 chars
}

const MAX_SUMMARIES = 3;
const MAX_SUMMARY_CHARS = 100;

function summarizeToolResult(...): StepSummary { /* no raw file contents */ }

function buildPreviousField(history: StepSummary[]): string {
  if (history.length === 0) return "none";
  return history.map(h => `Step ${h.step}: ${h.message}`).join("; ");
}
```

And in the worker prompt builder:

```ts
const previous = buildPreviousField(history);
// No more “sequences.fasta contains: [massive Fasta dump]”
```

This should eliminate:

* `Exceeded model context window size` on dna-assembly/sqlite-with-gcov.
* Prompt bloat that confuses FM (it doesn’t need to see whole genomes in its context).

---

### 3.3 Treat `task_complete` and repeat-3x as “run verification now”, not “accept”

Modify orchestrator semantics to something like:

```ts
if (toolCall.name === "task_complete" || repeatedSameAction3x) {
  log("[Orchestrator] FM signaled completion, running verification");
  if (options.verifyTask) {
    const ok = await options.verifyTask();
    if (ok) return successResult(...);
    log("[Verifier] Verification failed, continuing run");
    // maybe nudge FM: "Verification failed; please fix the solution"
  } else {
    // No verifier? ok, treat as done.
    return successResult(...);
  }
}
```

For TB you *have* verification scripts; wire them in from `tbench-local`:

```ts
const verifyTask = async () => {
  const proc = Bun.spawn(["sh", "-c", task.verifyScript], { cwd: workspace, ... });
  return (await proc.exited) === 0;
};

await runner.runTask(task, { ..., verifyTask });
```

This alone stops dna-assembly / regex-log from bailing out after doing nothing.

---

### 3.4 Stop using absolute `/app` paths at the FM level (or normalize them)

You already normalize `/app/foo` in `write_file`/`read_file` by dropping the leading `/` and using `basename`. But `run_command` is raw shell.

Two options:

1. **Instructional fix** (easy, good enough for now)

   Add to worker system prompt:

   > “All task files are available in the current working directory; do not use `/app/...` absolute paths in shell commands. When the spec says `/app/foo`, you should use `foo` as the path.”

   That alone would stop `cd /app` / `touch /app/jump_analyzer.py` from failing.

2. **Shell normalization** (heavier but robust)

   Preprocess `command` in `run_command`:

   * Replace `/app/` with `./`.
   * Maybe rewrite `cd /app && X` into just `X` if the workspace is already the correct root.

Given how messy shell can be, I’d start with the **system prompt rule** and see if the FM stops referencing `/app` in run_command.

---

### 3.5 Introduce `use_skill` (or don’t advertise skills as tools)

For now FM is *hallucinating* skill names as tools:

```text
tool=Setup Bun Project
```

Two paths:

* **Short term**:
  Make it very clear in the system prompt that the **only** valid tools are the 5 you list, and skills are just “ideas”. Don’t list skill names in any format that looks like tool names; keep them as natural language patterns.

* **Medium term**:
  Add a real tool:

  ```ts
  {
    name: "use_skill",
    description: "Execute a high-level skill by id",
    parameters: {
      type: "object",
      properties: { id: { type: "string" }, input: { type: "object" } },
      required: ["id"],
    },
  }
  ```

  And then in the orchestrator, map `use_skill` to the SkillService and run its implementation.

Right now the mismatch (“skills look like tools but aren’t”) is confusing the model *and* you. I’d at least make the textual formatting less tool-like (e.g., “Example patterns:” instead of `- SkillName:`).

---

### 3.6 Per-task-family hinting (instead of global hacks)

Once you’ve neutered the fm-mini hints and added StepSummary, then you can start layering TB-smart hints:

* For **regex-log**:

  * Hint: “You don’t need to read the log file; just write a regex into `/app/regex.txt` following the spec.”

* For **sqlite-with-gcov**:

  * Hint: “Use `tar -xzf /app/vendor/sqlite-fossil-release.tar.gz` in the current directory and compile from there. Do **not** `git clone`.”

* For **vim macros**:

  * Hint: “You only need to write three `call setreg` lines and three `:%normal! @x` lines plus `:wq`. No other commands are allowed.”

But crucially: these should be **tied to task ids or categories**, not generic string-matched across suites, to avoid a repeat of the mini→TB contamination.

---

## 4. Meta-takeaway

What this run really showed:

* The **micro-task + tool-calling skeleton is sound**; nothing crashed, all tools executed, logs are rich.
* FM as a small router **can move around**, but:

  * It’s extremely literal about hints (so hints must be surgically correct).
  * It can’t handle raw, unfiltered tool outputs in its prompt.
  * It will happily say “I’m done” even when it’s completely wrong unless you tie that to verification.

So I wouldn’t take these failures as “FM can’t do TerminalBench”; I’d take them as:

> “We just plugged a tiny router into a huge, complex benchmark without teaching the router the right rituals yet.”

If you implement:

1. Suite-aware hint gating (turn off mini hacks for TB),
2. StepSummary truncation for `Previous`,
3. Verification-gated completion (`task_complete` + repeat-3x),
4. A simple rule about `/app` paths,
5. Slightly less confusing skill presentation,

you’ll already get a *much* cleaner signal about where FM truly lacks domain competence vs where the orchestrator was just leading it astray.

And from there you can start adding true TB-specific skills / hints (regex, sqlite build patterns, Golden Gate primers via precomputed examples, etc.) instead of fighting with that fm-mini muscle memory.
