# Stall detection and next-step choice: offline protocol

Issue [#9627](https://github.com/OpenAgentsInc/openagents/issues/9627). This
protocol was written, and the task split, question wording, code rule, label
rule, and threshold-selection rule were fixed, before any Jev answer or label
was read. The thresholds themselves are chosen from calibration labels only,
by the rule below, and written to `selection.json` before any evaluation
label is read.

## Data

Every retained Microluna dispatch under `~/.openagents/terminal-bench/jobs`
and `bench/terminal-bench/traces`, read by
`coder-one component replay control.stall`. A trial is one attempt's
sessions, in dispatch and session order. A copy of a trial that appears
twice is read once. Interrupted copies and live mirrors are skipped.
`sources.json` pins each file read by SHA-256.

## Split by task

`split.json` holds the partition of each task:

- **Calibration:** `embedding-drift-monitor`, `sound-change-cascade`, and
  `interleaved-vigenere`, the Microluna development set. These traces have
  been studied repeatedly.
- **Evaluation:** `session-window-debug`, `fin-saccr-rwa`,
  `gsea-proteomics`, `shadow-relay`, `coq-block-bound`, `uefi-bootkit`, and
  `html-js-filter`.
- **Excluded:** `mvcc-lsm-compaction`, `wal-recovery-ordering`, and
  `cad-model`. They come from the best-of-N experiments (#9587), whose
  candidates run in parallel, so one candidate's stall is not the
  dispatch's. They were excluded after counting checkpoints per task and
  before any label or answer was read.

What was known before freezing: the trial-level outcomes of the evaluation
tasks are published (the v15 test set scored 0 of 4, and the
candidate-evidence experiment reported its session-window results). The
issue text describes `shadow-relay` and `coq-block-bound` as stalls. No
checkpoint-level label was computed or read for any task before this
protocol.

## Checkpoints

Inside each session, every 8 turns from turn 8. At the end of every
session but the attempt's last. The lean loop can act only at session
ends, so they are reported apart from in-session checkpoints.

## Code features and the code rule

Fixed in `crates/coder-one/src/stall.rs`, not tuned:

- **Quiet:** at least 8 turns since the last edit and since the score
  last rose.
- **Looping:** at least 2 failed commands in the 12-event window that
  repeat an earlier failed command, or at least 4 repeated commands.
- **Turned back:** at least 2 finishes the host turned back in the
  session since its last edit.
- **Suspect** is any of the three. **Strong** is quiet and looping, or
  turned back.

## Jev questions

Question set `stall-v1`: three Nouls (`repeating`, `progress`, and `done`)
and one Choice (`next`) in one request per checkpoint, over the task, the
last 12 events, the measured features without timing, and the candidate
next steps. The wording is the constants `Q_REPEATING`, `Q_PROGRESS`,
`Q_DONE`, `Q_NEXT`, and `CONTINUE_OPTION` in `stall.rs` at the commit that
adds this file.

## Calls

- **Cascade (the call code acts on):** suspect, and Jev's `progress` below
  `progress_below` or `repeating` at or above `repeating_at`. Without a Jev
  answer, strong.
- Also reported: code suspect alone, code strong alone, and Jev alone.
- `done` is reported only. No call uses it.

## Labels

From what happened after the checkpoint, in the same attempt:

- **Progress after** is a later score that beats the best earlier score
  with the same total, the first score with a new total that passes
  anything, or an attempt that passed its verifier. A pass labels every
  checkpoint as progress, because a stop there could have lost it.
- **Stall** is no progress after. A checkpoint with fewer than 3 agent
  turns after it isn't labeled.
- Scores are `SCORE <passed> <total>` lines in command output, the host's
  score in a turned-back finish, and the host's score after each session in
  the loop record.
- **Productive next step:** a score gain within 8 turns of the checkpoint.
  The next step's kind is the first call after the checkpoint, classified
  as `run_example`, `run_tests`, `read_region`, `edit`, or `other`.

## Threshold selection

On calibration checkpoints only, pooled: `progress_below` in {0.1, 0.2,
0.3, 0.4, 0.5} and `repeating_at` in {0.5, 0.6, 0.7, 0.8, 0.9, never}.
Choose the highest cascade recall with precision at least 0.80; break ties
by higher precision, then lower `progress_below`, then higher
`repeating_at`. If no setting reaches 0.80, choose the highest precision
among settings with at least 5 calls, then recall. `measure.py --select`
applies the rule.

## Analysis

`measure.py` reports precision and recall with 95% Wilson intervals, by
partition, by checkpoint kind, and by task, and a bootstrap that resamples
whole tasks (10,000 samples, seed 9627) for the cascade's precision and
recall and their difference from code suspect alone. Undefined resamples are
counted and left out. The next-step pick is scored as the share of next
steps matching Jev's pick that were productive (precision) and the share
of productive next steps Jev had picked (recall). This is association, not
a causal test: no session was steered by a pick.

## Budget

Offline Jev calls only, under $0.10 in total, recorded in
`jev-recorded.json` so the measurement replays without calls. No Luna
session and no Terminal-Bench trial.
