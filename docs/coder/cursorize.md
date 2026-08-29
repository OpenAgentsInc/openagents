# Cursorize: what Coder should take from the Cursor agent

Date: 2026-08-29. Evidence: one full Cursor agent session on this repository
(`~/.cursor/projects/Users-christopherdavid-work-openagents/agent-transcripts/7822942d-0768-4da1-8944-c1e44d9f0fec/`,
685 lines, 1.1 MB, 45 user turns, 10:44–15:14) beside one full Coder ATIF
export from the same day (Autopilot smoke test, 13 steps, ~7 min). The Cursor
session is the same person, the same repo, the same week — the Psionic →
qwen35 → Ollama-parity arc that produced `docs/psionic/*` and issues
#345/#346/#347 — so the behavioral deltas are apples to apples.

Every recommendation below names the file it would touch and the metric that
would falsify it. Adoption goes through the runbook loop: one change, the
same suite, the delta, then a ledger entry in `best-practices.md`. All items
are `proposed` until measured.

## What the numbers say

| Behavior | Cursor | Coder |
|---|---|---|
| Tool calls per assistant round | ~2.6 avg, max 11; 643 of 684 turns carry tool_use | ~1; the smoke session issued 9 calls in 13 steps, two of them pure sleeps |
| Inter-round narration | median ~150 chars ("Run 1 failed. Starting runs 2 and 3.") | multi-paragraph reasoning then report |
| Long waits | `Shell` takes `block_until_ms`; separate `AwaitShell` polls later with no model round | `bash` blocks up to `timeout_seconds`; the ATIF `waste` block attributes ~470 s to `echo`/`ps`/`tail` sleep-polls around one background process |
| Subagents | 34 `Task` transcripts, each with `UpdateCurrentStep` progress and its own tool pool | `delegate` with fan-out to 32, but the export's `extra.subagent` was empty `{}` — child work is invisible in the parent record |
| Plan tracking | `TodoWrite` 11× | `goal` tool exists, unused in the sampled session |
| Web | built-in `WebSearch`/`WebFetch` | none in the tool list |
| Transcript fidelity | tool results absent; only 3 `turn_ended` lines; cancellations invisible | ATIF records observations, reasoning, outcomes, and a `waste` audit |

## R1. One round, many calls — `proposed`

T1 in `best-practices.md` already says batch independent commands into one
*shell call* with `&&`. The harness already accepts multiple tool calls in
one assistant round (this document's research used it). What is missing is
the model-level instruction: our prompt surfaces only ever mention shell
concatenation, so the sampled Coder behavior is one call per round.

- **Change:** extend `CODER_CONCISION` and `RUST_BASH` in
  `crates/openagents-cli/src/surfaces.rs` with one sentence: independent
  tool calls go out together in one round — a batch of reads, greps, and
  shells costs one round trip, not one per call; only calls whose inputs
  depend on earlier output wait.
- **Measure:** add `tool_calls_per_round` mean and max to ATIF
  `final_metrics`. Target: mean ≥ 2 on research-heavy suites, without a rise
  in failed-call rate (batching must not batch dependent calls).

## R2. Background shell and a zero-model wait — `proposed`

The largest measured waste in the Coder session was scaffolding, not work:
`nohup … &`, then five rounds of `sleep N; ps; tail` to watch one Autopilot
process. Cursor avoids this with a shell that can start work and return
immediately plus a dedicated await tool.

- **Change:** give the `bash` tool a background mode that returns
  `{job_id, log_path}` immediately, and a companion `bash_wait` tool that
  parks at the turn boundary — the mechanism `swarm_wait` already has —
  until the job exits or a timeout, then returns the log tail and exit
  status. Both host-side in the Rust tool runner; no model round is spent
  while parked.
- **Where:** the shell tool runner in `openagents-cli`, the new tool
  description in `surfaces.rs`, and a line in `RUST_BASH` pointing at the
  background mode for anything expected to run over ~30 s (test suites,
  builds, `--autopilot`).
- **Measure:** the `waste` block. Target: repeated-command-head waste for
  `sleep`/`ps`/`tail`-style polling rounds near zero on sessions that launch
  long processes.

## R3. Waste lint as a live nudge — `proposed`

The ATIF export computes `waste.repeated_command_heads` after the fact. The
same detector, run during the session, is a cheap host-side correction: when
a command head repeats past a threshold (3), append one line to the next
tool result — "this is the 4th `sleep`-poll this session; use background
bash and `bash_wait`, or read the log you already have."

- **Change:** reuse the existing waste classifier on the session's own
  command history; emit the nudge from the host, not the model.
- **Why host-side:** per the #342 ruling, the CLI annotates and never blocks
  or rewrites input. A nudge inside a tool result is annotation.
- **Measure:** sessions that hit the threshold should show a drop in
  subsequent repeats of that head.

## R4. Terse inter-round narration — `proposed`

Cursor narrates between tool rounds in one short line naming the expected
observation, and puts paragraphs only in the final user-facing report. Our
concision instruction governs the report well but says nothing about the
inter-round text that also costs output tokens every round.

- **Change:** one sentence in `CODER_CONCISION`: between tool rounds, at
  most one line saying what the next batch should establish; the report at
  the end is where prose belongs.
- **Measure:** add `median_inter_round_text_chars` to ATIF
  `final_metrics`. Target: median ≤ ~200 chars on tool-heavy turns, with no
  regression in issue-closure rate (terse must not mean silent about
  mistakes).

## R5. Subagent telemetry and child progress — `proposed`

`delegate` already covers Cursor's `Task` (per-child tool pools, worktree
isolation, fan-out to 32), but two things are weaker: the parent's ATIF
export left `extra.subagent` empty, and a parent watching children has
nothing like Cursor's `UpdateCurrentStep` progress line.

