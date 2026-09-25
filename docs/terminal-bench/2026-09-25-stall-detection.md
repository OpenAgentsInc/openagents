# Stall detection and next-step choice for Microluna, measured offline

2026-09-25. Issue
[#9627](https://github.com/OpenAgentsInc/openagents/issues/9627), algorithms 4
and 5 of [the Luna pivot](../coder/design/luna-pivot.md).

**Code features alone find most of what the stall detector finds, and Jev
adds no precision on top of them.** On the evaluation tasks, the detector's
calls were right 30 of 35 times (86%, 71–94%) and caught 30 of 85 stalls
(35%, 26–46%). That precision is not distinguishable from the base rate:
83% of the evaluation checkpoints were stalls, because no evaluation trial
passed. Requiring Jev to confirm a code signal cost recall in both
partitions and left precision unchanged. The next-step choice showed no
established benefit. This is an offline measurement on retained
transcripts. It makes no live benchmark claim, and the detectors stay off
in every manifest.

## What was built

- **`control.stall`** (`crates/coder-one/src/stall.rs`). At a checkpoint,
  the end of a session or every 8 turns inside one, code reads the session
  log and computes features: turns since the last edit, turns since the
  score last rose, failed commands that repeat an earlier failure, and
  finishes the host turned back with no edit since. A checkpoint is
  *suspect* when the session is quiet (8 turns with no edit and no score
  gain), looping (2 repeated failures or 4 repeated commands in the last
  12 events), or turned back twice with no edit. Only a suspect checkpoint
  can be called a stall. Then one Jev request decides: three Nouls
  (`repeating`, `progress`, and `done`) over the task, the last 12 events,
  and the measured features. With no Jev answer, a strong code signal
  decides alone.
- **Actions** belong to code (`stall::action`). A first stall re-briefs the
  next session with the evidence: the quiet turns, the unchanged score, and
  the command that kept failing. A second stall right after a re-brief
  stops the work sessions; the self-check still runs when the policy has
  one.
- **Done detection is report-only.** The `done` answer is recorded in the
  loop record as `done_report_only`, and nothing reads it to end the loop.
- **`control.next`**. Code proposes next steps from the state: run the
  task's own example, rerun the tests the session ran, read the region the
  latest error points at, or edit the file worked on last. A Jev Choice in
  the same request picks one or defers to the session. The pick becomes a
  suggestion in the next brief only when its probability is at least 0.5.
  Luna still writes every edit.
- **The switch.** `executor.microluna.lean.detect` turns on `stall`,
  `next_step`, or both (`crates/coder-one/src/micro/detect.rs`, a small
  hook in `lean.rs`). It is absent from every manifest, so no existing
  policy's digest changes. No new manifest was added: nothing here
  justifies a matched run yet.
- **Tests** run the lean loop on scripted Luna replies: without the switch
  every session runs; a stalled session is re-briefed and a second stall
  stops the loop; and a recorded Jev answer that says the session is
  progressing keeps the loop going, carries the next-step suggestion, and
  leaves a done answer unused.

The older [`control.monitor`](../coder/guides/coder-one-components.md) stopped
working sessions 12 to 30 seconds in, on a flag that replays had found right
0 of 22 times ([tunable results](2026-09-23-tunable-results.md)). This
detector differs on purpose: it waits 8 turns, needs a code-side feature
before Jev matters, acts only between sessions, and re-briefs before it
stops.

## How it was measured

The [protocol](../../bench/terminal-bench/experiments/2026-09-25-stall-detection/protocol.md)
fixed the task split, question wording, code rule, label rule, and the
threshold-selection rule before any answer or label was read. Commit
`2caa97cd49` pushed the code, the protocol, and the thresholds the rule chose
from calibration labels alone, before the evaluation labels were read.
After the freeze, `measure.py` gained two reports, the base rate and the
simulated actions below; the selection rule and the calls didn't change.

- **Data.** Every retained Microluna dispatch on this machine and in
  `bench/terminal-bench/traces`: 68 trials in the two partitions, 450
  checkpoints, 429 of them labeled.
- **Split by task.** Calibration: `embedding-drift-monitor`,
  `sound-change-cascade`, and `interleaved-vigenere`, the development set.
  Evaluation: `session-window-debug`, `fin-saccr-rwa`, `gsea-proteomics`,
  `shadow-relay`, `coq-block-bound`, `uefi-bootkit`, and `html-js-filter`.
  The best-of-N tasks were excluded because their candidates run in
  parallel. The evaluation tasks' trial outcomes were already published,
  and the issue itself describes `shadow-relay` and `coq-block-bound` as
  stalls. No checkpoint label was read before the freeze.
- **Label.** A checkpoint is a stall when nothing after it in the same
  attempt made measurable progress: no later score beat the best earlier
  score, and the attempt failed. A pass makes every checkpoint progress,
  since a stop could have lost it. A checkpoint with fewer than 3 turns left
  isn't labeled.
- **Thresholds.** The rule picked the highest calibration recall with
  precision of at least 0.80: a stall when Jev's `progress` is below 0.5 or
  `repeating` is at least 0.5. Both sit at the edge of the fixed grid.

## Results

Precision is the share of stall calls that were stalls; recall is the share
of stalls called. Intervals are 95% Wilson intervals.

### Calibration tasks (327 labeled checkpoints, 66% stalls)

| Call | Calls | Precision | Recall |
| --- | ---: | --- | --- |
| Code: suspect | 121 | 109 of 121, 90% (83–94%) | 109 of 216, 50% (44–57%) |
| Code: strong | 10 | 8 of 10, 80% (49–94%) | 8 of 216, 4% (2–7%) |
| Jev alone | 131 | 106 of 131, 81% (73–87%) | 106 of 216, 49% (42–56%) |
| **Detector (suspect, then Jev)** | 88 | 80 of 88, 91% (83–95%) | 80 of 216, 37% (31–44%) |

### Evaluation tasks (102 labeled checkpoints, 83% stalls)

| Call | Calls | Precision | Recall |
| --- | ---: | --- | --- |
| Code: suspect | 54 | 45 of 54, 83% (71–91%) | 45 of 85, 53% (42–63%) |
| Code: strong | 3 | 3 of 3, 100% (44–100%) | 3 of 85, 4% (1–10%) |
| Jev alone | 49 | 42 of 49, 86% (73–93%) | 42 of 85, 49% (39–60%) |
| **Detector (suspect, then Jev)** | 35 | 30 of 35, 86% (71–94%) | 30 of 85, 35% (26–46%) |

At session ends, where the lean loop can act, the detector was right 15 of
16 times (94%, 72–99%) and caught 15 of 41 stalls (37%, 24–52%). But 41 of
the 42 labeled session ends were stalls, so a detector that always said
stall would have scored higher precision.

### By evaluation task

| Task | Trials (passed) | Stalls of labeled | Detector: right of calls | Detector recall | Code suspect: right of calls |
| --- | --- | --- | --- | --- | --- |
| `coq-block-bound` | 2 (0) | 12 of 12 | 6 of 6 | 6 of 12, 50% (25–75%) | 6 of 6 |
| `fin-saccr-rwa` | 1 (0) | 2 of 2 | none called | 0 of 2 | none called |
| `gsea-proteomics` | 3 (0) | 18 of 21 | 7 of 8 | 7 of 18, 39% (20–61%) | 8 of 9 |
| `html-js-filter` | 1 (0) | 9 of 9 | 3 of 3 | 3 of 9, 33% (12–65%) | 3 of 3 |
| `session-window-debug` | 9 (0) | 16 of 22 | none called | 0 of 16, 0% (0–19%) | none called |
| `shadow-relay` | 2 (0) | 11 of 19 | 5 of 9 | 5 of 11, 45% (21–72%) | 11 of 19 |
| `uefi-bootkit` | 1 (0) | 17 of 17 | 9 of 9 | 9 of 17, 53% (31–74%) | 17 of 17 |

On calibration, the detector was right 44 of 44 times on
`embedding-drift-monitor`, 31 of 34 on `interleaved-vigenere`, and 5 of 10
on `sound-change-cascade`, where later score gains were common even in
failing attempts.

### Does Jev add anything over the code features?

A bootstrap that resamples whole tasks (10,000 samples, seed 9627) compares
the detector with the code's suspect call alone:

| Partition | Precision difference | Recall difference |
| --- | --- | --- |
| Calibration (3 tasks) | −7.1 to +4.4 points | −24.1 to −3.6 points |
| Evaluation (7 tasks) | −1.8 to +6.4 points | −37.1 to −1.1 points |

The precision intervals include zero; the recall intervals don't. Requiring
Jev's confirmation lost detections and gained no measurable precision.

## What held up

- **No call in a passing attempt.** The 12 passing calibration trials had no
  detector call and no code-suspect call. Jev alone made 4 calls in them;
  the code gate removed all 4. This is the older monitor's failure, which
  the gate prevents. No evaluation trial passed, so the evaluation data
  can't test it.
- **No simulated stop before progress.** Applying the lean hook's rule at
  session ends, a re-brief after the first stall and a stop after a stall
  right after a re-brief, gives 3 stops in each partition. None came before
  later progress, and they would have skipped 146 and 126 turns. All six
  are attempts with four or more sessions, mostly from the older
  requirements loop; most lean-loop attempts had one work session and a
  self-check, so only a re-brief could have applied to them. A re-brief would have changed the
  later sessions, so these counts don't predict a live run.
- **Done stayed report-only for good reason.** `done` reached 0.8 or more at
  3 checkpoints, all in failing attempts.

## Negative results

- **Jev's stall questions add nothing measurable to the code features.**
  The simplest defensible detector is the code's suspect call, with Jev
  recorded beside it.
- **Evaluation precision is uninformative.** With no passing evaluation
  trial, most checkpoints are stalls by construction, and precision sits at
  the base rate. The data can't show whether the detector would avoid a
  false stop on a task Luna can pass, beyond the calibration trials.
- **Wrong-but-done isn't a stall.** On `session-window-debug`, sessions
  finished quickly with a wrong answer. The label counts those checkpoints
  as stalls, but no stall feature fires, and none should. Those failures
  belong to truthful checks (#9584), not to this detector.
- **The next-step choice is not established.** On calibration, a next step
  that matched Jev's pick was followed by a score gain 19 of 62 times (31%,
  21–43%), against 39 of 174 (22%, 17–29%) when it didn't match. On
  evaluation, 4 of 6 (67%, 30–90%) against 4 of 25 (16%, 6–35%). The
  intervals overlap on calibration, and evaluation has 6 matches. The
  measure is association, not cause: no session was steered by a pick. Jev
  deferred to the session at 90 of 326 calibration and 63 of 94 evaluation
  checkpoints.
- **The labels measure progress, not being stuck.** A score gain in a
  failing attempt counts as progress, and an attempt without a score has
  no way to show progress except a pass. The thresholds sit on the edge of
  the grid, so the grid limited them.

## What would make a live claim possible

1. Replace the Jev gate with the code's suspect call, keep Jev's answers in
   the record, and remeasure on the same inputs; no new calls are needed.
2. Find retained Microluna attempts on tasks Luna sometimes passes, beyond
   `embedding-drift-monitor`, so false stops can be counted on held-out
   passes.
3. Only then, a pinned, matched run with the operator's approval: the same
   policy with and without `detect.stall`, comparing passes, turns, and cost.
4. Test the next-step choice causally, by steering a session with the pick,
   before it enters a brief.

## Reproduce

The [experiment directory](../../bench/terminal-bench/experiments/2026-09-25-stall-detection/)
holds the inputs (`records/inputs.jsonl`, one row per checkpoint), the labels
by partition, the rows of answers and calls, `sources.json` with the SHA-256
of every retained file read, and `jev-recorded.json` with all 450 answers.
The replay makes no model call:

```sh
python3 bench/terminal-bench/experiments/2026-09-25-stall-detection/replay.py \
  --coder-one /absolute/path/to/coder-one
```

It reproduced all 450 rows and the summary exactly
([`records/replay.txt`](../../bench/terminal-bench/experiments/2026-09-25-stall-detection/records/replay.txt)).
To rebuild the inputs from the traces, run
`coder-one component replay control.stall --traces DIR --split split.json --out DIR`.
The four component fixtures run with
`coder-one component suite control.stall` and `control.next`.

## Spend

450 live Jev calls, one per checkpoint: 340 calibration calls for
1,128,068 input tokens ($0.0474) and 110 evaluation calls for 365,333
input tokens ($0.0153), **$0.0627 in total** at the repository's pinned
$0.042 per million input tokens. No Luna session and no Terminal-Bench
trial ran.
