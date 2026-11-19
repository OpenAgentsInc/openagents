# Coder compaction theory

This document describes how Coder keeps a long run inside the model's context,
what OpenAI Codex and T3 Code do instead, and what Coder should do next. It
also describes the compaction evals in `evals/coder-desktop/compaction/`.
GitHub issue #13706 tracks this work.

## The theory in two lines

Compact by structure, not by summary: Coder keeps every step twice, whole and
compacted, and moves the compacted copies into an append-only `earlier` list,
so compaction costs no model call and the cached prompt prefix survives it.
Keep what the model needs to finish in the working memory, which compaction
never touches, and tell the model exactly what left its context and how to get
it back.

## How Coder compacts today

Microcoder renders its whole state as one JSON object for every step
(`mc_state_json` in `coax/lib/microcoder/state.coax`):

```text
{"environment":…, "task":…, "route":…,
 ["left_out":N,] ["earlier":[…],] "actions":[…],
 ["memory":{"findings":…,"changed":…,"open":…},] "notes":…}
```

The order serves the prompt cache. Vertex serves a request's prefix from its
implicit cache when the prefix matches the previous request, so the parts that
never change come first and the parts that change every step come last:

- `environment`, `task`, and `route` never change during a run.
- `earlier` only grows at its end, except when it loses its oldest steps.
- `actions` only grows at its end between compactions.
- `memory` and `notes` change every step, so they come after `actions`.

### The two logs

Each step is written twice as it runs (`mc_action_begin`, `mc_action_command`,
`mc_file`, and `mc_action_edit_shown`): whole into `log`, and compacted into
`clog`. A compaction moves the compacted copies of the oldest steps from
`clog` to the end of `earlier` and drops their whole copies from `log`.
Nothing allocates, and no model call is made.

| Constant | Value | What it bounds |
| --- | --- | --- |
| `MC_LOG_BYTES` | 144 KB | The recent steps, whole (`log`) and compacted (`clog`). |
| `MC_ACTION_ROOM` | 48 KB | Free log a new step needs; with less, the oldest steps are compacted. |
| `MC_KEEP_RECENT` | 24 KB | The recent steps a compaction keeps whole, at most. |
| `MC_EARLIER_BYTES` | 40 KB | The compacted steps (`earlier`). |
| `MC_MAX_ACTIONS` | 512 | Steps the log can hold. |
| `MC_STEP_BUDGET` | 32 KB | What one step's results can add, so one step cannot flush the rest. |
| `MC_OUTPUT_HEAD`, `MC_OUTPUT_TAIL` | 6,000 B each | A whole command output's start and end. |
| `MC_BRIEF_RATIONALE` | 1,200 B | A compacted step's rationale. |
| `MC_BRIEF_OUTPUT` | 160 B each | A compacted command output's start and end. |
| `MC_FACTS_BYTES` | 128 KB | The environment, including earlier turns of the conversation. |
| `MC_REQUEST_MAX` | 256 KB | One request (`coax/lib/microcoder/loop.coax`). |

### When compaction runs

- **Before a step.** `mc_action_begin` calls `mc_compact(st, MC_KEEP_RECENT)`
  when the log has less than `MC_ACTION_ROOM` free or holds `MC_MAX_ACTIONS`
  steps. Compaction therefore runs once per 72 KB or more of new steps, not
  every step.
- **Before a request.** `mc_request` in `loop.coax` halves the log, then
  empties it, until the escaped state fits `MC_REQUEST_MAX`. Coder has no
  retry on a provider's context-length error; the byte bound stands in for it.

### What a compacted step keeps

| Result | Whole (`actions`) | Compacted (`earlier`) |
| --- | --- | --- |
| Rationale | Up to 2,000 B | Up to 1,200 B |
| Applied edit | Path, lines, and the new text, numbered | Path and lines |
| Failed edit | Path, why, and the file's nearest lines | Path and why |
| Command | Command, exit code, 6,000 B of the output's start and of its end | Command (300 B), exit code, 160 B of the output's start and of its end |
| Refused command | Command and why | Command and why (300 B each) |
| View | Path and content | Path and size in bytes |
| Missing file | Path and why | Path and why (300 B) |

The system prompt (`MC_SYSTEM` in `loop.coax`) tells the model that its
rationale stays and asks it to write there what it needs later: the path, the
line, and the text.

