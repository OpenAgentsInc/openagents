# Cursorize: what Coder should take from the Cursor agent

Date: 2026-08-29. Evidence: the full PSIONIC Cursor conversation
(`7822942d-0768-4da1-8944-c1e44d9f0fec`), reconstructed from the canonical
store — `globalStorage/state.vscdb` → `cursorDiskKV`, 2,981 in-order bubbles
(43 user, 2,938 assistant; 2,049 tool calls) across 2.28 hours — beside one
full Coder ATIF export from the same day (Autopilot smoke test, 13 steps,
~7 min). The agent-transcripts JSONL (685 lines, 1.1 MB) is an agent-facing
export that carries tool **requests** but zero tool **results**; the
`cursorDiskKV` bubbles carry both (`toolFormerData.result` on 1,653 of 2,049
calls, the rest pruned into `toolCallBinary`), plus per-call status (2,019
completed, 29 error), timestamps, and todo state. All Cursor numbers below
are computed from the ordered bubbles, not the JSONL.

The two sessions are the same person, the same repo, the same week — the
Psionic → qwen35 → Ollama-parity arc that produced `docs/psionic/*`, the
crates under `crates/psionic-gguf/`, and issues #345/#346/#347/#358 — so the
behavioral deltas are apples to apples. The conversation ends with a fully
autonomous final turn: after the last user message, 175 more tool calls
(70 edits, 24 shells, including `git push github/openagents` and an
`openagents issue comment 358` claim-status post) with only 9 short text
bubbles. That is the behavior to reproduce.

Every recommendation below names the file it would touch and the metric that
would falsify it. Adoption goes through the runbook loop: one change, the
same suite, the delta, then a ledger entry in `best-practices.md`. All items
are `proposed` until measured.

## What the numbers say

| Behavior | Cursor (PSIONIC, from `cursorDiskKV`) | Coder (ATIF export) |
|---|---|---|
| Tool calls | 2,049 over 2.28 h: read 742, edit 510, rg 372, shell 270, glob 72, task 34, todo_write 11, await 11 | the smoke session issued 9 calls in 13 steps, two of them pure sleeps |
| Parallel batching | 237 batches of 2–8 calls issued in the same instant (110×2, 72×3, 26×4, 13×5, 11×6, 3×7, 2×8, plus five larger flush groups up to 36); single-call rounds still the mode at ~1,341 | ~1 call per round; the smoke session batched nothing |
| Round anatomy | tool-heavy turns run long and quiet: tools per user-turn median 17, mean 47, max 350; narration median 150 chars, 86 of 120 text bubbles ≤ 200 chars | multi-paragraph reasoning then report |
| Long waits | `await` tool takes `blockUntilMs` (sample: `{taskId, blockUntilMs: 600000}`) and parks with no model round | `bash` blocks up to `timeout_seconds`; the ATIF `waste` block attributes ~470 s to `echo`/`ps`/`tail` sleep-polls around one background process |
| Reads | 742 `read_file_v2`, **0 with a line range** — whole files, relying on the harness's cheap re-read | same habit in ours; `Read` returns whole files by design |
| Self-correction | 29 tool errors (10 shell, 10 edit, 5 read), absorbed inline and retried | similar; ATIF records the retry cost |
| Subagents | 34 `task_v2` calls, each returning `{agentId}` mapped to a `subagents/*.jsonl` thread with `UpdateCurrentStep` progress | `delegate` with fan-out to 32, but the export's `extra.subagent` was empty `{}` — child work is invisible in the parent record |
| Plan tracking | `todo_write` 11× with dependencies and status transitions (`TODO_STATUS_IN_PROGRESS` → …) | `goal` tool exists, unused in the sampled session |
| Token accounting | none in the bubbles (`tokenCount` all zero) — Cursor does not surface its own metering | ATIF carries per-step tokens; the store rows carry billed totals |
| Transcript fidelity | full results in the DB (the JSONL is the lossy export); status `completed`; turn timestamps | ATIF records observations, reasoning, outcomes, and a `waste` audit — parity with the DB, not the JSONL |

## R1. One round, many calls — `proposed`

