Here’s something you can more or less drop straight into a new `oa-*` task (or split into a few). It assumes the commit you linked is the baseline where **FM + hello-world TB** works.

---

## Task 1 — Harden FM adapter (context + tool parsing)

**Title:** Harden FM adapter for Terminal-Bench (context + tool parsing)

**Read:**

* `docs/logs/20251206/1140-fm-tbench-working.md`
* `src/bench/model-adapter.ts`
* `src/cli/tbench-iterate.ts`
* `tasks/hello-world-suite.json`
* `.openagents/gym/episodes.jsonl` (only the latest FM hello-world runs)

**Goals:**

1. Make FM context handling configurable & test-covered.
2. Make FM tool-call parsing robust against weird outputs and future tasks.
3. Add clear failure logging when FM doesn’t emit usable `<tool_call>`s.

**Implementation details:**

1. **Configurable max context**

   * Extract the current FM char limit into a config constant, e.g.:

     ```ts
     const FM_MAX_CONTEXT_CHARS_DEFAULT = 1100;
     ```

   * Allow overriding per-model via a small config object in `model-adapter.ts` (e.g. `FM_MODEL_CONFIG[modelName].maxContextChars`).

   * Update `truncateMessagesForFM()` to:

     * Accept `maxChars: number`.
     * Keep **system prompt + last user/assistant message**.
     * Drop oldest messages first, not arbitrary slices.

   * Add unit tests for `truncateMessagesForFM()`:

     * Keeps system + last message.
     * Drops middle when over size.
     * Is deterministic.

2. **FM tool-call parsing hardening**

   * In the FM branch of `model-adapter.ts`, refactor `parseDescriptiveToolCall()` so it:

     * Handles:

       * Proper `<tool_call>{…}</tool_call>` tags.
       * JSON with a `response` string describing tool usage.
       * Raw text like `"Using write_file tool with arguments: path=..., content=..."`.
     * Returns **structured** `{ name: string; arguments: Record<string, unknown> }` or a typed error.
   * Add unit tests for all known patterns seen in the hello-world episodes, plus a couple of adversarial ones:

     * Missing quotes.
     * Extra commentary around the JSON.
     * Multiple candidate `<tool_call>` tags (pick the first valid).

3. **Failure logging**

   * When FM output cannot be parsed into a valid tool call:

     * Log a **single structured error** line (e.g. `[FM_TOOL_PARSE_ERROR] {rawSnippet, reason}`) that will show up in:

       * TB logs and/or `.openagents/run-logs`.
     * Return a clear error to the harness so TB marks the iteration as failed with a precise reason, not “generic error”.
   * Add at least one integration test (can be small TB stub) that:

     * Forces a bad FM output.
     * Asserts that the error is surfaced in a structured way.

**Acceptance criteria:**

* All new tests pass via `bun test`.
* FM hello-world TB still passes 5/5 iterations.
* There is a single place to tune FM context size (no magic numbers scattered).
* FM tool-call parse errors are understandable from logs without re-running under a debugger.

---

## Task 2 — Expand FM hello-world suite into a small FM-TB mini-suite

**Title:** Expand FM hello-world TB suite into multi-task FM mini-suite

**Read:**

* `tasks/hello-world-suite.json`
* `docs/terminal-bench.md` (or TB docs in `docs/tbench/*`)
* Latest TB run artifacts under `.openagents/tb-runs/`

**Goals:**

Create a **small FM-friendly TB suite** (5–10 tasks) that exercises:

* Multiple tools (`write_file`, `read_file`, `run_command`).
* Simple multi-step behavior.
* Verification via custom scripts.

**Implementation details:**

1. **New suite file**

   * Create `tasks/fm-mini-suite.json` (or similar) that includes:

     * **Task 1:** “hello-world” file creation (existing one).
     * **Task 2:** Write a file then read it back and print content.
     * **Task 3:** Append to an existing file.
     * **Task 4:** Use `run_command` to execute something simple (e.g. `ls` or `cat hello.txt`) and verify output.
     * **Task 5+:** One slightly more “real” coding task, e.g. generate a tiny script and run it.

2. **Custom verification scripts**

   * For tasks that require filesystem verification, use the existing `"verification": "custom"` mechanism and add scripts under a predictable path, e.g. `tasks/verify/fm-mini-*`.
   * Scripts should:

     * Exit 0 on success, non-zero on failure.
     * Check file existence, content, and command output where relevant.

3. **Integrate with TB CLIs**

   * Ensure `tbench-local` and `tbench-iterate` can run the new suite via a simple command:

     ```bash
     bun run tbench-local \
       --model fm.apple-foundation \
       --suite tasks/fm-mini-suite.json
     ```

   * Document the suite briefly in `docs/terminal-bench.md` as “FM mini-suite for regression”.

**Acceptance criteria:**

* Running the new suite with FM and the hardened adapter from Task 1 yields **consistent, repeatable successes** (e.g. ≥ 4/5 runs fully passing).
* Each task has a clear, deterministic verifier; no “eyeball the logs” steps.
* Suite is small enough to be used in CI or fast local smoke runs.

---

## Task 3 — Add regression test wiring for FM mini-suite

**Title:** Add FM mini-suite regression path for Terminal-Bench

**Read:**

* `src/cli/tbench-iterate.ts`
* TB CI workflow YAML under `.github/workflows/*` (if present)
* New `tasks/fm-mini-suite.json` from Task 2

**Goals:**

Make it easy to run “FM + mini-suite” as a regression and, optionally, wire it into CI as an opt-in job.

**Implementation details:**

1. **CLI wrapper**

   * Add a convenience script to `package.json`, e.g.:

     ```json
     "scripts": {
       "tbench:fm-mini": "bun src/cli/tbench-local.ts --model fm.apple-foundation --suite tasks/fm-mini-suite.json --max-tasks 10"
     }
     ```

   * Optionally, a variant using `tbench-iterate` once the learning flags are in:

     ```json
     "scripts": {
       "tbench:fm-mini:iterate": "bun src/cli/tbench-iterate.ts --model fm.apple-foundation --suite tasks/fm-mini-suite.json --max-tasks 10"
     }
     ```

2. **Optional CI job (if you want it now)**

   * Add a *non-blocking* CI job (e.g. `fm-tbench-mini`) that:

     * Installs deps,
     * Starts FM bridge (behind a flag / only on macOS runners if applicable),
     * Runs `bun run tbench:fm-mini`,
     * Reports pass/fail but can be marked as “allowed to fail” initially.

**Acceptance criteria:**

* From a fresh checkout + FM bridge running, `bun run tbench:fm-mini` passes end-to-end.
* If CI wiring is added, it doesn’t slow down core tests and can be disabled easily via config.

---

If you want this as *one* big epic for MechaCoder, you can wrap them as:

> **Epic:** “Harden FM + Terminal-Bench integration beyond hello-world”
> Children: Task 1 (adapter hardening), Task 2 (FM mini-suite), Task 3 (regression wiring).

You can then follow up with a second epic later:

* “Wire `tbench-iterate` learning flags (`--skills`, `--memory`, `--reflect`) and run FM learning sweeps.”

But the three tasks above are the clean “next steps” straight after the `f5744434` commit.