### When `earlier` is full

`mc_earlier_drop` drops the oldest compacted steps in one block, at a step
boundary, until `earlier` is half empty, and adds their count to `left_out`.
The model sees only that count. The steps themselves are gone.

### The working memory

The working memory (`coax/lib/agent/workmem/`, set with `mc_memory_set`) holds
three views: current findings (4,000 B), every edit and the lines it wrote
(2,000 B), and what still stops the run from finishing (1,500 B). Compaction
never touches it, so it is the one record that outlives `left_out`.

### Cross-turn recall

A run sees earlier turns of the conversation as `environment.conversation`:

- The TUI (`coder/tui/microcoder_keys.coax`) keeps the newest `MK_TURNS` (6)
  turns, each task and reply up to `MK_TURN_TEXT` (8,000 B). A cut text ends
  with `MK_CUT`, so the model knows its reply goes on. `mk_facts` drops the
  oldest turns first when the facts do not fit.
- The desktop app sends the turns of a chat you open again as an `MWW_RECALL`
  line (`coder_page_recall_line` in `apps/coder-desktop/page.coax`, read by
  `coder/tui/mcworker.coax`). It sends the newest 6 turns but cuts each
  message and reply at 1,200 bytes, with no marker and on a byte, not a
  character. Issue #13729 covers the fix.

### Measured behavior

`evals/coder-desktop/compaction/report.coax` runs four 200-step scenarios
against `state.coax` and prints these numbers. "Cache share" is the bytes each
prompt shares with the one before it, as a share of all prompt bytes.

| Scenario | Compactions | Evictions | Steps left out | In `earlier` | Largest prompt | Cache share |
| --- | --- | --- | --- | --- | --- | --- |
| Reader: a search, then 8–20 KB views | 34 | 2 | 103 | 94 | 146 KB | 79.6% |
| Notes: 1,190 B rationales, small commands | 3 | 3 | 143 | 31 | 136 KB | 96.9% |
| Fix loop: edit, then a 12 KB test output | 28 | 8 | 169 | 27 | 142 KB | 81.3% |
| Mixed: the three in turn | 22 | 6 | 164 | 31 | 140 KB | 84.6% |

Every scenario keeps the cache prefix at every step that does not evict, keeps
the step numbers continuous, and keeps the working memory. Two numbers stand
out: `earlier` holds only about 30 compacted steps of edit-and-test work, and
the notes scenario evicts on every compaction. Before the fix in finding 1,
the notes scenario crashed at its first compaction.

## Audit findings

1. **Fixed: a compaction larger than `earlier` lost steps and crashed the next
   prompt.** When the compacted copies of the steps to move exceeded
   `MC_EARLIER_BYTES`, `mc_compact` set the bytes to move to 0 but still
   counted the steps in `earlier`. The next `mc_state_json` sliced
   `earlier[1..<0]` and trapped. Long rationales with small commands trigger
   this within 80 steps, because a compacted step is then about as long as a
   whole one. A second path passed a negative size to `mc_earlier_drop`, which
   moved `nearlier` below zero. `mc_compact` now empties `earlier`, counts the
   oldest compacted steps that still do not fit as `left_out`, and moves the
   newest that fit. The state test "steps whose compacted copies overflow
   earlier go to left_out" and the eval C9 cover it.
2. **Desktop recall drops the whole conversation on a split character.** When
   byte 1,200 of a kept reply falls inside a multi-byte character, the recall
   line carries invalid UTF-8 and the worker refuses all of it. Eval C11 shows
   the failure and the fix (`text_byte_floor`). Issue #13729.
3. **`left_out` is opaque.** The model sees a count, not which steps, files,
   or edits left, and `MC_SYSTEM` does not explain the key. T3 Code lists the
   IDs it omits and names the tool that reads them back. Issue #13730.
4. **The budgets ignore the model's window.** The byte constants are the same
   for a 1M-token Gemini model and a 32K-token local model. `earlier` is 40 KB
   while the log is 144 KB, so a long fix loop keeps only its last ~30 steps
   even in compacted form, on models that could hold far more. Issue #13730.
5. **Corrections to the first draft of this document.** The desktop recall
   buffer is 256 KB (`App.swift`), not 4 KB; the 4 KB buffer is in a test.
   `loop.coax` has no provider context-length retry; the guard is the byte
   bound in `mc_request`. Adding the `left_out` key before `earlier` costs no
   extra cache: it changes only when an eviction already rewrites `earlier`.