T1 in `best-practices.md` already says batch independent commands into one
*shell call* with `&&`. The DB reconstruction sharpens the target: Cursor's
batching is not shell concatenation but multiple tool calls fired in the
same model call — 237 same-instant batches of 2–8 (and flush groups to 36),
concentrated exactly in the recon phases: bursts of `read_file_v2` +
`ripgrep_raw_search` + `glob_file_search` together. Median tools per
user-turn is 17; the mode of *rounds* is still single-call. So the pattern
is "single call when one suffices, wide bursts at phase boundaries" — not
uniform parallelism.

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
process. Cursor avoids this with a dedicated `await` tool: raw args
`{"taskId": "437530", "blockUntilMs": 600000}` — the model parks a whole
user-turn (the heaviest turn ran 24 minutes wall) with **zero model calls**
until the task completes or the block expires. It called it 11 times. Note
this complements R1: batching cuts model rounds when calls are independent;
`await` cuts them when the next step needs one long-running thing to finish.

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

The DB confirms the JSONL picture with better coverage: 120 assistant
text-only bubbles, median 150 chars, 86 of them ≤ 200 chars, mean 389 —
and the heaviest autonomous turn (175 tool calls, 70 edits) carried only 9
text bubbles. Cursor narrates between tool rounds in one short line naming
the expected observation, and puts paragraphs only in the final user-facing
report. Our concision instruction governs the report well but says nothing
about the inter-round text that also costs output tokens every round.

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

## R7. Promote the goal tool where Cursor used todo_write — `proposed`

The DB shows `todo_write` 11× with a structured schema: id, content, status
(`TODO_STATUS_IN_PROGRESS`/`…_PENDING`/done), createdAt/updatedAt, and
dependencies between todos — a real DAG, updated as work progressed across
a 2.3-hour session. Our `goal` tool carries objective, status, and budget
with every turn, which is strictly more useful — but the sampled session
never called it, and nothing in the prompts suggests it at task start.

- **Change:** one sentence in `CODER_BUDGET` or the goal tool description:
  on a prompt spanning more than one unit of work, register the units as a
  goal first, then report against them as they complete or block.
- **Measure:** multi-issue sessions with a registered goal vs. without,
  compared on reports-per-issue accuracy at budget cutoff (the smoke
  session died mid-poll with no record of what remained).

## R8. What not to adopt

- **Cursor's export thinness.** The agent-facing JSONL drops every tool
  result, has no model or token attribution, and shows only three
  `turn_ended` lines for a 2.3-hour session — cancellations and failures are
  unrecoverable from it. (Cursor's own canonical store, `cursorDiskKV`, does
  keep results and statuses — 1,653 result payloads, 29 errors — but none of
  it reaches the export, and `tokenCount` is zeroed even in the DB.) ATIF's
  observations, reasoning, turn outcomes, and waste audit are the reason the
  autoimprove loop can run at all. Keep the fidelity; the numbers in this
  document came from reconstructing the DB for exactly this reason.
- **`GetDynamicTools`/MCP namespacing.** Our capability loader covers the
  same ground with digest pinning. Revisit only if a concrete MCP-only
  integration is needed.
- **Narration-as-progress theater.** Cursor's terse lines work because they
  precede real parallel batches; terse narration without R1 would just be
  quieter stalling.

## Measuring each change: the rails already exist

This repository already built the instrument for exactly this question. The
loop is `docs/coder/autoimprove.md` (the plan and the verification law),
`docs/coder/runbook.md` (the operating procedure), and
`docs/coder/best-practices.md` (the falsifiable ledger). The substrate:

- **Harbor + Terminal-Bench 2.0** through `bench/adapters/openagents_coder.py`,
  which builds the working-tree CLI, installs it in each trial container, and
  copies the trial's native ATIF export out as `trajectory.json`. A verifier
  in the environment decides pass/fail; the agent's claim never does.
- **Pinned suites** (`bench/suites/*.suite.json`): tasks pinned by content
  digest, not name. A run that skipped a pinned task is a smoke run the store
  refuses to record as a score.
- **An append-only, hash-chained store** (`bench-results/*.jsonl`) with a
  deliberate regression row kept in `tb2-quick.jsonl` to prove the floor
  fires. Threshold edits are their own change (ledger M4) — never in the same
  commit as a run they would flatter.
- **A citation-checked review loop** (`pnpm run coder:review`): a separate
  reviewer conversation over the trial artifacts, where every claim must cite
  `trial:<task>#step-<id>` refs that resolve, or the review is refused whole.
