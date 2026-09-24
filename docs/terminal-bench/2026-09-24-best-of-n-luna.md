# Best-of-N Luna, selected by the combined verdict

2026-09-24, on the `coderos` benchmark host. This is the first measurement
of algorithm 6 of the [Luna pivot](../coder/design/luna-pivot.md), issue
[#9587](https://github.com/OpenAgentsInc/openagents/issues/9587): does
best-of-N GPT-6 Luna, kept by the combined verdict from
[truthful checks](2026-09-24-truthful-checks.md), pass more tasks than one
Luna attempt, at a cost still far below one Opus trial?

**Status: incomplete, and not a result.** The policy option shipped and
ran end to end on mini-tasks and in Terminal-Bench 4.0 task containers.
The matched TB4 experiment ran for 34 minutes and graded 8 of its 100
planned trials before it stopped: first the host's disk filled, and then
the operator restricted the night's runs to Microluna, so every arm that
ran Luna through the Codex CLI was stopped. The live measurement will
rerun on Microluna. What follows is what exists, stated with how little
it rests on.

## Summary

- **`control.best_of` works.** N candidates of the first executor run at
  once, each in its own copy of the workspace, and the host checks each
  candidate in the real workspace, asks the verdict about its final
  report, and keeps the best verdict, then the best check scorecard, then
  the lowest cost. It ran with Codex on Luna and with Microluna, on
  mini-tasks and in TB4 containers, and recorded every candidate, its
  verdict, and the pick.
- **Nothing passed on TB4.** All 8 graded trials failed: 0 of 2 for one
  Luna candidate, 0 of 1 for best of 3, 0 of 1 for best of 5, 0 of 3 for
  one Microluna candidate, and 0 of 1 for Microluna best of 3. No
  comparison between arms is possible from this.
- **One oracle hit, lost by the selection.** On
  `mvcc-lsm-compaction`, the Luna best-of-3 trial's first candidate
  passed the verifier (15 of 15 tests), and the other two failed 4 of 15.
  All three verdicts were `unknown`, all three passed the same 2 check
  scenarios, and the tie went to cost, which kept a failing candidate.
  On the one trial whose candidates are all graded, generation had what
  the selection didn't find.
- **The verdict was silent where it mattered.** On all 11 TB4 candidates
  of the Luna best-of arms, the verdict said `unknown`. That matches the
  truthful-checks finding: Codex's final reports on Luna are short, so
  the report questions have little to read. On Microluna's reports it
  spoke: it called `fail` on 5 candidates, and all 5 were in failed
  trials.
- **Spend was small.** The TB4 trials cost $0.1516 in all (a lower bound:
  two Microluna trials have one unpriced call each), of which Jev was
  $0.0193. The mini-tasks cost $0.0873, Jev $0.0059. No Claude quota was
  used.

## What shipped

| Commit | What |
| --- | --- |
| 7e2ecfefeb | `control.best_of` in Coder One (`compose/best_of.rs`), the five manifests below, and `gym coder composition --best-of` |
| 678caf4198 | `coder-one minitask setup` and `grade`, so any policy runs on a mini-task through `coder-one episode run` |
| a40e88a04c | `--best-of` names arms by the experiment's job |

The manifests, in `crates/coder-one/policies/`:

| Manifest | What it is |
| --- | --- |
| `luna-best-of-1.json` | `tunable-luna-pack-solo` with `verify.verdict` on, and without `control.monitor` and `verify.repair`, so the Luna arms differ only in N |
| `luna-best-of-3.json`, `luna-best-of-5.json` | The same, with `control.best_of.n` 3 or 5 |
| `microluna-best-of-1.json`, `microluna-best-of-3.json` | `microluna-v1` with `verify.verdict` on and without `verify.repair`, with N 1 or 3 |

How a candidate is kept, and what is recorded:

- **Isolation.** Each candidate works in a private copy of the workspace
  under `/tmp`. Every mention of the workspace's path in its briefing, and
  for Microluna in the task and evidence it reads, is rewritten to its
  copy's, and a closing section says where it works. A workspace over
  `max_copy_mb` (256 MiB) runs one candidate and says why.
- **Selection.** The verdict's call ranks first (pass, then unknown, then
  fail; a session without an answer ranks last), then the check
  scorecard (fewer failed scenarios and contradicted requirements, then
  more confirmed requirements), then the priced cost, then the lowest
  number.