## How Codex compacts

Reference: `~/work/projects/repos/codex` at `a098129` (2026-11-01).

- **Thresholds.** `ModelInfo::auto_compact_token_limit`
  (`codex-rs/protocol/src/openai_models.rs`) is 90% of the model's context
  window, or a configured lower limit. The usable window reserves 5% headroom
  (`effective_context_window_percent`, default 95). Codex counts tokens from
  the server's reported `input_tokens`, not bytes.
- **Summary handover.** `codex-rs/core/src/compact.rs` runs a summarization
  turn with the prompt in `codex-rs/prompts/templates/compact/prompt.md`
  ("Create a handoff summary for another LLM that will resume the task").
  `build_compacted_history` rebuilds the history as the initial context, then
  the newest user messages up to `COMPACT_USER_MESSAGE_MAX_TOKENS` (20,000),
  then the summary behind `summary_prefix.md`. Tool outputs do not survive
  unless a tool marks them (`compact_retained_tool_outputs.rs`).
- **Background compaction.** `codex-rs/core/src/session/async_compaction.rs`
  starts a speculative compaction request in the background once the context
  passes `async_compaction_threshold_percent` (default 65%) of the window, and
  uses it when compaction falls due, so you rarely wait for it.
- **Remote compaction.** `compact_remote_v2.rs` asks the Responses API to
  compact, which returns opaque compaction items.
- **Tool-output truncation.** Separate from compaction: a command's output is
  cut to `DEFAULT_MAX_OUTPUT_TOKENS` (10,000) by keeping its start and end, with
  a header that gives the original token and line counts
  (`codex-rs/utils/output-truncation/src/lib.rs`).
- **Context-length errors.** When a compaction request itself overflows, Codex
  removes the oldest history item and retries, to keep the cached prefix and
  the recent messages (`compact.rs`).
- **Cache.** Requests carry a `prompt_cache_key` per thread. A compaction
  replaces the history, so the cached prefix ends at the system prompt.

## How T3 Code compacts

Reference: `~/work/projects/repos/t3code` at `a6d2bed55b` (2026-11-18). T3 Code
drives other agents (Claude Code, Codex), so it schedules their compaction and
hands context between them rather than compacting itself.

- **Idle compaction before the cache expires.** `idleCompactionWindow` in
  `apps/server/src/orchestration-v2/IdleCompaction.ts` compacts an idle Claude
  thread 2 minutes before its prompt cache expires (1 hour on subscriptions, 5
  minutes on API keys). It skips threads under
  `COMPACTION_SUGGESTION_MIN_TOKENS` (200,000), because below that a cold
  re-read is cheap, and it never compacts a compaction.
- **A question on stale resume.** `packages/shared/src/claudeCompaction.ts`
  asks whether to compact a session that is old and large before it continues.
- **A budgeted handover.** `handoffBudget` in
  `packages/provider-core/src/server/handoffBudget.ts` counts one byte as one
  token, caps history at 16,000 tokens (`DEFAULT_HANDOFF_TOKEN_CAP`) and 64,000
  bytes, and reserves a quarter of the window, clamped between 16,000 tokens
  and half the window, for the work itself.
- **Intact items, in priority order.** `selectHistory` keeps whole items,
  never parts: the last user message, the last assistant message, the first
  user message, then the rest newest first.
- **Explicit omission.** The handover says how many items it left out, lists
  up to 20 of their IDs within an eighth of the budget, and names a tool
  (`t3_thread_read`) that reads an omitted item back. It also tells the model
  that historical material is context, not a new request.

## Comparison

| | Coder | Codex | T3 Code |
| --- | --- | --- | --- |
| Mechanism | Structural: two logs, append-only `earlier`, working memory | Model-written summary, or remote compaction | The agent's own compaction, plus a budgeted handover |
| Cost | No model call, under a millisecond | One model call, started early in the background | None for the handover |
| Trigger | Bytes: free log under 48 KB, request over 256 KB | Tokens: 90% of the window; background at 65% | 200,000 tokens and cache expiry; a provider switch |
| Cache | Prefix through `earlier` survives compaction | Prefix ends at the system prompt | Compacts while the cache is warm |
| What survives | Rationale, edit lines, exit codes, output ends, view sizes, memory | The summary and the newest 20,000 tokens of user messages | Whole items in priority order |
| Omission | A count (`left_out`) | Implicit in the summary | A count, IDs, and a tool to read them back |

