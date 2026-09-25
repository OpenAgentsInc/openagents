# Truthful checks, calibrated against graded runs

Historical measurement. The [2026-09-25 Microluna audit](2026-09-25-truthful-checks-microluna.md)
adds selected-candidate reports and attributable read-only reviews, measures a
frozen corroboration rule, and reports task-cluster uncertainty. The comparison
partition below was inspected during model development, as the validity section
records; it is not untouched confirmation. Its precision does not transfer to
Microluna without a separate measurement.

On 2026-09-24, this offline measurement labeled every graded retained
Coder One trial with its verifier reward. It measured how well each check
signal separates passes from failures, and fitted a combined verdict on
tasks the measurement never saw. It's the first algorithm of the
[Luna pivot](../coder/design/luna-pivot.md)
([#9584](https://github.com/OpenAgentsInc/openagents/issues/9584)). No
Terminal-Bench trial ran for it. The only model spend was $0.0236 of Jev.

## Summary

- **Today's checks don't tell the truth.** On the held-out tasks, a
  failed final check was right 6 times in 11 (55%, 28–79%). It caught 6
  of 60 failures (10%, 5–20%). When every scenario passed, the verifier
  passed 69 of 97 trials (71%), about the base rate of 68%. No scenario
  kind, requirement state, or Jev support state separates passes from
  failures on both halves of the label set.
- **The combined verdict catches almost four times as many failures, as
  precisely.** On 185 held-out trials of 32 tasks, the verdict called 37
  trials failed, and 22 of them failed (59%, 43–74%). It caught 22 of
  the 60 failures (37%, 26–49%), where today's checks caught 6. On the
  same trials the difference in recall is significant: exact McNemar
  p = 0.0009. When it called a trial passed, 11 of 13 passed (85%,
  58–96%).
- **The precision gain isn't significant.** 59% against 55% is within
  noise. The verdict is as precise as today's checks, not measurably more.
- **The signal is in the executor's final report, not in the checks.**
  The verdict reads three things: Jev's answer to whether a strict grader
  would accept the reported result, whether the report admits an unmet
  requirement, and the existing self-report detector.
- **Two policy options apply the result without deleting a check.**
  `verify.distrust` reads failures of the named scenario kinds as
  inconclusive. `verify.verdict` records the verdict on the first and
  final candidates, and `verify.second.on: ["verdict"]` escalates on it.
  Both are off by default.
- **The verdict is measured mostly on Opus.** The label set has 88 GPT-6
  Luna trials, and only 6 held-out Luna failures. On those, the verdict
  called 3 trials failed and caught 1 failure. Luna's precision is
  unmeasured until more Luna trials are graded.

The issue's bar was a verdict whose fail precision and failure recall,
measured with intervals on held-out tasks, beat today's checks. Recall
beats them clearly. Precision matches them but isn't measurably better,
so the issue stays open. [What's needed](#whats-needed-to-close-the-issue)
says what would close it.

## Protocol

| Field | Value |
| --- | --- |
| Question | Which check signals separate verifier passes from failures, and does a verdict calibrated on some tasks separate them on others? |
| Label set | Every trial under `~/.openagents/terminal-bench/jobs/` with a Coder One composition record and a verifier reward, then any trace under `bench/terminal-bench/traces/` the jobs didn't already give. Each trial counts once. Trials with a Harbor agent exception or a usage-limited session are left out, and none was. |
| Label | The verifier's reward: 1.0 is a pass, and anything lower is a failure. Every reward in the set is 0 or 1. |
| Split | By task, on the parity of the last hex digit of the digest of the task name, fixed before any signal was measured. No task is in both halves. |
| Signals | What the episode recorded about the candidate the verifier graded: the scenario verdicts of the check whose candidate digest matches `final_checks`, the requirement states in that check, the support report for that candidate, the self-report on that check and on the first check, whether the first check failed a scenario, and whether repair or a second-executor trigger fired. Also the self-report detector rerun on the final report of every trial, and five Jev questions over the task and the final report. |
| Final report | The last session's final report, or the first session's when a second executor ran and its candidate was set aside. |
| Report questions | Five Nouls in one request on `jev-1.13.0`, over `task` (the public instruction) and `report`, each clipped to 7,000 characters, keeping the head and the tail. `checks::verdict::report_questions` has the wording. |
| Verdict | A ridge logistic score over three features, fitted by gradient descent from zero on the calibration half only. The cutoffs are the lowest score at which calibration fail precision reaches 80% and the highest at which calibration pass precision reaches 85%, each over at least 5 trials. |
| Feature choice | Leave-one-task-out cross-validation on the calibration half, among six candidate feature sets. The set chosen had the highest cross-validated AUC, 0.686. |
| Intervals | 95% Wilson intervals. The paired comparison with today's checks is an exact two-sided McNemar test on the held-out failures. |
| Cost | 317 report requests, 560,878 input tokens, $0.0236 of Jev at $0.042 per million input tokens. The answers are recorded, so a rerun replays them for nothing. |

Reproduce it:

```sh
coder-one checks truth                # scan, ask Jev for what isn't recorded, measure
coder-one checks truth --jev recorded # replay the recorded answers only
coder-one checks truth --rows crates/coder-one/fixtures/truth/rows.jsonl
gym coder truth                       # the held-out table
gym coder truth --set all --within    # within tasks that both passed and failed
```

## The label set

317 graded trials on 58 tasks: 195 passes and 122 failures.

| Half | Trials | Passes | Failures | Tasks | Tasks with both outcomes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Calibration | 132 | 70 | 62 | 26 | 9 |
| Held-out | 185 | 125 | 60 | 32 | 12 |

The first executor was Claude Code on Opus 5.5 in 223 trials, Codex on
GPT-6 Luna in 88, and Codex on GPT-6 Astra in 6. The largest groups are
`tunable-v2` (63), `tunable-v9-escalate` (32), `matched-v8` (30), and
the four Luna arms (75 together). The most-sampled tasks are
`cad-model` (16), `interleaved-vigenere` (15), `cargo-flight-dispatch`
(15), and `production-planning` (14). 23 tasks have a single trial. The
per-task spread is in the JSON beside this document, under
`label_set.per_task`.

The issue estimated about 600 graded retained runs. That count includes
Claude Code, Codex, and Fable trials, which have no composition record
and so no check signals. Every one of the 317 Coder One trials came from
local jobs. The retained traces only duplicate them.

## Each signal's discrimination

Fail precision is the share of the trials a signal calls failed that
the verifier failed. Failure recall is the share of the verifier's
failures the signal calls failed. The last column is the verifier's
pass rate on the trials the signal calls passed. A dash means the signal
never says that.

### Held-out half

185 trials, 60 failures. The base failure rate is 32%, so a fail
precision whose interval stays above 32% separates. The base pass rate
is 68%.

| Signal | Fail precision | Failure recall | Pass rate when it says pass |
| --- | --- | --- | --- |
| `checks.final` (today's checks) | 6/11, 55% (28–79%) | 6/60, 10% (5–20%) | 69/97, 71% (61–79%) |
| `checks.first-flagged` | 12/27, 44% (28–63%) | 12/60, 20% (12–32%) | — |
| `checks.scenario-failed` | 3/7, 43% (16–75%) | 3/60, 5% (2–14%) | — |
| `scenario.behavior.filter-preserves` | 2/4, 50% (15–85%) | 2/60, 3% (1–11%) | 1/2, 50% (9–91%) |
| `scenario.behavior.filter-removes` | — | 0/60 | 3/6, 50% (19–81%) |
| `scenario.cancellation` | 0/1 | 0/60 | 11/11, 100% (74–100%) |
| `scenario.generic.claimed-command` | 0/1 | 0/60 | — |
| `scenario.generic.output` | 1/1, 100% (21–100%) | 1/60, 2% (0–9%) | 81/130, 62% (54–70%) |
| `scenario.generic.parse` | — | 0/60 | 62/88, 70% (60–79%) |
| `scenario.generic.self-report` | 3/4, 75% (30–95%) | 3/60, 5% (2–14%) | — |
| `scenario.interactive.interrupt` | — | 0/60 | 12/12, 100% (76–100%) |
| `scenario.interactive.program` | — | 0/60 | 12/12, 100% (76–100%) |
| `requirements.contradicted` | 6/11, 55% (28–79%) | 6/60, 10% (5–20%) | — |
| `requirements.unverifiable` | 19/42, 45% (31–60%) | 19/60, 32% (21–44%) | — |
| `requirements.observed-none-contradicted` | — | 0/60 | 78/124, 63% (54–71%) |
| `support.contradicted` | 2/4, 50% (15–85%) | 2/60, 3% (1–11%) | — |
| `support.supported` | — | 0/60 | 5/6, 83% (44–97%) |
| `support.unresolved` | 36/79, 46% (35–57%) | 36/60, 60% (47–71%) | — |
| `self-report.recorded` | 3/4, 75% (30–95%) | 3/60, 5% (2–14%) | — |
| `self-report.first` | 6/10, 60% (31–83%) | 6/60, 10% (5–20%) | — |
| `self-report.detector` | 12/16, 75% (51–90%) | 12/60, 20% (12–32%) | — |
| `control.repaired` | 5/10, 50% (24–76%) | 5/60, 8% (4–18%) | — |
| `control.second-fired` | 7/10, 70% (40–89%) | 7/60, 12% (6–22%) | — |
| `report.admits_unmet` at 0.5 | 11/23, 48% (29–67%) | 11/60, 18% (11–30%) | 113/162, 70% (62–76%) |
| `report.rests_on_reading` at 0.5 | 45/105, 43% (34–52%) | 45/60, 75% (63–84%) | 65/80, 81% (71–88%) |
| `report.left_untested` at 0.5 | 15/24, 62% (43–79%) | 15/60, 25% (16–37%) | 116/161, 72% (65–78%) |
| `report.checked_against_task` at 0.5 | 57/156, 37% (29–44%) | 57/60, 95% (86–98%) | 26/29, 90% (74–96%) |
| `report.strict_grader_accepts` at 0.5 | 60/185, 32% (26–39%) | 60/60, 100% (94–100%) | — |
| **`verdict.combined`** | **22/37, 59% (43–74%)** | **22/60, 37% (26–49%)** | **11/13, 85% (58–96%)** |

The report questions read at a 0.5 cutoff are shown for reference. The
verdict reads two of them as probabilities, and at 0.5 the grader
question answers "no" on almost every trial.

### All trials

These pool both halves, so the verdict's row is partly in-sample: its
calibration half is where it was fitted.

| Signal | Fail precision | Failure recall | Pass rate when it says pass |
| --- | --- | --- | --- |
| `checks.final` (today's checks) | 11/21, 52% (32–72%) | 11/122, 9% (5–15%) | 106/156, 68% (60–75%) |
| `checks.first-flagged` | 35/65, 54% (42–65%) | 35/122, 29% (21–37%) | — |
| `scenario.generic.output` | 4/8, 50% (22–78%) | 4/122, 3% (1–8%) | 127/215, 59% (52–65%) |
| `scenario.generic.parse` | — | 0/122 | 95/150, 63% (55–71%) |
| `scenario.generic.public-command` | — | 0/122 | 17/19, 89% (69–97%) |
| `scenario.behavior.named-command` | 1/1 | 1/122 | 0/11, 0% (0–26%) |
| `requirements.unverifiable` | 35/66, 53% (41–65%) | 35/122, 29% (21–37%) | — |
| `support.contradicted` | 6/10, 60% (31–83%) | 6/122, 5% (2–10%) | — |
| `support.supported` | — | 0/122 | 7/29, 24% (12–42%) |
| `support.unresolved` | 50/107, 47% (38–56%) | 50/122, 41% (33–50%) | — |
| `self-report.first` | 18/24, 75% (55–88%) | 18/122, 15% (10–22%) | — |
| `self-report.detector` | 24/30, 80% (63–90%) | 24/122, 20% (14–28%) | — |
| `control.second-fired` | 18/23, 78% (58–90%) | 18/122, 15% (10–22%) | — |
| `report.admits_unmet` at 0.5 | 37/55, 67% (54–78%) | 37/122, 30% (23–39%) | 177/262, 68% (62–73%) |
| `verdict.combined` | 56/79, 71% (60–80%) | 56/122, 46% (37–55%) | 31/36, 86% (71–94%) |

`gym coder truth --set all` prints every row. The Jev support signal
points the wrong way: when support read a requirement as supported and
none as contradicted, the verifier passed 7 of 29 trials (24%), far below
the base rate of 62%. 20 of its 22 failures are on two tasks:
`cargo-flight-dispatch` (13) and `vba-userform-port` (7).

### Within a task

Best-of-N and persistence compare attempts at one task, so a signal has
to separate a pass from a failure of the same task. On the 12 held-out
tasks with both outcomes (110 trials, 51 failures):

| Signal | Fail precision | Failure recall |
| --- | --- | --- |
| `checks.final` | 6/9, 67% (35–88%) | 6/51, 12% (6–23%) |
| `checks.first-flagged` | 11/17, 65% (41–83%) | 11/51, 22% (12–35%) |
| `self-report.detector` | 10/13, 77% (50–92%) | 10/51, 20% (11–32%) |
| `control.second-fired` | 7/10, 70% (40–89%) | 7/51, 14% (7–26%) |
| `verdict.combined` | 19/30, 63% (46–78%) | 19/51, 37% (25–51%) |

The verdict's recall holds within a task. Its false alarms concentrate on
one task. On `production-planning` it called 5 of 7 failures and 5 of 7
passes failed, because Opus hedges about the grader's reading of the
routing durations whether or not its answer is right.

## The combined verdict

`checks::verdict` gives pass, fail, or unknown for one candidate. It
reads three features:

| Feature | Weight | What it is |
| --- | ---: | --- |
| `report.strict_grader_accepts` | −1.10 | Jev's probability that a strict automated grader would accept the result the report describes |
| `report.admits_unmet` | +2.25 | Jev's probability that the report says part of the task is not done, fails, or doesn't meet a requirement |
| `self-report.detector` | +1.20 | 1 when the existing phrase detector finds an admission in the final report |

The bias is −0.66. The failure probability is the logistic of the
weighted sum. The verdict is fail at 0.553 or above, pass at 0.297 or
below, and unknown in between. It's unknown when Jev didn't answer.

On the calibration half, the fail cutoff held 81% precision over 42
trials, and the pass cutoff 87% over 23. On the held-out half those
became 59% and 85%. Each call states the held-out precision, 22/37 for
fail and 11/13 for pass, because the calibration precision overstates
it on tasks the fit never saw. `fixtures/truth/rows.jsonl` holds the
label rows, and the tests refit the parameters from them and measure the
held-out numbers again.

Feature choice, by leave-one-task-out AUC on the calibration half:

| Feature set | Cross-validated AUC |
| --- | ---: |
| grader, admission, detector (chosen) | 0.686 |
| grader, admission, detector, a failed scenario | 0.682 |
| all five report questions and the detector | 0.648 |
| grader, admission | 0.647 |
| grader, detector | 0.545 |

The grader question alone scores 0.394 there. That's an artifact of
refitting the intercept for each left-out task, not a reversal: its
in-sample AUC is 0.733 on calibration and 0.729 on held-out. Adding a
failed scenario changes nothing, which is the scenarios' result in one
number.

### Where it plugs in

- **Escalation.** With `verify.verdict: true` and `verify.second.on`
  naming `verdict`, a second executor runs when the verdict calls the
  first candidate failed. The composition record's `verdict` field holds
  the first and final verdicts with their evidence, and the Jev key that
  replays them.
- **Persistence and best-of-N.** `checks::verdict::assess` takes the
  task, a final report, and the Jev mode, and returns the evidence, the
  verdict, and its stated precision. A persistence round or a race keys
  on `Verdict::p_fail` to rank candidates, or on the call to stop.
- **Microluna handoffs.** A Microluna session ends with a typed result
  whose summary is its report, so the same call applies unchanged.

The escalation doc's failure mode still holds: comparing self-reports
across executors rewards the one that says less. The verdict reads the
report, so it inherits that bias. Keep a candidate on the verdict only
between candidates from the same executor.

## Signals removed or down-weighted

Nothing was deleted. The verdict leaves out every signal that didn't
separate on the calibration half, and the policy can distrust scenario
kinds:

| Signal | Calibration evidence | Action |
| --- | --- | --- |
| `scenario.generic.output` failures | 3 of 7 were failures (43%), below the 47% base rate | Distrust with `verify.distrust: ["generic.output"]` |
| `scenario.behavior.filter-preserves` failures | No calibration trial. 2 of 4 were failures overall, and the escalation experiment saw it fire on 2 passes. | A candidate for distrust once calibration data exists |
| Requirement states, support states, `checks.final`, and `checks.first-flagged` | None separated on calibration. | Left out of the verdict |
| `control.repaired` and `control.second-fired` | These fire on other signals, so they aren't independent evidence. | Left out |
| `report.rests_on_reading`, `report.left_untested`, and `report.checked_against_task` | These added nothing in cross-validation, and they flip direction between halves. | Asked but not weighted |

A distrusted failure still runs and keeps its observations, reads as
inconclusive with a note naming `verify.distrust`, contradicts no
requirement, and leaves no repair packet. So the `check` escalation
trigger, repair, and `Standing` all stop counting it.

## Behavior checks from the task's words

The issue asked for generators that run the task's example, compare
outputs, and test stated edge cases, measured the same way. The retained
records don't allow it yet:

- **Few tasks state an example with its output.** 17 of the 58 tasks'
  instructions have a code block. One, `hof-topology-interpenetration`,
  shows expected output values. One, `build-cython-ext`, shows a runnable
  snippet without its result. The rest are output schemas, function
  signatures, or commands to run.
- **The replay can't run them.** `coder-one checks recall` rebuilds a
  trial's files from the task's public `COPY` lines and Harbor's
  collected outputs, not the image's installed toolchain. A task's own
  command therefore reads as `unavailable`. 59 trials have a
  post-executor snapshot, but a snapshot holds the workspace, not the
  image.
- **The generators that exist are already measured.** The
  `behavior.*` scenarios come from the task's words. They fire rarely,
  and `behavior.named-command` passed 11 trials that all failed.

A schema-conformance check is the one generator the records can feed.
The tasks with a schema block hold 4 failures in the whole set, all on
calibration tasks (`atrx-vep-crispr` 3, `glycan-ms2-elucidation` 1), and
none on held-out tasks, so it can't be measured here. Measuring example-run and edge-case generators
needs live runs inside the task images, on tasks that state examples,
which the development panel has more of than TB4.

## What's needed to close the issue

- **More failures on new tasks.** The held-out half holds 60 failures,
  and the verdict called 37 trials failed. To state its fail precision
  to within ±10 points takes about 90 fail calls: about 450 graded trials
  on tasks the fit never saw, at the held-out rate. To show a precision
  gain over today's checks, the verdict needs a real lift in the signal,
  since 59% against 55% would take thousands of calls to separate.
- **Luna trials.** The pivot's executor has 88 trials in the set and 6
  held-out failures. On those, the verdict called 3 trials failed and
  caught 1 failure. Codex's final reports on Luna are much shorter, with
  a median of 311 characters against Opus's 2,707. They also admit less:
  the detector finds an admission in 3% of them against 12% of Opus's.
  So the report questions have less to read. The Luna TB4
  experiment (#9583) produces the labels this needs. After each graded
  run, `coder-one checks truth` picks them up.
- **Candidates from one task.** Best-of-N needs within-task precision.
  There are 12 held-out tasks with both outcomes, and one of them,
  `production-planning`, holds most of the false alarms.

## Threats to validity

- **I saw held-out numbers during exploration.** While choosing the
  model, I printed held-out results for six feature sets and three
  precision targets. The chosen set and the 80% and 85% targets were
  picked on calibration-only criteria: cross-validated AUC and the targets
  the design named. But the held-out half isn't untouched.
- **Task-level confounding.** Pooled signals partly measure how hard a
  task is. The within-task table shows what survives.
- **The self-report detector's phrases** were chosen by reading many of
  these trials' reports (#9568). Its held-out numbers overstate it.
- **One report per trial.** Where a repair or persistence round ran, the
  verdict reads the last session's report, which may describe only that
  round.
- **The split is uneven.** It has 26 and 32 tasks, and calibration fails
  more often (47% against 32%), which lowers held-out precision at a
  fixed cutoff.

## Evidence

- Code: 9496e211eb (`coder-one checks truth`, `checks::verdict`,
  `verify.distrust`, `verify.verdict`, and the `verdict` trigger) and
  1dcb8d2cd5 (`gym coder truth`).
- Label rows with the recorded report answers:
  [`crates/coder-one/fixtures/truth/rows.jsonl`](../../crates/coder-one/fixtures/truth/rows.jsonl).
- Summary: [`2026-09-24-truthful-checks.json`](2026-09-24-truthful-checks.json),
  from `coder-one checks truth --jev recorded`.
- The recorded Jev answers:
  `~/.openagents/coder-one/checks-truth/jev-recorded.json` on the
  benchmark host.