- **Prior art:** the T1/T2/T3 cycles on 2026-08-28 measured batching,
  `--stat`-before-`-p`, and lane-aware verbosity exactly this way — T1
  with/without on `git-leak-recovery` showed +52% billed tokens without the
  sentence; T3 cut completion tokens 41% (`reviews/2026-08-28-t1-t2-t3-git-leak.md`).
  The plugin A/B disposition (`plugin-ab-disposition.json`) is the same
  pattern for tool availability.

So the Cursorize program does not need a new harness. Each R-item becomes a
runbook §4 cycle: same suite, same model, same lane, one lever, ATIF pair,
store rows, cited review, ledger entry.

### Suites and their roles

| Suite | Tasks | Role in this program |
| --- | --- | --- |
| `tb2-quick` | 2 (`regex-log`, `openssl-selfsigned-cert`) | The iteration lane. Two trials, minutes per run, gate-floored. A delta here is a **selector, not a conclusion** (ledger M5: rates of 0/.5/1 only). |
| `plugin-ab-test` | 1 (`build-cython-ext`) | The compile-fix loop; the R1/R2 round-count oracles in miniature. |
| `plugin-ab-git` | 2 (`git-leak-recovery`, `sanitize-git-repo`) | High-output-volume forensics; where the quadratic-replay cost shows. T1/T2 were measured here. |
| `tb2-cross-section` | 12 | The confirmation lane, only after a quick-lane selector fires. Includes `openssl-selfsigned-cert` — "the purest batching discriminator in the set" — and `sqlite-with-gcov` for output volume. |
| `tb2-quick` local lane | 2 | Unmetered; wall clock and output tokens are the axes. The R4/R5 falsification lane when proxy spend should stay flat. |

### What one cycle looks like

Concretely, R1 (parallel tool-call rounds) as the worked example:

1. **Baseline** (unchanged tree, no new lever):

   ```sh
   bench/run-suite.sh bench/suites/tb2-quick.suite.json \
     --model openai/gpt-5.6-luna --lane proxy \
     --jobs-dir /tmp/gym-jobs/cursorize-r1-baseline
   openagents gym results score /tmp/gym-jobs/cursorize-r1-baseline/<run> \
     --suite tb2-quick --lane proxy --append
   openagents gym results trend tb2-quick
   ```