## What Coder should do

### Principles

1. **Keep the prefix.** Every byte before the append point stays the same
   between steps and across compactions. Anything that changes every step goes
   after `actions`.
2. **Compact transcripts, keep evidence.** File bodies and long outputs can
   shrink to a path, a size, or their ends, because the model can read them
   again. Edits, failing checks, open requirements, and findings belong in the
   working memory, which never shrinks.
3. **Bound each step.** One result cannot push out the rest (`MC_STEP_BUDGET`).
4. **Say what left.** Whenever context is cut, held back, or evicted, the model
   sees what and how to get it back.
5. **Summarize only at eviction.** Structural compaction stays free. A
   model-written summary is worth its cost only when steps would otherwise
   leave `earlier` for good, or when you ask for one.

### Recommendations

| Priority | Where | Change |
| --- | --- | --- |
| Done | `coax/lib/microcoder/state.coax` | `mc_compact` handles a compaction larger than `earlier` (finding 1). |
| P0 | `apps/coder-desktop/page.coax` | Cut recall texts on a character boundary, at `MK_TURN_TEXT`, with `MK_CUT` (#13729). |
| P1 | `state.coax`, `loop.coax` | Replace the bare `left_out` count with an omission line: the step range that left and the files and edits it touched; explain it in `MC_SYSTEM`. Take T3 Code's wording that history is context, not instructions (#13730). |
| P1 | Coder Desktop, TUI | Turn the working memory on in every Coder run, so findings, edits, and open checks outlive eviction (#13730). |
| P2 | `state.coax`, `loop.coax` | Size `MC_EARLIER_BYTES` and `MC_REQUEST_MAX` from the model's window, and grow `earlier` first: compacted steps are the cheapest context per step (#13730). |
| P2 | `loop.coax` | Retry a provider's context-length error once with `mc_compact(st, 0)`, as Codex trims and retries (#13730). |
| P3 | `loop.coax` | Before an eviction, optionally write one model-made finding that sums up the steps about to leave, like Codex's handover summary, behind a setting so default runs stay free. |
| P3 | Coder Desktop | Compact or warn before a long idle chat's cache expires, as T3 Code does. |

## Evals

The suite lives in `evals/coder-desktop/compaction/`. It calls no model; every
byte is deterministic.

| ID | File | What it checks |
| --- | --- | --- |
| C1 | `compaction_test.coax` | Between compactions, each prompt starts with the one before it. |
| C2 | `compaction_test.coax` | Across compactions, the prefix through `earlier` stays the same. |
| C3 | `compaction_test.coax` | A compacted step keeps rationale, edits, failures, refusals, and commands. |
| C4 | `compaction_test.coax` | The working memory survives compaction and sits after `actions`. |
| C5 | `compaction_test.coax` | 36 steps with several compactions and an eviction stay valid. |
| C6 | `compaction_test.coax` | The 32 KB step budget keeps one noisy step from flushing the rest. |
| C7 | `compaction_test.coax` | The `MWW_RECALL` line round-trips; `mk_facts` drops the oldest turns first. |
| C8 | `compaction_test.coax` | A full flush (`mc_compact(st, 0)`) recovers and runs on. |
| C9 | `compaction_scenarios_test.coax` | Four 200-step scenarios: no prefix break, no gap in step numbers, every prompt valid and under the request limit, cache share at least 75%. |
| C10 | `compaction_scenarios_test.coax` | What compaction keeps and loses, and that only the memory remains after eviction. |
| C11 | `compaction_scenarios_test.coax` | A recall text cut on a character reaches the worker; one cut inside a character is refused. |
| C12 | `compaction_scenarios_test.coax` | The newest 6 turns, each 8,000 B, fit the run's facts whole. |

`scenarios.coax` holds the scenario driver that C9 and `report.coax` share.

Run the suite:

```sh
evals/coder-desktop/compaction/run.sh
```

Or run it with the other Coder Desktop evals, which write one JSONL record per
check to `evals/coder-desktop/results/`:

```sh
evals/coder-desktop/run.sh --tier compaction
```

When you change a compaction constant or the state's layout, run the suite and
update the measured table above from `report.coax`'s output.
