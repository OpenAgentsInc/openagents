# Luna-sized family: pre-registration

Status: pre-registered on 2026-09-25, not run. No Terminal-Bench trial,
Luna session, or Jev call was made to write it. It answers step 4 of
[the path to definitive wins](../../../../docs/coder/design/2026-09-24-assessment.md#the-path-to-definitive-wins)
and belongs to issue
[#9607](https://github.com/OpenAgentsInc/openagents/issues/9607).

This document fixes, before any run, which Terminal-Bench 4.0 (TB4) tasks
the next matched run of the cheaper-work claim uses, which policy it runs,
how many attempts it makes, how it counts cost and time, and what result
counts as a win, a loss, or inconclusive. The machine-readable companion,
[`tasks.json`](tasks.json), holds every TB4 task's scores, exclusions, and
Fable 5.1 figures, the split, and the pinned manifest's digests.
[`select.py`](select.py) regenerates it from retained repository files.

## Summary

- **The family is six tasks, not ten, because the untouched pool is
  that small.** Nine of the 66 TB4 tasks meet every "Luna-sized" criterion
  below. Six of those nine are already used for tuning or testing, or need
  a GPU. Of the tasks that no rule excludes, six have a Fable 5.1 low pass
  rate of at least 4 of 5, and they are the whole family. The next
  untouched tasks, `kv-live-surgery` (2 of 5) and
  `satb-audio-transcription` (1 of 5), fail the reliability criterion and
  aren't Luna-sized, so adding them would only add tasks where the claim is
  implausible.
- **Split, by a rule fixed before it was computed:** development
  `mp-checkpoint-consolidation`, `live-database-cutover`, and
  `photonic-waveguide-routing`; confirmation `payments-pipeline-fix`,
  `cumulative-layout-shift`, and `telecom-entity-resolution`.
- **Hypothesis.** The pinned `microluna-v15` passes at least 2 of 3
  attempts on at least 2 of the 3 confirmation tasks, each at an all-in
  cost per pass under 10% of Fable 5.1 low's cost per pass on that task.
  Trial time per pass is compared with Fable low's trial time per pass and
  reported, but it isn't part of the win.
- **Size.** 3 attempts on each of the 6 tasks: 18 counted trials. Expected
  spend $0.50 to $1.80 of Luna and Jev at list price; ceiling $3.00.
  Expected wall time about 5 hours with two trials at a time, or about 3
  hours with four.

## What a Luna-sized task is

The one task Microluna passes reliably is `embedding-drift-monitor`: 6 of 6
across v13 to v17, 3 of 3 for `microluna-v13-retained`, and 2 of 3 for v12
([candidate evidence](../../../../docs/terminal-bench/2026-09-24-microluna-candidate-evidence.md),
[iterations](../../../../docs/terminal-bench/2026-09-24-microluna-iterations.md)).
Its passes share a shape:

- **A repair of shipped source against stated symptoms.** The instruction
  says the monitor is broken, names what goes wrong, and says the defects
  are in the production modules. The defect is in the source Luna reads,
  so the first session can find it.
- **A correct first session.** The v12 attempt 3 and v13 passes came from a
  first session that wrote the right fix and a later session that left it
  alone. Once, in v12 attempt 1, an editing review fixed a defect the first
  session had kept. No pass came from looping a failing candidate to green.
- **Local material to run.** Data scenarios ship with the task, so a
  session can exercise the code it changed.
- **A cheap, quick Fable reference.** Fable 5.1 low passes 5 of 5 at $0.87
  and 3.1 minutes, so a pass is achievable, and Microluna's $0.016 per pass
  is a large cost gap.

The failures fall into three kinds, from the
[capability-gap log](../../../../docs/terminal-bench/capability-gaps.md):

- **Search with no locatable defect** (capability gaps).
  `sound-change-cascade` and `interleaved-vigenere` ask Luna to infer a
  hidden structure; 0 of 19 trials found it.
- **A format-only self-check that called a wrong answer done** (signal
  gaps). `fin-saccr-rwa` and `gsea-proteomics` finished with correctly
  formatted wrong figures; nothing in the workspace held a reference to
  check the substance against.
- **A long, hard core step.** `shadow-relay` needs a binary session decoded
  and `coq-block-bound` a proof; both ran to the 25-minute bound.
  `session-window-debug` is repair-shaped, but Fable fails it 0 of 25, and
  Luna missed the same cases in all nine attempts.

The score below turns that into four parts, each from public data: the
task's instruction and Fable 5.1's public trials.

| Part | Points | Rule |
| --- | --- | --- |
| Shape | 0 to 2 | 2: a repair of shipped source against stated symptoms or a normative local spec. 1: a build or engineering task to a clear specification. 0: search, inference, perception, a proof, open optimization, or a judgment that needs outside domain knowledge. |
| Check | 0 to 2 | 2: an exact local reference, a reproducer, or runnable tests or examples ship with the task. 1: the stated criteria can be checked with a harness built from local tools. 0: no local ground truth. |
| Fable low | 0 to 2 | 2 × Fable 5.1 low passes ÷ 5. A reliable pass means the task is achievable and there's a cost gap to win. |
| Time | 0 to 2 | Fable 5.1 low mean trial time: 2 at 10 minutes or less, 1.5 at 25 or less (v15's wall bound), 1 at 45 or less, 0.5 at 90 or less, else 0. |

A task is **Luna-sized** when shape and check are each at least 1, Fable
low passes at least 4 of 5, and Fable low's mean trial time is at most 45
minutes. The rule reproduces the known outcomes: `embedding-drift-monitor`
scores 7.0 and is Luna-sized, and every capability-gap task fails at least
one criterion (the search tasks, `shadow-relay`, `coq-block-bound`,
`fin-saccr-rwa`, and `gsea-proteomics` on shape, and
`session-window-debug` on Fable's 0 of 5).

Shape and check are judgments from the instruction alone. They name no
hidden test, reference value, or verifier fact; `tasks.json` gives a short
reason for each.

## Exclusions

A task is excluded when any rule applies. Every rule is written from a
named record; `tasks.json` lists each task's rules and reasons.

| Rule | Excludes a task that | Source |
| --- | --- | --- |
| R1 | Is in Microluna's dev set or held-out test set | [Iterations](../../../../docs/terminal-bench/2026-09-24-microluna-iterations.md) |
| R2 | Is in the capability-gap log | [Capability-gap log](../../../../docs/terminal-bench/capability-gaps.md) |
| R3 | Has a GPT-6 Luna trial in any harness | The [Luna baseline](../../../../docs/terminal-bench/2026-09-24-luna-tb4-baseline.md), the Luna rows of `crates/coder-one/fixtures/truth/rows.jsonl`, and the Microluna jobs under `~/.openagents/terminal-bench/jobs/` on the benchmark host |
| R4 | Fitted or evaluated a signal, or was the chosen target of a Coder One experiment on a mechanism v15 turns on | The truthful-checks calibration half, the #9584 prospective Microluna cohort, the stall-detection split, the persistence and escalation targets (`production-planning`), and the matched Opus controller test (`batched-eval-parity`) |
| R5 | Had its hidden tests and reference solution read | The [task anatomy](../../../../docs/terminal-bench/2026-09-24-task-anatomy.md) |
| R6 | Shares a group with a task excluded under R3 to R5 | The task pool's grouping rule: a group is the task name's first word (`layout-config-recreation2` with `layout-config-recreation`, `freecad-platform-drawing` with the #9584 FreeCAD tasks) |
| R7 | Needs a GPU, or can't finish inside v15's 25-minute wall bound by its own clock | The TB4 catalog; `ctr-optimization`'s simulated campaign runs about 4.8 hours |

Three checks found nothing to exclude:

- **Live instructions.** `cargo run -q -p coder-one -- contamination
  check` is clean: 18,269 texts scanned against 156 task ids, 528 verifier
  test names, and 292 anatomy facts, with no finding. No instruction text
  was changed for this protocol.
- **Mini-task-adjacent tasks.** The mini-tasks reproduce Terminal-Bench 2.0
  failure families (`log-summary-date-ranges`, `headless-terminal`,
  `cancel-async-tasks`, and `fix-git`), and the evidence packer v15 uses was
  tuned on those tasks and mini-tasks. No TB4 task maps to them.
- **Luna trials on family tasks.** No Luna or Microluna job on the
  benchmark host names a family task.

Some exposure doesn't exclude a task, and `tasks.json` doesn't hide it:
Coder One's full-suite Opus run (`coder-one-tunable-v2`) touched every
task, retrospective analyses (the task-win analysis, the strategy
fingerprints, and the truthful-checks held-out half) read those records,
and the v4 check levers were designed on retained Opus trials that include
`payments-pipeline-fix`. None of these is a Luna result, and `verify.checks`
is off in v15.

## The pool

Nine TB4 tasks are Luna-sized. Six are out:

| Task | Score | Why it's out |
| --- | ---: | --- |
| `risk-scorer-replay` | 7.5 | R3, R4: a Luna trial, and signal calibration |
| `batched-eval-parity` | 7.1 | R4: the matched Opus controller test |
| `embedding-drift-monitor` | 7.0 | R1: the dev set |
| `retro-console-soc` | 6.0 | R4: signal calibration |
| `fp8-rmsnorm-gemm` | 6.0 | R7: needs a GPU |
| `distributed-dedup` | 5.1 | R3, R4: the #9584 Microluna cohort |

The family rule is: no exclusion applies, and Fable 5.1 low passes at
least 4 of 5. Six tasks meet it. Three are Luna-sized; the other three are
in because Fable low passes them reliably and no rule excludes them, and
each misses one Luna-sized criterion, which the table names.

## The family and the split

Fable figures come from
[`fable-5.1-replays.json`](../../reference/fable-5.1-replays.json)
(retrieved 2026-09-24): five public attempts at each of five efforts.
Trial time is the trial's start to its end. Cost per pass divides all five
low attempts' cost by the low passes.

| Rank | Task | Split | Score (shape, check, Fable low, time) | Luna-sized | Fable low: passes, mean trial time, cost per pass | Fable, all efforts | CPUs |
| ---: | --- | --- | --- | --- | --- | --- | ---: |
| 1 | `payments-pipeline-fix` | Confirmation | 6.0 (2, 1, 2.0, 1.0) | Yes | 5 of 5, 26.7 min, $6.25 | 25 of 25 | 4 |
| 2 | `mp-checkpoint-consolidation` | Development | 6.0 (1, 2, 2.0, 1.0) | Yes | 5 of 5, 26.7 min, $5.93 | 25 of 25 | 4 |
| 3 | `cumulative-layout-shift` | Confirmation | 5.5 (2, 1, 2.0, 0.5) | No: Fable low's time, 83 minutes | 5 of 5, 83.4 min, $14.38 | 22 of 25 | 4 |
| 4 | `live-database-cutover` | Development | 4.6 (1, 1, 1.6, 1.0) | Yes | 4 of 5, 35.5 min, $12.61 | 17 of 25 | 16 |
| 5 | `telecom-entity-resolution` | Confirmation | 4.5 (1, 0, 2.0, 1.5) | No: no local ground truth | 5 of 5, 22.1 min, $6.05 | 25 of 25 | 2 |
| 6 | `photonic-waveguide-routing` | Development | 3.6 (0, 1, 1.6, 1.0) | No: an optimization | 4 of 5, 38.6 min, $11.88 | 16 of 25 | 2 |

**Split rule, fixed before the split was computed.** Order the family by
total score, descending, with ties broken by the SHA-256 of the task id,
ascending. Pair ranks 1 and 2, 3 and 4, and 5 and 6. Within each pair, the
task whose SHA-256 of its id (UTF-8, no newline) is smaller is a
confirmation task, and the other is a development task. Pairing by score
keeps the halves alike; the hash decides within each pair. The digests are
in `tasks.json`.

- **Development tasks** run in this experiment like the others. Their
  traces may be read after grading, and a later policy may be developed on
  them.
- **Confirmation tasks** decide the hypothesis. Their traces aren't read
  for lessons, now or when a later policy is developed: only outcome fields
  (verifier result and test counts, cost, time, and session outcomes) are
  read. They stay held out until the family is retired.

## The pinned policy

| Field | Value |
| --- | --- |
| Manifest | [`crates/coder-one/policies/microluna-v15.json`](../../../../crates/coder-one/policies/microluna-v15.json), `coder-one-microluna-v15` |
| File SHA-256 | `161f51c78fc0f0f15949fc77f20e20cfaef5200a2cbd19a747bbec396d0dad0b` |
| Resolved policy digest | `d3e1396f7b5ad7b36ec39ade8ac483dc803d375d0d844916dfd1feafe2af2ea1`, the `policy_digest` all seven v15 trials of 2026-09-24 recorded |
| Artifact | `coder-one 0.1.0 (83b48ccc08c8)`, built from `83b48ccc08c8f3ed7f5d25e41594e2efb348d105`, the artifact of the dev and held-out v15 trials |

**Why v15.** It's the best existing manifest by the record: the cheapest
and fastest configuration that passes the dev task it can pass, and v16 and
v17 changed effort and practices without a gain. It's the policy the
held-out test set measured at 0 of 4, so this result sits beside that one.
It's also the baseline arm that step 3 of the assessment names: each use of
a new signal is matched against v15, and this run gives that match its
baseline on the same tasks.

What v15 runs, from its manifest: GPT-6 Luna at the provider's default
effort, Jev's file survey and coverage briefing, up to 4 lean sessions and
a fresh self-check inside a 25-minute wall bound, a frozen self-score with
keep-best, a host that turns back early finishes, Jev-ranked suspects, a
hard-coding scan, and a $0.09 Luna spend bound per trial. `verify.checks`
and `verify.support` are off.

**Launch conditions.** The same harness as the dev and held-out v15 trials:
`--without-claude`, the #9599 login protection, the trial network
allowlist, and the memory caps. TB4 at `v4.0.0`
(`452bf305c6daa62fc59061d22133a7cbc7c1572e`). Before launch, record the
host, the `bench/terminal-bench` commit, and the artifact's SHA-256. A trial
whose recorded `policy_digest` differs from the pinned one is void, and so
is the run if the policy, the artifact, or the harness changes during it.

## Procedure

1. **Attempts.** 3 on every family task: 18 counted trials, 9 of them on
   confirmation tasks.
2. **Order.** Attempt 1 of all six tasks, then attempt 2, then attempt 3.
   Within a round, launch in rank order as the host's CPU budget allows.
   `live-database-cutover` reserves 16 CPUs.
3. **Stopping rule.** Every counted trial runs; there's no early stop for
   success or futility. The run stops early only when the cost ceiling is
   reached, when the run is invalid, or when the operator stops it for the
   host's safety; each of these makes the result inconclusive.
4. **Infrastructure failures.** A trial that fails before the agent's first
   model request (an environment build, the egress probe, the host's disk)
   or whose verifier doesn't run is rerun once, and both records are kept.
   It counts as neither a pass nor a fail. The run is invalid if more than
   3 of the 18 trials can't be graded after their rerun. A trial the
   policy's own bounds end, such as the wall bound, is a counted failure.
5. **Cost ceiling.** $3.00 of recorded Luna and Jev spend at list price,
   reruns included. No Claude quota is used.

## Counting cost and time

- **A trial's cost** is every Luna request at list price plus every Jev
  request, from the trial's records. When a request was still open at a
  bound and its usage was never reported, the recorded cost is a lower
  bound. For the threshold, such a trial counts at the larger of its
  recorded cost and v15's $0.09 Luna spend bound plus its recorded Jev
  cost.
- **Cost per pass** is the sum over a task's 3 counted attempts, failed and
  time-bounded ones included, divided by its passes. It's undefined with no
  pass.
- **Fable's cost per pass** is from the same public file: the five low
  attempts' cost divided by their passes, as in the table above.
- **Trial time** is the Harbor trial's start to its end: environment build,
  agent, and verifier. That matches Fable's trial time, which is also start
  to end. **Trial time per pass** is the sum over the 3 counted attempts
  divided by the passes, compared with Fable low's trial time per pass.
  Agent time is reported too, but never compared with Fable's trial time.

## Outcomes, fixed now

A confirmation task is a **cheap win** when at least 2 of its 3 attempts
pass and its cost per pass is under 10% of Fable 5.1 low's cost per pass on
that task: under $0.625 for `payments-pipeline-fix`, $1.438 for
`cumulative-layout-shift`, and $0.605 for `telecom-entity-resolution`. The
10% bar is the determinism thesis's prediction 3, an order of magnitude.
At v15's spend bound it binds only if a trial overruns its bounds; the
reliability bar is the real test.

| Result | Condition |
| --- | --- |
| **Win** | At least 2 of the 3 confirmation tasks are cheap wins. |
| **Loss** | No confirmation task passes 2 of 3, and at most 1 of the 9 confirmation attempts passes. |
| **Inconclusive** | Anything else: one cheap win, passes spread too thin to reach 2 of 3, or an invalid or stopped run. |

What each result means:

- **A win** supports the claim "on untouched TB4 repair and build tasks
  that Fable low passes reliably, a pinned Luna policy passes reliably at
  under a tenth of Fable's cost", limited to this family. Its time label
  says whether that work is also faster or slower per pass.
- **A loss** means the embedding result doesn't extend to the tasks most
  like it that remain, and the cheaper-work lane waits on the signal
  (#9584) or a new Luna, as the capability-gap log's reopen conditions
  say. With 0 of 9, the 95% Wilson interval on v15's pass rate on this
  half is 0% to 30%.
- **An inconclusive result** is reported as it stands, with the per-task
  results; it doesn't license re-running confirmation tasks until they
  pass.

The report also states, as description and not as a test:

- The pooled confirmation pass rate with a 95% Wilson interval.
- The same labels on the development tasks, marked as in-family
  development evidence.
- Passes on the three Luna-sized tasks against the other three: the score
  predicts that the Luna-sized ones pass more often.
- Each failure's kind: a signal gap when the self-score was full on a
  failing trial, a capability gap when no session produced a passing
  candidate, and the bound that ended a time-bounded trial.
- Verifier test counts for every trial, and each task's trial time per pass
  against Fable low's, labeled faster or slower.

## Estimated cost and wall time

- **Cost.** v15's recorded trials cost $0.015 to $0.079 each, and its spend
  bound holds Luna to $0.09 a trial. Eighteen trials are expected to cost
  $0.50 to $1.80 with Jev, under the $3.00 ceiling even with reruns. Fable
  low's own attempts on the six tasks cost $5.93 to $14.38 per pass.
- **Wall time.** Each trial is an environment build, at most 25.5 minutes
  of agent time, and a verifier with a 5- to 25-minute bound. Estimate 30
  to 40 minutes a trial, about 10.5 trial-hours in all: about 5 hours of
  wall time with two trials at a time, or about 3 hours with four, within
  the host's 24-CPU budget.

## What this run doesn't test

- **Not a matched-model comparison.** Microluna on Luna and Claude Code on
  Fable differ in model, harness, and host. The comparison is with Fable's
  public trajectories, as every Microluna report states.
- **Not a random sample of TB4.** A rule chose the family from public data;
  the claim holds for tasks of this shape, not for the suite.
- **Small numbers.** Three attempts per task give wide intervals; a win
  here is a first definitive result, not a reliability estimate.
- **Not the pass-where-Fable-fails lane.** That lane waits on the signal,
  as the assessment says.

## Coordination

The #9584 prospective cohort (`distributed-dedup`, `formal-crypto`,
`freecad-impeller`, `freecad-spring-clip`, `math-eval-grader`,
`pretrain-shard-corruption`, `shadow-relay`, and `vpp-loss-divergence`) and
this family share no task. Until this run is graded, no Luna or Microluna
trial, Luna review, or signal calibration should use the six family tasks,
and no guidance, practice, or instruction text should name them or their
facts.

## Reproduce the selection

From the repository root:

```sh
python3 bench/terminal-bench/experiments/2026-09-25-luna-sized-family/select.py
cargo run -q -p coder-one -- contamination check
```

`select.py` reads only retained repository files and rewrites `tasks.json`
byte for byte. The benchmark host's job list (for R3) was read by hand on
2026-09-25 and is recorded in each task's reasons.