- **Record.** `composition.json` holds `best_of`: each candidate's status,
  cost, time, checks, verdict with its evidence, score, and archive, the
  kept number, why, and whether the real workspace changed while the
  candidates ran. Every candidate is a `delegate` call and a
  `candidate-N` dispatch, so each is charged.
- **Oracle.** Each candidate's workspace is archived as
  `best-of/candidate-N.tar.gz` in the episode. `tbench verify --task T
  --candidate DIR` grades one, and the rewards go in
  `best-of/grades.json`, which `gym coder composition --best-of` reads to
  report the oracle rate and the selection's accuracy.

## Mini-tasks

Each mini-task ran through `coder-one episode run` under a `bwrap`
boundary on the host (a read-only root, with only the run directory,
`/tmp`, and Codex's state writable), then the mini-task's grader graded
the result and each archived candidate. One run per cell, Jev live, the
host's Codex login.

| Task | Arm | Grade | Candidates passing | Kept | Verdicts | Cost | Time |
| --- | --- | --- | --- | --- | --- | ---: | ---: |
| `git-recovery` | Luna best of 3 | passed | 3 of 3 | 3 | pass, pass, pass | $0.0031 | 22 s |
| `log-severity` | Luna single | failed | — | — | — | $0.0074 | 150 s |
| `log-severity` | Luna best of 3 | failed | 0 of 3 | 2 | unknown, **pass**, unknown | $0.0111 | 190 s |
| `log-severity` | Microluna best of 3 | failed | 0 of 3 | 1 | no answer ×3 | $0.0008 | 95 s |
| `interactive-terminal` | Luna single | passed | — | — | — | $0.0053 | 474 s |
| `interactive-terminal` | Luna best of 3 | passed | 3 of 3 | 3 | unknown, unknown, pass | $0.0286 | 336 s |
| `interactive-terminal` | Microluna best of 3 | passed | 3 of 3 | 2 | pass, pass, pass | $0.0182 | 239 s |
| `cancel-cleanup` | Luna single | passed | — | — | — | $0.0033 | 73 s |
| `cancel-cleanup` | Luna best of 3 | passed | 2 of 3 | 2 | pass, pass, pass | $0.0095 | 95 s |

Time is the episode's wall time, which for best of N is the slowest
candidate's plus the per-candidate checks. Cost includes Jev.

- **The checks carried the one real choice.** On `cancel-cleanup`,
  candidate 1 failed the grader. The verdict called all three `pass`,
  but candidate 1's checks failed 2 scenarios and the others' passed 6,
  so the scorecard kept a passing candidate.
- **The verdict passed a failure.** On `log-severity`, it called
  candidate 2 `pass` (the only non-`unknown` call), and every candidate
  failed the grader. The checks passed all 6 scenarios on all three, so
  nothing in the ranking could tell.
- **Microluna's `log-severity` run lost its provider.** All three
  Microluna sessions ended with `transport: the stream broke`, which is a
  Microluna transport failure, not a candidate's work. The remaining
  mini-task cells didn't run: the host's disk filled, and then the
  operator stopped Codex runs.

## The TB4 experiment

### Protocol

| Field | Value |
| --- | --- |
| Tasks and why they were chosen | The #9583 subset, minus the tasks a workspace copy can't isolate: `heat-pump-warranty` and `legacy-utility-triage` keep their state in service containers, `shadow-relay` writes its artifact to `/tmp`, and `uefi-bootkit`'s disk images exceed the copy bound. That leaves 10: `mvcc-lsm-compaction`, `ks-solver-cpp`, `wal-recovery-ordering`, `cad-model`, `nextjs-performance`, `embedding-drift-monitor`, `fin-saccr-rwa`, `sound-change-cascade`, `wdm-design`, and `coq-block-bound`. Fixed before any trial ran. |
| Arms | Baseline `luna-bo1`; candidates `luna-bo3` and `luna-bo5` on the `coder-one-tunable-luna-pack` profile, and `micro-bo1` and `micro-bo3` on `coder-one-microluna-v1`, each with the manifest above. Artifact `coder-one-9587-678caf4198` (SHA-256 `28c82a6c…3f4cef`), built from 678caf4198. |
| Held fixed | Codex CLI 0.155.1, `gpt-6-luna`, the long-task effort `high`, the Jev briefing with the coverage packer, `verify.checks`, `verify.verdict`, the task's 8-hour timeout, and its verifier. |
| Varied | N, and Codex against Microluna. |
| Stopping rule | Early stopping at alpha 0.05 against `luna-bo1`, 2 attempts per task. It never fired: the operator stopped the experiment first. |
| Quota budget | `--quota-usd 0.01`. No arm draws on Claude. Luna's cost is a price estimate from token counts. |

### Results

8 of 100 planned trials were graded. Every graded trial failed.

| Arm | Graded | Passed | Oracle (any candidate passed) | Selection accuracy | Cost | Cost per pass | Mean agent time |
| --- | ---: | ---: | --- | --- | ---: | --- | ---: |
| `luna-bo1` | 2 | 0 | 0 of 2 | — | $0.0247 | — | 3m51s |
| `luna-bo3` | 1 | 0 | 1 of 1 | 0 of 1 | $0.0158 | — | 2m21s |
| `luna-bo5` | 1 | 0 | not graded | — | $0.0213 | — | 7m01s |
| `micro-bo1` | 3 | 0 | 0 of 3 | — | ≥ $0.0303 | — | 3m25s |
| `micro-bo3` | 1 | 0 | not graded | — | $0.0187 | — | 2m38s |

For a single candidate, the oracle is its own reward. Cost is per arm over
its graded trials and includes Jev; `micro-bo1` has two unpriced
Microluna calls, so its figure is a lower bound. The candidates of the
`luna-bo5` and `micro-bo3` trials were still being graded when this was
written.

The only task every arm reached is `mvcc-lsm-compaction`:

| Arm | Reward | Candidates | Verdicts | Checks per candidate | Kept, and why | Cost | Agent time |
| --- | ---: | --- | --- | --- | --- | ---: | ---: |
| `luna-bo1` | 0 | 1 | unknown | — | — | $0.0056 | 1m52s |
| `luna-bo3` | 0 | 3, graded 1.0, 0.0, 0.0 | unknown ×3 | 2 passed each | 2: verdicts and checks tie, cheapest | $0.0158 | 2m21s |
| `luna-bo5` | 0 | 5 | unknown ×5 | — | 3 | $0.0213 | 7m01s |
| `micro-bo1` | 0 | 1 | fail | — | — | ≥ $0.0035 | 1m21s |
| `micro-bo3` | 0 | 3 | fail ×3 | — | 2 | $0.0187 | 2m38s |

Candidate 1 of the `luna-bo3` trial passed all 15 of the verifier's tests
when graded from its archive. The kept candidate 2 failed 4 of 15, and
the trial's own verifier run agrees: 4 failed, 11 passed. Candidate 1
cost $0.0060 against candidate 2's $0.0043, so the cost key made the
wrong pick.

### Losses

- `luna-bo3` on `ks-solver-cpp` and `luna-bo5` on
  `wal-recovery-ordering` ended with `OSError` when the host's disk filled
  at 08:11 UTC, which also stopped the scheduler. It was restarted once
  space was freed.
- `luna-bo3` on `wal-recovery-ordering`, `luna-bo5` on `ks-solver-cpp`,
  and `micro-bo3` on `wal-recovery-ordering` were cancelled by the
  operator's stop at 08:36 UTC. The `luna-bo3` trial had already kept a
  candidate; its verifier never ran.

## Cost and time

| Scope | Total | Jev | Jev requests | Delegate calls |
| --- | ---: | ---: | ---: | ---: |
| TB4 trials, including lost and cancelled | $0.1516 (lower bound) | $0.0193 | 193 | 19 |
| Mini-tasks | $0.0873 | $0.0059 | 111 | 21 |

Jev is priced at $0.042 per million input tokens for `jev-1.13.0`. A
best-of-N trial adds about two Jev requests per candidate: the verdict's
report questions. Five Luna candidates on `mvcc-lsm-compaction` cost
$0.0200 of delegate time, against about $2 for one Opus 5.5 trial
([what we have learned](2026-09-23-what-we-have-learned.md)).

## Analysis

- **Generation has headroom that the selection didn't reach.** The one
  fully graded best-of trial had a passing candidate, and one Luna run on
  the same task in #9583 and here didn't pass. That is one trial, not a
  rate.
- **The verdict can't rank Codex Luna candidates yet.** It said `unknown`
  on all 11 TB4 best-of candidates, and `pass` on a failing
  `log-severity` candidate and on the failed `luna-bo1` trial on
  `wal-recovery-ordering`. The truthful-checks measurement warned that Luna's reports
  are short (a median of 311 characters). Selection then falls to the
  checks, which tied on `mvcc-lsm-compaction`, and to cost, which carries
  no information about correctness.
- **What would help selection.** The checks separated candidates only
  when a scenario failed (`cancel-cleanup`). A tie-breaker that reads more
  than the report, such as the verdict's `p_fail` as a continuous score,
  or behavior checks that run the task's own tests on each candidate,
  should come before cost. On `mvcc-lsm-compaction`, `p_fail` would have
  ranked the passing candidate first (0.36 against 0.40 and 0.37), which
  is a hint, not evidence.

## Threats to validity

- **Too few trials.** 8 graded trials, one per task and arm at most, and
  one fully graded best-of trial. No pass rate here has a useful
  interval.
- **Copy isolation.** A candidate that writes to the real workspace path
  anyway changes what the others read. The record's `leaked` was `false`
  on every best-of trial and mini-task that ran. Build outputs that embed
  the copy's path move with the kept candidate.
- **Grading from an archive.** A candidate is graded from its archived
  workspace over the task image, with only the task's declared artifacts.
  On `mvcc-lsm-compaction`, the replay of the kept candidate matched the
  trial's own verifier (0.0, the same 4 failures).
- **A host event.** The disk filled during the run, and two trials were
  lost to it.

## What's needed to close the issue

- Rerun the matched experiment on Microluna only: `microluna-best-of-1`
  against `microluna-best-of-3` and a `microluna-best-of-5`, on the same
  10 tasks, with early stopping, once Microluna's transport holds up.
- Grade every candidate (`tbench verify --candidate`, then
  `best-of/grades.json`) so the oracle rate and the selection accuracy
  have a denominator.
- Try a selection key that reads more than Luna's short report before
  cost.

## Evidence

- Experiment: `~/.openagents/terminal-bench/experiments/best-of-9587/`,
  and its jobs under `~/.openagents/terminal-bench/jobs/*best-of-9587*`.
- Candidate grades: `best-of/grades.json` in each graded episode, and the
  verifier replays under
  `~/.openagents/terminal-bench/replays/mvcc-lsm-compaction--candidate--*`.
- The reader: `gym coder composition --best-of --job best-of-9587`, and
  `gym coder composition --job best-of-9587 mvcc` for each candidate.