2. **Lever**: the one-sentence prompt change in `surfaces.rs` (R1's change),
   in a fresh worktree; repack the CLI tarball so the arena runs it. CLI
   version and surface digests pin into every row, so baseline and lever rows
   differ in exactly that axis.
3. **Re-run**: identical command, new jobs-dir. Nothing else varies.
4. **Read the ATIF pair**, not only the score: `tool_calls_per_round`,
   billed prompt tokens, rounds, the `waste` block, plus the verifier
   verdicts. Process levers move cost before they move success rate —
   the T1 review is the template for reading a token delta without a
   score delta honestly.
5. **Review** with `pnpm run coder:review -- <job-dir> --suite tb2-quick \
   --lane proxy --lever <diff-ref> --slug cursorize-r1 --reviewer-model <other-family>`.
   Every proposal cites trajectory steps; unresolvable citations refuse the
   review.
6. **Decide and record**: keep (append ledger entry, promote R1 to `adopted`
   in this document), revert (keep the row, write the refutation), or
   escalate to `tb2-cross-section` when the quick delta encourages but
   n=2 cannot conclude.

### Per-recommendation measurement contracts

Each R-item gets its oracle and its expected axis stated **before** the run
(runbook §3: if you cannot name the measuring suite, the lever is not ready).

| Lever | Oracle suite (selector → confirm) | Axis that confirms | Axis that would refute | Metric gaps to close first |
| --- | --- | --- | --- | --- |
| R1 parallel rounds | `tb2-quick` → `tb2-cross-section` (`openssl-selfsigned-cert` is the batching discriminator) | rounds ↓, billed tokens ↓ at equal success | success ↓, or tokens flat while failed-call rate rises | none — ATIF steps already carry call counts |
| R2 background bash + zero-model wait | `plugin-ab-test` (`build-cython-ext` compile loop) → cross-section | wall clock ↓, sleep/poll rounds → 0 | rounds ↑ without wall-clock gain | none — wall clock is in every row |
| R3 live waste nudge | `tb2-quick` local lane (unmetered; output tokens = minutes) | repeated-command-head waste ↓ in ATIF | nudge fires but waste unchanged | none — the classifier exists |
| R4 terse inter-round narration | `tb2-quick` local, then proxy | completion tokens ↓ (T3-style), wall clock ↓ on local | success ↓ on `fix-code-vulnerability` (terse hides reasoning errors) | add `median_inter_round_text_chars` to ATIF `final_metrics` |
| R5 subagent telemetry | no behavior change claimed | `extra.subagent` populated in exports | — | telemetry only; not measurable on suites |
| R6 web capabilities | no honest tb2 oracle today (`allow_internet = true` makes web available to every trial) | needs a designated offline→online task pair first | — | per `plugin-ab-disposition.json`: `no_oracle_yet` until an oracle is named |
| R7 goal registration | cross-section task 12 (`schemelike-metacircular-eval`, long horizon) | fewer abandoned/timeout-shaped failures | overhead tokens with no completion change | requires the task to run at all under Rosetta; budget the 2400 s verifier |

### Where the evidence lives (Cursor side)

The PSIONIC conversation's canonical store is not the JSONL. To re-derive any
number in this document:

```sh
DB="/Users/christopherdavid/Library/Application Support/Cursor/User/globalStorage/state.vscdb"
# conversation index: bubble order, title, status
sqlite3 "file:$DB?mode=ro" "SELECT value FROM cursorDiskKV \
  WHERE key='composerData:7822942d-0768-4da1-8944-c1e44d9f0fec';"
# each bubble: text, toolFormerData {name, params, rawArgs, result, status}, createdAt
sqlite3 "file:$DB?mode=ro" "SELECT value FROM cursorDiskKV \
  WHERE key LIKE 'bubbleId:7822942d-0768-4da1-8944-c1e44d9f0fec:%';"
```

Walk `fullConversationHeadersOnly` for order; ~4,100 of 7,068 bubble rows are
orphaned retries not in the header list — count only headered bubbles.
Results can be pruned into `toolCallBinary` (base64 protobuf,
`additionalData.isPruned`), so a naive `result`-only read undercounts
(1,653 of 2,049 here). The Coder-side equivalent is one ATIF export plus
`openagents trace show` — no reconstruction needed. The asymmetry is itself
a finding: our agent-facing transcript is the better archival format.

### Threats to validity, from the runbook's stop rules

- **Lane flakiness is not a delta.** Flash proxy trials have died on
  container→proxy misses (`regex-log` 502s, three store rows). A 0-success
  row on a lane that previously scored is a lane signature; re-run before
  reading it as the lever's effect.
- **Emulation noise.** Under qemu the verifier segfaults; Rosetta (or a
  cloud `--env`) is a prerequisite for any graded run. Timeout-shaped
  failures get `--timeout-multiplier 2.0` before they count.
- **n=2 arithmetic.** `tb2-quick` admits three success rates. Treat quick
  rows as selectors; only a cross-section row (or a repeated quick pair)
  concludes. The store's threshold gates enforce this for score tier.
- **Cost honesty.** Local-lane rows price as `null` (`unmetered_local_lane`),
  and cached-token splits are missing on proxy rows, so dollar figures are
  ceilings. Compare levers on rounds, tokens, and wall clock first.
- **The store is the conscience.** Never edit `bench-results/*.jsonl`;
  append through `gym results score --append`, keep regression rows, and
  never change a threshold in the same change as the run it flatters.

### Sequencing

1. **R1 first**: prompt-only change (no harness work), the suite with a
   named batching discriminator, and prior T1 art to compare against.
2. **R2 + R3 together are the predicted largest movers** (the smoke session
   burned most of its wall clock on wait scaffolding), but R2 is Rust tool
   work — land it as its own cycle, then measure R3 on top so their effects
   do not confound.
3. **R4** reuses the T3 measurement pattern directly.
4. **R5–R7** trail: telemetry, oracle-less, and long-horizon respectively —
   each waits for its measurement gap to close.

Every adopted item flips to `adopted` in this document with its store rows
and review file as provenance; every refuted one stays, struck through, so
the loop does not rediscover it.
