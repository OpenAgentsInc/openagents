# `checks.metric_target` offline: protocol

Issue [#9657](https://github.com/OpenAgentsInc/openagents/issues/9657).
This protocol is committed with the hand labels and the extraction code,
before any Jev answer on a task instruction is read. It fixes the
population, the labels, the extraction, the comparison rule, and the
budgets.

## What is frozen

- The labels, `labels.json` (SHA-256
  `65cba8582f2033b974a3bdecd76f8cd223aab19cf2b57937cff7ff2879492327`).
  They were written from each instruction's text only, without the
  task's tests, its solution, a verifier output, or a trial log, and
  before any Jev answer existed.
- The question set, `questions/metric-target.json` (SHA-256
  `e17f96a9ef74e64b13cf68d316c06df3595b754af90bb0f0c872dad5aeae994d`).
- The extraction code in `crates/coder-one/src/checks/metric_target/`:
  `candidates`, `references`, `parse_number`, `request`, `targets_from`,
  and the comparison in `offline.rs`. At the freeze, `mod.rs` has SHA-256
  `fd72e10abdd72e26813629343b1f9d1149dd81f24402c41bcc9369db312834c9` and
  `offline.rs` has
  `ae897b20eeb83203f5c865319cad81aea2a849db3ab9fdb56821af914787b352`.
  Later edits to other parts of those files, such as formatting or lint
  fixes, are allowed. Any change to the functions named here after the
  first Jev answer is listed in the results with its reason, and both
  numbers are reported.

## Population

Every task with at least one retained Coder One or Microluna trial under
`~/.openagents/terminal-bench/jobs`, found by job name, less the eleven
tasks the Fable pattern map was learned from: `embedding-drift-monitor`,
`coq-block-bound`, `shadow-relay`, `risk-scorer-replay`,
`mp-checkpoint-consolidation`, `payments-pipeline-fix`,
`telecom-entity-resolution`, `fp8-rmsnorm-gemm`, `distributed-dedup`,
`intrastat-meldung`, and `photonic-waveguide-routing`. That's 61 tasks.

## Procedure

1. `coder-one checks metric-target offline --labels labels.json --out
   records --jev live` reads each labeled instruction, checks its digest,
   and asks Jev one request per task. Every live answer is kept in
   `records/jev-recorded.json`. The run stops asking once the recorded
   input tokens reach $0.10 (the Jev budget).
2. `records/summary.json` compares the answers with the labels by the rule
   below. `replay.sh` reruns it from the recorded answers alone.
3. The trial count comes from `trials.json` and `trials-summary.json`,
   which were built from the verifier outputs and the trial logs without
   Jev.
4. The harness spread is measured with `coder-one checks metric-target
   measure` on a few retained workspaces whose task image is on this
   machine and needs no GPU, with a harness written for each. Luna may
   write those harnesses, at most $0.50 in all, recorded.

## Comparison rule

- A task is positive when its label states a numeric goal; the extraction
  is positive when it names at least one target.
- On a task both call positive, the extraction's first target, the one the
  lean loop holds, is correct when its threshold equals the first labeled
  target's, its direction matches, and both are absolute or both are
  relative. The quantity is compared and reported apart.
- A task both call negative is a correct negative.

## Admission

The switches stay off in every manifest whatever the result. This
measurement says how often the extraction is right on tasks it wasn't
built from, and how often a failed trial ended with its stated target
unmeasured or unmet while the session claimed done. Running the component
in a policy needs matched mini-task runs first.
