# 0045 FM Terminal-Bench Fix Plan

**Based on:** First full TB2 run (tb-20251208061743-cawusg) + ChatGPT analysis
**Status:** Ready for review

---

## Executive Summary

The FM micro-task skeleton works mechanically (tool calling, orchestration, logging). But 5 systemic issues cause near-total TB2 failure:

1. **Mini-suite hints pollute TB tasks** (wrong guidance)
2. **Context overflow** (raw file contents in `Previous`)
3. **Premature completion** (`task_complete` accepted without verification)
4. **`/app` path confusion** (FM uses absolute paths that don't exist)
5. **Skills mistaken for tools** (FM tries to call skill names)

**Goal:** Fix these 5 issues to get a clean signal on actual FM capability.

---

## Priority 1: Blocking Issues (Enable any pass rate)

### 1.1 Gate fm-mini hints for TB2

**Problem:** Hints like `"This task requires reading a file first"` are tuned for 7 toy tasks and now actively mislead FM on TB2.

**Evidence:**
- path-tracing: Hint tells FM to read file, but spec forbids reading image.ppm
- dna-assembly: Hint says "write content EXACTLY", FM creates empty primers.fasta
- regex-log: FM reads non-existent log.txt instead of writing regex.txt

**Fix Location:** `src/fm/worker.ts` or wherever hints are injected

**Implementation:**
```typescript
// src/fm/hints.ts (new file)
export type SuiteMode = "fm-mini" | "tb2" | "unknown";

export function buildHint(
  taskId: string,
  description: string,
  mode: SuiteMode
): string | undefined {
  // For TB2, start with NO hints - let FM figure it out
  if (mode === "tb2") {
    return undefined;
  }

  // fm-mini keeps existing behavior
  if (mode === "fm-mini") {
    // ... existing hint logic from worker.ts
  }

  return undefined;
}
```

**Propagation:**
- `model-adapter.ts`: Pass `suiteMode: "tb2"` when running TB
- `orchestrator.ts`: Accept suiteMode in options
- `worker.ts`: Use `buildHint()` with mode

**Test:** Run path-tracing, confirm no "read file first" hint appears.

---

### 1.2 Implement StepSummary (cap Previous field)

**Problem:** `Previous` field carries raw tool output (e.g., entire DNA sequences), causing:
- Context window exceeded errors
- FM confusion from irrelevant verbosity

**Evidence:**
- dna-assembly: `Previous: sequences.fasta contains: >input actcttcctt...` (massive)
- sqlite-with-gcov: Error messages compound until context exhausted

**Fix Location:** `src/fm/orchestrator.ts`

**Implementation:**
```typescript
// src/fm/step-summary.ts (new file)
export interface StepSummary {
  step: number;
  tool: string;
  success: boolean;
  message: string; // Capped at 100 chars
}

const MAX_SUMMARIES = 3;
const MAX_MESSAGE_CHARS = 100;

export function summarizeToolResult(
  step: number,
  tool: string,
  success: boolean,
  rawOutput: string
): StepSummary {
  // Extract meaningful prefix, truncate
  let message = rawOutput.slice(0, MAX_MESSAGE_CHARS);
  if (rawOutput.length > MAX_MESSAGE_CHARS) {
    message = message.slice(0, 97) + "...";
  }
  return { step, tool, success, message };
}

export function buildPreviousField(history: StepSummary[]): string {
  if (history.length === 0) return "none";

  // Keep only last N summaries
  const recent = history.slice(-MAX_SUMMARIES);
  return recent
    .map(h => `Step ${h.step} (${h.tool}): ${h.message}`)
    .join("; ");
}
```

**Integration in orchestrator:**
```typescript
// In executeStep():
const summary = summarizeToolResult(step, result.tool, result.success, result.output);
history.push(summary);

// In buildWorkerPrompt():
const previous = buildPreviousField(history);
```

**Test:** Run dna-assembly, confirm Previous is <300 chars total.

---

### 1.3 Verification-gated completion

**Problem:** FM calls `task_complete` or loops same action 3x, orchestrator accepts as "done" even when nothing is achieved.

**Evidence:**
- regex-log: FM fails read, immediately calls task_complete, 0 work done
- dna-assembly: Creates empty file, calls task_complete
- video-processing: Rewrites script 3x → "Same action repeated 3 times - task complete"

**Fix Location:** `src/fm/orchestrator.ts`

**Implementation:**
```typescript
interface OrchestratorOptions {
  // ... existing
  verifyTask?: () => Promise<boolean>;
  maxRetryAfterFailedVerify?: number; // default 2
}

// In the main loop:
if (toolCall.name === "task_complete" || repeatedSameAction >= 3) {
  if (options.verifyTask) {
    const passed = await options.verifyTask();
    if (passed) {
      return { success: true, message: "Task verified" };
    }

    // Verification failed - nudge FM to continue
    retryCount++;
    if (retryCount >= (options.maxRetryAfterFailedVerify ?? 2)) {
      return { success: false, message: "Verification failed after retries" };
    }

    // Add feedback to context
    history.push({
      step: currentStep,
      tool: "verification",
      success: false,
      message: "Task verification failed. Please fix and try again."
    });
    continue; // Don't exit loop
  }

  // No verifier = trust the signal (backward compat for fm-mini)
  return { success: true, message: "FM signaled completion" };
}
```

**Wire from TB runner:**
```typescript
// In model-adapter.ts or tbench-local.ts
const verifyTask = async () => {
  const result = await runVerification(task, workspace);
  return result.passed;
};

await runMicroTaskPlan(client, plan, { ...options, verifyTask });
```

**Test:** Run regex-log, confirm FM continues after first task_complete fails verification.

---

## Priority 2: Quality Improvements

### 2.1 Normalize /app paths in system prompt

**Problem:** TB tasks reference `/app/foo`, but FM runs in workspace where `/app` doesn't exist.

**Evidence:**
- video-processing: `touch: /app/jump_analyzer.py: No such file or directory`
- sqlite-with-gcov: `cd /app` fails

**Fix Location:** `src/fm/worker.ts` system prompt

**Implementation:** Add to system prompt:
```
PATH RULES:
- The workspace is the current directory (.)
- When task mentions "/app/foo", use "foo" (relative path)
- Never use absolute /app paths in commands
```

**Alternative (heavier):** Rewrite `/app/` in run_command args:
```typescript
// In tool execution
if (tool === "run_command") {
  args.command = args.command.replace(/\/app\//g, "./");
}
```

Start with prompt fix, escalate to rewrite if needed.

---

### 2.2 Clarify skills vs tools

**Problem:** FM sees skill names and tries to call them as tools.

**Evidence:**
- git-multibranch: `tool=Setup Bun Project` → Unknown tool error

**Fix Location:** `src/fm/worker.ts` prompt building

**Implementation:**
```typescript
// Current (confusing):
// Relevant Skills:
// - Setup Bun Project: Initialize a Bun project...

// Better (clearly not tools):
// Example Patterns (for reference only, not callable):
//   Pattern: "Setup Bun Project" - Initialize a Bun project...
//   Pattern: "Grep Code" - Search for text patterns...
```

Or just remove skill names entirely and keep descriptions:
```
Example approaches:
- Initialize a Bun project with proper configuration
- Search for text patterns in files
```

---

## Implementation Order

```
Day 1:
  [ ] 1.1 Gate hints (new hints.ts, wire through)
  [ ] 1.2 StepSummary (new step-summary.ts, integrate in orchestrator)

Day 2:
  [ ] 1.3 Verification-gated completion (orchestrator changes, wire from TB)
  [ ] 2.1 /app path normalization (prompt rule)
  [ ] 2.2 Skills presentation fix

Day 3:
  [ ] Full TB2 re-run
  [ ] Document new baseline
```

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Tasks with context overflow | ~3/11 | 0 |
| Tasks that bail on first error | ~4/11 | 0 |
| Tasks confused by hints | ~3/11 | 0 |
| Tasks with /app path errors | ~2/11 | 0 |
| Clean pass rate baseline | 0% | Measurable (even 5% is signal) |

---

## Files to Modify

```
src/fm/
├── hints.ts          (NEW) - Suite-aware hint logic
├── step-summary.ts   (NEW) - StepSummary type + builders
├── worker.ts         - Use buildHint(), fix skills presentation
├── orchestrator.ts   - Use StepSummary, verification-gated completion

src/bench/
├── model-adapter.ts  - Pass suiteMode, wire verifyTask
```

---

## Questions for Review

1. **Verification integration:** Should verification run inline in orchestrator, or return control to TB runner to verify? (I lean toward inline for tighter loop)

2. **Hint strategy for TB2:** Start with zero hints, or add a few TB-specific ones? (I lean toward zero, add incrementally based on patterns)

3. **Skills:** Keep them at all for TB2, or remove entirely until we have use_skill tool? (I lean toward keep but reformat)

---------

ChatGPT response:

Yeah, this is a *very* solid fix plan. You’re aiming at exactly the five things that are actually killing the run, not prematurely redesigning the world.

I’d keep almost everything, but I’d nudge / add a few details:

---

## 1. Hints: fully agree, but add suite-aware wiring

Your 1.1 is right on:

* **Key points**:

  * Hints must be suite-aware.
  * TB2 should start with **no generic hints** until you add TB-specific ones.

**Minor additions:**

* Make sure `suiteMode` is **actually coming from TB**, not hardcoded:

  * E.g. `terminal-bench-mini.json` → `"fm-mini"`.
  * `terminal-bench-2.json` → `"tb2"`.
* Consider also having a **task-level escape hatch**:

  * E.g. a per-task `hints: "off"` flag in the suite file if you ever want to disable hints selectively.

But conceptually: ✅

---

## 2. StepSummary: yes, but make it tool-aware, not just slice

I love the `StepSummary` abstraction and the per-entry cap. One tweak:

Your current sketch:

```ts
message = rawOutput.slice(0, MAX_MESSAGE_CHARS)
```

will still produce unreadable mush like:

> `sequences.fasta contains: >input actcttcctttttc...`

Better to make it **tool-specific**:

* For `read_file`:

  * message: `Read file sequences.fasta (length ~12345 chars)`
* For `run_command`:

  * message: `Ran: gcc -static -o image image.c -lm (exit 1)`
* For `write_file`:

  * message: `Wrote 123 bytes to jump_analyzer.py`

So I’d change `summarizeToolResult` to something like:

```ts
if (tool === "read_file") {
  const firstLine = rawOutput.split("\n", 2)[0] ?? "";
  // Parse out path if your executor prefixes it
  return { step, tool, success, message: `Read file sequences.fasta (ok)` };
}
```

…and *only* fall back to rawOutput slicing when you don’t recognize the tool.

Your StepSummary plan + MAX_SUMMARIES = 3 is exactly what you want; just don’t let raw blobs sneak in.

---

## 3. Verification-gated completion: yes, but *only* on “done” signals

Your design is good:

* Only run `verifyTask` when:

  * FM uses `task_complete`, or
  * repeated-same-action >= 3.

I’d **not** run verification on every turn; that will be slow and noisy.

Two small tweaks:

1. When verification fails, give FM a very explicit, short nudge:

   ```ts
   history.push({
     step,
     tool: "verification",
     success: false,
     message: "Verification failed: your output does not meet the spec. Fix and try again."
   });
   ```

   That way you’re not just silently looping.

2. Consider a **hard stop** after verify-fail + N retries:

   * You already suggest `maxRetryAfterFailedVerify` – I’d set default = 2.
   * After that, return failure with `error: "Verification failed after retries"`.

Inline vs TB runner: I’d keep verification **inline in orchestrator** as you proposed; TB runner just provides `verifyTask` as a function. Clean and composable.

---

## 4. `/app` paths: I’d do prompt + lightweight normalization

What you wrote (system prompt rule) is a good first move:

> “When the task mentions `/app/foo`, use `foo` as the relative path.”

Given your logs (`cd /app`, `touch /app/jump_analyzer.py`), I’d also add one tiny safeguard in `run_command`:

```ts
let cmd = String(args.command);

// Rewrite obvious /app/ usages to relative
cmd = cmd.replace(/\/app\//g, "./");

// Optionally strip `cd /app &&` prefix entirely
cmd = cmd.replace(/^cd\s+\/app\s*&&\s*/g, "");

args.command = cmd;
```

That alone would make:

* `cd /app && ./configure ...` become `./configure ...`,
* `touch /app/jump_analyzer.py` become `touch ./jump_analyzer.py`.

You can keep it simple and conservative; you don’t need a full shell parser.

---

## 5. Skills vs tools: presentation is good, plus a safety net

Your “clarify skills vs tools” step is right: stop listing them in a `- Name:` format that looks like tools.

I’d:

* Change the heading to something like:

  ```txt
  Example approaches (for reference only; not tools):
    • Initialize a Bun project with typical TS config
    • Search for text patterns in files with grep-like tools
  ```

* And add one line in the system prompt:

  > “The only tools you may call are write_file, read_file, run_command, edit_file, task_complete. Skills listed in the prompt are not tools and must not be used in the `name` field.”

Optional extra: for TB2, you might even **turn off skill injection** initially (useSkills: false) until you’ve stabilised hints + verification. Mini-suite showed that skill text isn’t necessary to get basic behavior.

---

## 6. Two small things I’d add

### 6.1 Robustify `parseToolCalls` for TB (later, but note it)

You saw a couple of:

```text
[Worker] No tool call parsed, raw: <tool_call>{"name":"write_file","arguments":{"path":"/app/apply_macros.vim","content":"""\n...
```

That’s FM spewing triple-quoted nonsense. Not P1, but you’ll eventually want:

* A more forgiving parser that:

  * Accepts `<tool_call>{…}` possibly with junk after the JSON,
  * Trims trailing text until JSON parses.

You don’t have to put it in this fix plan, but I’d keep a TODO.

### 6.2 Suite-aware `useSkills` default

In your current code, `useSkills` defaults to `true` globally now. For this debugging phase, I’d:

* Default `useSkills: true` for **fm-mini**.
* Default `useSkills: false` for **TB2**, unless explicitly turned on.

That gives you a simpler surface while you stabilize hints/verification, then you can reintroduce skills as a controlled variable.

---

## 7. Answers to your review questions

> 1. Verification inline or TB runner?

Inline in the orchestrator with a `verifyTask` callback is the best tradeoff. It keeps the control loop tight and lets TB just provide the script.

> 2. Hint strategy for TB2?

Start with **zero generic hints**, then add a tiny number of **task-family-specific** hints where they’re obviously correct (e.g. “use `wc -w`” for word-count-like tasks). Definitely *remove* all fm-mini heuristics from TB2.

> 3. Skills: keep or remove?

For now: **keep skills for fm-mini**, but I’d either:

* Disable `useSkills` for TB2, or
* Keep them but reformat them as “approach descriptions”, and explicitly instruct FM they are *not* tools.

Given how noisy the first TB2 run is, I’d be slightly biased to **turn skills off for TB2** until you see stable behavior from hints/verification, then flip them back on and measure delta.

---

Overall: your plan is absolutely on target. With:

* suite-aware hints,
* StepSummary,
* verify-gated completion,
* `/app` soft normalization,
* and less confusing skill presentation,

you’ll get to a much cleaner “baseline FM TB2 behavior” where failures are about **domain difficulty**, not scaffolding bugs.
