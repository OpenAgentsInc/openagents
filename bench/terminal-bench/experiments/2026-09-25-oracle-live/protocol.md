# `checks.oracle` in a live Microluna loop: protocol

Status: pre-registered on 2026-09-25, not run. No Terminal-Bench trial,
container, Luna session, or Jev call was made to write it. It belongs to
issues [#9656](https://github.com/OpenAgentsInc/openagents/issues/9656)
(`checks.oracle`), [#9657](https://github.com/OpenAgentsInc/openagents/issues/9657)
(`checks.metric_target`), [#9607](https://github.com/OpenAgentsInc/openagents/issues/9607),
and [#9587](https://github.com/OpenAgentsInc/openagents/issues/9587).

This document fixes, before any run, the tasks, the two arms, the
attempts, the measures, the bar, the stopping rule, and the spend cap of a
small live run of the lean Microluna loop with the host-written oracle on.
Nothing in it changes after the first launch. A correction goes in a
separate, dated `amendments.md` beside this file, written before the
labels it could affect are joined, and says what it changes and why. This
file's SHA-256 is the digest the oracle-on manifest names.

## Why this run

The [tier 0 measurement](../../../../docs/terminal-bench/2026-09-25-oracle.md)
left `checks.oracle` unadmitted for one reason: too few graded passes.
Only 3 tasks had a graded pass and a graded failure that a usable, blind
oracle answered, and 5 passes put the low end of "passes kept green" at
0.57 against a bar of 0.8. Its conclusion: "The next measurement needs
more graded passes, not a better prompt."

This run has two purposes:

1. **Collect graded workspaces for the admission bar.** Run the loop
   with the oracle on, on tasks that nothing the oracle, the metric
   target, or the Fable pattern map was learned from, retain every
   candidate, grade every candidate with the task's own verifier after the
   run, and read the bar on them.
2. **See whether the oracle-gated finish changes outcomes.** Compare the
   same manifest with the oracle on and off, on the same tasks, with the
   same attempts.

## Tasks

`select.py` in this directory writes `pool.json`, which lists every
Terminal-Bench 4.0 (TB4) task, the rules that exclude it, and its Fable
5.1 figures. It reads only retained repository files and the pinned TB4
checkout's environment definitions (`environment/Dockerfile*`), never a
task's tests or solution.

### Exclusion rules

A task is excluded when any rule applies.

| Rule | Excludes a task that | Source |
| --- | --- | --- |
| E1 | Fable 5.1's winning runs were mapped from (11 tasks) | [Fable pattern map](../../../../docs/terminal-bench/2026-09-25-fable-pattern-map.md) |
| E2 | Is in the oracle's tier 0 population (17 tasks) | [Tier 0 protocol](../2026-09-25-oracle/protocol.md) |
| E3 | A Coder One or Microluna policy was tuned on, measured on, or studied for lessons: every task in the tuned-task list, which covers the Microluna dev, held-out, family, iteration, and confirmation sets (42 tasks) | [`crates/coder-one/contamination-tuned.json`](../../../../crates/coder-one/contamination-tuned.json) |
| E4 | States a numeric goal by the metric target's labels, so the metric target's extraction, harness, and finish rule act on it (19 tasks) | [Metric target labels](../2026-09-25-metric-target/labels.json) |
| E5 | Shares a group (the task name's first word) with a task E1 to E4 exclude | The family pre-registration's grouping rule |
| E6 | Needs a GPU | The TB4 catalog |
| E7 | Has no Python 3 in the image its environment definition builds. A written oracle runs as `python3 oracle.py` | `environment/Dockerfile*` |
| E8 | Fable 5.1 low never passed, or its mean trial time is over 45 minutes, the Luna-sized family's time limit | [Family table](../2026-09-25-luna-sized-family/tasks.json) |

### The strict reading leaves no usable task

Read strictly, "the metric target's in-sample tasks" are all 61 tasks of
its population: its second measurement changed code after reading the
first run's answers on all of them. With E1, E2, E3, and all 61, only
`lake-temp-glm` (Fable 5.1 low 0 of 5) and `wdm-design` (5 of 5 at a mean
of 210 minutes, 8 times the loop's 25-minute deadline) are left. Neither
can yield a graded pass inside this loop's bounds. No TB4 task is
Luna-sized and outside all four sources.

### The one declared relaxation

E4 excludes the metric target's 19 goal tasks, not all 61. On the other
42, the metric target's measurements read only the instruction text, and
its code was changed so that it holds no target there (1 false positive
in 42 with the shipped wording). No trial log, workspace, or verifier
output of those 42 was read for it. In this run, on a task without a
stated goal, the metric target is expected to hold nothing, and it's on
in both arms, so it can't explain a difference between them. Every run
of it is recorded, including any target it holds.

**This relaxation needs the operator's consent before launch.** Without
it, the run has no tasks.

### What's left

Three tasks meet every rule:

| Task | Fable 5.1 low: passes, mean trial time, cost per pass | All efforts | CPUs, memory | Earlier exposure (not an exclusion) |
| --- | --- | --- | --- | --- |
| `hof-topology-interpenetration` | 4 of 5, 20.3 min, $4.38 | 24 of 25 | 2, 8 GB | Opus `coder-one-tunable-v2` trial; truthful-checks calibration half; the metric target's instruction-only read |
| `takens-embedding-lean` | 3 of 5, 43.7 min, $42.19 | 20 of 25 | 4, 16 GB, 50 GB disk | Opus `coder-one-tunable-v2` and `claude-code-opus` trials; truthful-checks calibration half; the metric target's instruction-only read |
| `satb-audio-transcription` | 1 of 5, 28.7 min, $39.83 | 16 of 25 | 4, 8 GB | Opus `coder-one-tunable-v2` trial; the metric target's instruction-only read |

None of the three has a GPT-6 Luna or Microluna trial. None is
Luna-sized by the family's rule: each fails its shape or check criterion,
and two fail its reliability criterion. Tasks that E7 alone excludes,
`nextjs-performance` (3 of 5) and `react-lead-form` (2 of 5), are listed
in `pool.json`; with the oracle unable to run there, both arms would be
the same loop.

**Author's exposure.** While choosing the pool, the author read the Luna
baseline report's one-line summary of its `nextjs-performance` trials,
which that task's exclusion under E7 makes moot, and each candidate task's
`environment/` directory listing, `task.toml` budgets, and Dockerfile
`FROM` and Python lines. No task's instruction, tests, or solution was
read for this protocol, and no earlier trial of the three tasks was
opened.

## Arms

Both arms are `microluna-v18` with `checks.metric_target` on. They differ
in one field, `executor.microluna.lean.oracle`.

| Arm | Manifest | Terminal-Bench profile |
| --- | --- | --- |
| Oracle on | `crates/coder-one/policies/microluna-oracle-live-on.json` | `coder-one-microluna-oracle-live-on` |
| Oracle off | `crates/coder-one/policies/microluna-oracle-live-off.json` | `coder-one-microluna-oracle-live-off` |

- **Base.** `microluna-v18.json`, file SHA-256
  `4da78507d84a67bbc18886ba9e6975d1f8b0bd15962dde8901f4186f9b1ba912`:
  Luna at high effort, one first session, 4 sequential sessions, keep-best
  on the frozen score, `retain_candidates`, the score-only finish rule,
  the baseline, a $0.09 spend bound for each dispatch, a 1,200-second loop
  wall bound, and a 1,500-second dispatch deadline.
- **`checks.metric_target`, both arms.** `write_harness` true, `finish`
  true, `harness_usd` 0.02, `harness_sec` 180, and a measurement protocol
  of 1 warmup, 3 repeats, 60 seconds a run, and 180 seconds a
  measurement. The harness session's spend comes out of the dispatch's
  $0.09. `control.optimize` stays off: it refuses `retain_candidates`.
- **`checks.oracle`, oracle-on arm only.** The tier 0 writer bounds:
  30 turns, 600 seconds, $0.08, `gpt-6-luna` at high effort, and Jev's
  bound of 0.5 for every Noul. The Jev questions (`DEFINES`, `BOUNDARY`,
  `PARAMETER`) and the writer's `TASK` and `PROTOCOL` prompts are the ones
  frozen at tier 0. Since then, the writer running in its own container
  also gets the `CONTAINED` paragraph, which says where its commands run
  and that only `oracle.py` is kept. The host writes the oracle before the trial
  with `coder-one checks oracle write`, in its own container of the
  task's image with no network and no host mount, and delivers it to
  `/opt/openagents/oracle`, read-only. The manifest names this
  experiment, `2026-09-25-oracle-live`, and this file's SHA-256.
  Validation accepts the switch only for that exact pair, which
  `checks::oracle::EXPERIMENTS` lists; `checks::oracle::ADMITTED` stays
  `false`.
- **Agent setup time, both arms: 900 seconds.** Harbor's default of 360
  seconds leaves the host writer 150 seconds after the adapter's install
  margin (150 seconds) and step margin (60 seconds), a quarter of the tier
  0 writers' 600. 900 seconds leaves it the full 600. Both profiles set
  the same value, so the arms differ only in the manifest.

Commit, artifact, and digests are pinned in `pins.json` before the first
launch: the source commit, the `coder-one` artifact's SHA-256 (a static
Linux build from that commit, which also runs as the host writer), each
manifest's file SHA-256 and resolved digest, and the cohort spec's
SHA-256.

## Schedule

- **Attempts.** 4 per task per arm: 24 counted trials.
- **Order.** Round `a` runs every task once in each arm. Within a round,
  tasks go in the order above, and the arm that goes first alternates by
  round and task, so each arm goes first in half the slots (on first in
  round 1 for the first task, off first for the second, and so on,
  swapped in the next round).
- **Driver.** One `tbench cohort` spec, `cohort.json` in this directory,
  with per-slot manifests, concurrency 2, and one infrastructure retry a
  slot. A retry runs only after a failure before the agent started, as the
  cohort's rule already defines it.
- **Host.** The x86_64 benchmark host, TB4 at `v4.0.0`
  (`452bf305c6da`), kept task images (`tbench-warm/*`), the network
  allowlist `openagents.com`, `api.typesafe.ai`, and `chatgpt.com` for the
  agent phase. The host writer's own container has no network; it reaches
  Luna and Jev from the host.

## Candidate retention

Both arms keep `retain_candidates`, so the adapter captures each
session's candidate with the task's declared artifacts
(`candidate-checkpoint-v1`). All three tasks pass `tbench
candidate-preflight`. After the cohort ends, `tbench candidates` grades
every retained candidate of every trial with the task's own verifier, in
a separate verifier environment, and no grade reaches the executor.

**Oracle headroom** for a trial is whether any of its graded candidates
passed. It's `null`, never 0, when any candidate's grade is missing.

## Measures

### Primary: the admission bar, on held-out tasks

Every graded workspace is a final submission (its trial's verifier
reward) or a retained candidate with a valid binary grade from `tbench
candidates`. Workspaces with no valid grade are listed and never counted.

- **The task's oracle.** For each task, the primary oracle is the one
  written by the first oracle-on slot, in schedule order, whose host step
  delivered an oracle. The others are reported beside it as repeats of
  the writer, never pooled with it. An oracle whose writer record doesn't
  say `writer-container` isn't blind and is reported apart.
- **Scoring.** After the run, the primary oracle runs once on every graded
  workspace of its task, from both arms, each in its own container of the
  task's image with the network off, as tier 0 did. Every verdict is
  written before any grade is joined with it.
- **Calls,** as at tier 0: `pass` when at least one case passed and none
  failed, `fail` when any case failed, none otherwise. **Usable** when
  its call on the untouched workspace is `fail`; **trivially passing**
  when it's `pass`.
- **Mixed task:** a task with at least one graded pass and one graded
  failure that its usable primary oracle answered.

**The bar, verbatim from the tier 0 protocol:** "The bar is the one
#9629 set for a class that may hold the loop, read on at least 2 mixed
tasks: passes kept green with the interval's low end at 0.8 or more,
failures called red with its low end at 0.2 or more. Meeting it makes the
component a candidate for matched mini-task runs; it doesn't admit it.
`checks::oracle::ADMITTED` stays `false` either way." Intervals are 95%
Wilson intervals. The effective sample is the number of mixed tasks.

Reported with the bar: pass-and-fail pairs ordered right by the oracle's
score (ties left out and counted), and the tier 0 across-task rates (a
`fail` call on a failing workspace, a `pass` call on a passing one).

**What the bar needs.** A 95% Wilson low end of 0.8 needs 16 graded
passes kept green with none turned red, or 25 with one turned red. Three
tasks at 4 trials an arm, each with at most 4 retained candidates and a
final, can give that many passes only if Luna passes these tasks often.
If it doesn't, the bar is reported as not met, with the counts, and this
run's result is the rates and their intervals.

### Secondary

1. **Outcomes by arm.** Final passes over graded trials, per task and
   pooled, with 95% Wilson intervals, and oracle headroom by arm. The
   difference is described, not tested for significance: 12 trials an
   arm can't separate small effects.
2. **What the oracle did in the loop.** For each oracle-on trial: the
   host step's status (`delivered` or `unavailable`, with its reason),
   the loop's status (usable, trivially passing, could not run, refused,
   missing), the `done` finishes it refused, and whether the submitted
   candidate is the one the frozen self-score would have kept, where the
   records show both.
3. **Each trial's own oracle.** Its in-loop verdict on each retained
   candidate beside that candidate's grade.
4. **What the metric target did.** Per trial, whether it held a target,
   and any finish it refused.
5. **Cost and time.** Luna, Jev, and host-writer cost by arm, per graded
   trial and per pass; Harbor's phase times, with agent setup reported
   apart because the oracle-on arm's includes the writer.

## Stopping rule

There's no early stop on outcomes: no result, pass or fail, stops the
run. The run stops, and is reported as it stands, when any of these
happens:

1. **The cohort stops.** The spend cap, an unknown cost, a reservation
   breach, or a changed source, artifact, manifest, or task identity:
   `tbench cohort`'s own refusals.
2. **The oracle can't be delivered.** The first two oracle-on trials
   both record the host step `unavailable` for an infrastructure reason
   (no host binary, the image unknown or absent, Docker unreachable, or
   too little setup time). That makes the run invalid for the primary
   measure; fixing it is a harness change and needs a new
   pre-registration.
3. **Contamination.** Any trial's contamination check reports a finding.
4. **Infrastructure.** Three slots end without a graded trial after
   their retry.

After a stop, no slot is rerun under this protocol.

## Spend

- **Cap: $5.00** for Luna, Jev, and the host writer together, enforced by
  the cohort's `budget_usd`. No Claude.
- **Counting.** The cohort's `microluna-open-request-v1` rule, with the
  host step's cost added under `oracle-host-v1`: a Microluna dispatch cut
  off with a request open counts `max(recorded, $0.09 + Jev)`, a writer
  whose cost isn't known counts its $0.08 bound, and an unrecorded host
  Jev cost counts $0.01. Any other unknown cost keeps the attempt's whole
  reservation and stops new launches.
- **Reservation:** $0.20 an attempt, above the largest counted cost of an
  oracle-on trial ($0.09 + Jev + $0.08 + $0.01).
- **Estimate.** Expected about $1.40: v18's 18 family trials counted
  $0.054 a trial, and the tier 0 writers cost $0.004 each in Luna and less
  than a cent in Jev. The most the counting rule can reach is about $3.50
  (12 trials at $0.19 and 12 at $0.10), before retries.
- **Time.** About 4 to 5 hours of wall time at two trials at once, plus the
  first build of each task's image.

## Afterward

The report goes in `docs/terminal-bench/` with every measure above, the
spend by component, and the evidence retained with `tbench retain`. The
oracle-on manifest keeps its experiment field after the run; no other
manifest may name it. If the bar is met, the next step is the tier 0
report's: matched mini-task runs, with `ADMITTED` still `false`.
