# Executed contract checks: protocol

Issue [#9628](https://github.com/OpenAgentsInc/openagents/issues/9628).
Written on 2026-09-25, before the component exists and before any of its
outputs are joined with a verifier reward.

## What is measured

`checks.contract` extracts, by code, every executable item that a task's
instruction states, and the files the instruction points to: shell
commands, example invocations, output paths, and stated expected results
(values, counts, file formats, and exit codes). It runs each item in the
candidate's workspace and compares the observed result with the stated one
by code. Each item gets one of four outcomes: `matched`, `differed` (with a
bounded diff), `could_not_run` (with the reason), or `not_executable`.

A Jev Noul may classify an ambiguous span, for example whether a command is
stated to succeed. It never sees a candidate, a candidate's output, or a
grade, and it never judges the candidate.

## Population

Every retained workspace that `coder-one accept offline`'s reader finds
under `~/.openagents/terminal-bench/jobs`, with the candidate grades under
`bench/terminal-bench/experiments/2026-09-24-candidate-evidence/records/candidate-grades`
and the reconstruction
`bench/terminal-bench/experiments/2026-09-24-candidate-evidence/records/reconstructed-v12-embedding-r1-before-review`:
Coder One snapshots, Microluna final workspaces, retained lean-loop
candidates, and the reconstruction.

Excluded before anything runs:

- The eight tasks of #9584's prospective cohort: `distributed-dedup`,
  `formal-crypto`, `freecad-impeller`, `freecad-spring-clip`,
  `math-eval-grader`, `pretrain-shard-corruption`, `shadow-relay`, and
  `vpp-loss-divergence`. Their official outcomes are sealed by the Codex
  agent's protocol. Every trial of these tasks is excluded, including
  older trials, so that no measurement here touches that cohort.
- Every job whose name contains `truth-confirmation` or `truth-control`.
- Workspaces whose verifier reward is unknown. They're listed, and never
  counted as a pass or a failure.

The primary set is the workspaces the verifier graded: snapshots whose
check candidates all share the snapshot's digest, Microluna finals,
candidates with a known reward, and the reconstruction. A snapshot the
verifier didn't grade (later rounds changed the workspace) is reported in
a secondary set only.

## Split, frozen now

The split is by task, by code, not by choice:
`int(sha256("openagents-9628:" + task)[:8], 16) % 2 == 0` is development,
otherwise held out.

| Development (12) | Held out (11) |
| --- | --- |
| `embedding-drift-monitor` | `bun-sourcemap-leak` |
| `fin-saccr-rwa` | `cad-model` |
| `gsea-proteomics` | `cargo-flight-dispatch` |
| `interleaved-vigenere` | `coq-block-bound` |
| `legacy-utility-triage` | `heat-pump-warranty` |
| `mp-checkpoint-consolidation` | `html-js-filter` |
| `mvcc-lsm-compaction` | `ks-solver-cpp` |
| `protein-autointerp-disulfide` | `photonic-waveguide-routing` |
| `sound-change-cascade` | `production-planning` |
| `telecom-entity-resolution` | `risk-scorer-replay` |
| `uefi-bootkit` | `session-window-debug` |
| `wal-recovery-ordering` | |

The extractor's rules may be written and changed while reading the
development tasks' instructions, their workspace files, the component's
outputs on development candidates, and those candidates' rewards. Nothing
about a held-out task is read before the freeze except its Dockerfile's
base image line, which was read to decide which images to build.

What was already known before this protocol: per-task pass and fail counts
of the retained workspaces, printed by an inventory script while listing
the population (that printout also included the sealed cohort's counts by
mistake; see the write-up), and the tables already published in
`docs/terminal-bench/2026-09-24-acceptance-first.md`.

## Frozen rules

The freeze is the commit that holds the component's code, recorded in the
write-up. After it, nothing in the extractor, the comparisons, the
timeouts, the Jev question, or the threshold changes before the held-out
tasks are run and joined.

- **Item outcomes** come from the component alone.
- **Per-kind outcome of a candidate.** For each item kind, a candidate is
  `differed` when any item of that kind differed, else `matched` when any
  matched, else it has no outcome for that kind.
- **Candidate call.** Fail when any item differed. Pass when at least one
  item matched and none differed. Otherwise no call.
- **Candidate score.** `matched / (matched + differed)`, undefined with no
  executed item.
- **Jev.** A span is treated as an executable example only when code finds
  it, and where code alone can't tell whether a command is stated to
  succeed, one Noul decides at 0.5. Answers are recorded and replayed.
  Total Jev spend stays under $0.05.
- **Timeouts.** A stated time bound when the instruction gives one near
  the invocation; otherwise 120 seconds for an example and 300 seconds for
  a command.

## Analysis, frozen now

For each set (development, held out, both):

1. Per item kind: of candidates whose kind outcome is `differed`, the share
   the verifier failed; of those whose outcome is `matched`, the share it
   passed. 95% Wilson intervals, and a task-grouped bootstrap (10,000
   resamples of whole tasks, seed 9628) because candidates within a task
   aren't independent.
2. Candidate calls: fail precision, failure recall, pass precision, and
   pass recall, with Wilson intervals.
3. Within-task discrimination: for each task with at least one passing and
   one failing candidate, the concordance of the candidate score over
   every (pass, fail) pair, a tie counting half. Report per task, the mean
   over tasks with a task-grouped bootstrap interval, and the number of
   tasks where the score differs at all between candidates.
4. Coverage: items per task by kind and outcome, and how many candidates
   get no call.

Negative results are reported as they come. The held-out numbers are the
only ones that can support a claim; the development numbers are fitted.
The component isn't wired into any policy.