- **Change (record):** when a delegated child registers, record its session
  id, lane, and prompt label into the parent's session metadata so the ATIF
  exporter fills `extra.subagent` instead of `{}`.
- **Change (progress):** make a child's first checkpoint publish its swarm
  status automatically (unit, issue, done-when), and have `swarm_list
  --tree <parent>` show those one-liners. That is `UpdateCurrentStep` with
  machinery we already have — checkpoints and the status field — and it
  also makes stale children visible.
- **Measure:** any session that delegated has a non-empty `extra.subagent`
  in its export; zero model calls added.

## R6. Web fetch and search as sandboxed capabilities — `proposed`

The Cursor session used `WebSearch`/`WebFetch` once each, but on this
repository's work (reading upstream docs, checking Ollama/llama.cpp
behavior) web access is routine. Coder has the right container for this
already: digest-pinned WASM capabilities.

- **Change:** add `web_fetch` and `web_search` capabilities under
  `plugins/` with the same manifest discipline as `test_report` and
  `git_facts`; load them per-session like any capability rather than
  granting every session network reach.
- **Measure:** capability provenance in `tool.ran` steps makes their effect
  attributable per the autoimprove contract; adoption gated on a suite where
  tasks require reading upstream documentation.

## R7. Promote the goal tool where Cursor used TodoWrite — `proposed`

Cursor wrote an 11-item plan with `TodoWrite` and updated it as flake re-runs
progressed. Our `goal` tool carries objective, status, and budget with every
turn, which is strictly more useful — but the sampled session never called
it, and nothing in the prompts suggests it at task start.

- **Change:** one sentence in `CODER_BUDGET` or the goal tool description:
  on a prompt spanning more than one unit of work, register the units as a
  goal first, then report against them as they complete or block.
- **Measure:** multi-issue sessions with a registered goal vs. without,
  compared on reports-per-issue accuracy at budget cutoff (the smoke
  session died mid-poll with no record of what remained).

## R8. What not to adopt

- **Cursor's transcript thinness.** Its export drops every tool result, has
  no model or token attribution, and shows only three `turn_ended` lines for
  4.5 hours — cancellations and failures are unrecoverable. ATIF's
  observations, reasoning, turn outcomes, and waste audit are the reason the
  autoimprove loop can run at all. Keep the fidelity; the numbers above came
  from it.
- **`GetDynamicTools`/MCP namespacing.** Our capability loader covers the
  same ground with digest pinning. Revisit only if a concrete MCP-only
  integration is needed.
- **Narration-as-progress theater.** Cursor's terse lines work because they
  precede real parallel batches; terse narration without R1 would just be
  quieter stalling.

## Measurement plan

Land one recommendation at a time. Before each: record the current suite row
(`bench/suites/`, pinned digests, same thresholds). After: same suite, and
read three numbers from the ATIF pair — `tool_calls_per_round`, the `waste`
block, `median_inter_round_text_chars` — plus the score. Keep what moved the
score without raising cost; write the falsifiable entry in
`best-practices.md` (T-series for process levers) with this document as
provenance. R2 and R3 are the predicted largest movers: the smoke session
spent most of its wall clock on wait scaffolding that neither change allows.
