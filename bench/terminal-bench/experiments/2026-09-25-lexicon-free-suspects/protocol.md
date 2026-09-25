# Lexicon-free suspects offline: protocol

Issue [#9652](https://github.com/OpenAgentsInc/openagents/issues/9652).
Written on 2026-09-25, after the code of the lexicon-free mode and before
any defect-site label was computed or any Jev answer was asked on a task
workspace. Nothing below changes after that.

## What is compared

The `rationale` step asks Jev one question per comment, the v13 question
in `questions/departure-rationale.json`, unchanged. Two arms decide which
comments Jev sees:

- **`keywords`**, the default: v13's gate, `accept::rationale_choices`,
  comment lines that contain a phrase from `GENERAL_MARKS` or
  `RATIONALE_MARKS`, 24 at most. Five of the 24 phrases come from
  `embedding-drift-monitor`'s own comments, so this arm is in-sample on that
  task and a labeled comparison arm everywhere.
- **`lexicon-free`**: `departures::comments`, every comment and docstring
  attached to a function, to its enclosing class, or to the module, in the
  same source files the keyword scan reads (`accept::source_files`, 60 at
  most), with no phrase filter. Consecutive full-line comments form one
  candidate. The bound is **96 candidates per workspace**, taken one per
  file in turn when a workspace has more. Jev reads them 24 to a request.

Both arms run through the `evidence.departures` component with
`sources: ["rationale"]` and `comments` set to the arm, on `jev-1.13.0`,
live once, with every answer saved and replayed from then on.

## Population and split, frozen now

A task enters the cohort when both hold:

1. its Dockerfile puts at least one source file the scan reads (`py`, `js`,
   `ts`, `go`, `rs`, `rb`, `java`, `c`, `cc`, `cpp`, `h`, `sh`) in the
   agent's working directory, and
2. its reference solution changes at least one of those files in place,
   and the change is recorded in the solution itself: a file it copies over
   the workspace's, a patch, or literal text a script writes or replaces.
   A solution that only adds new files, computes its edits at run time, or
   edits files outside the working directory doesn't qualify.

By that rule the cohort is 15 tasks. `embedding-drift-monitor` is
**in-sample** and reported alone. The other 14 are the **off-sample** set,
and every result that decides anything comes from them:
`batched-eval-parity`, `biped-contact-dynamics`, `bun-sourcemap-leak`,
`cargo-flight-dispatch`, `fp8-rmsnorm-gemm`, `jax-speedrun-gpu`,
`live-database-cutover`, `mvcc-lsm-compaction`, `nextjs-performance`,
`payments-pipeline-fix`, `react-lead-form`, `risk-scorer-replay`,
`session-window-debug`, and `wal-recovery-ordering`.

Nothing is fitted, so there's no fit half: the question, the threshold,
and the bound are fixed here. A scored task is one whose fix changes at
least one site (below) and leaves at least half of the functions in the
scanned files unchanged; a fix that rewrites more than half isn't a
localized defect, and such a task is listed with its numbers but not
scored.

## Labels, frozen now

`build.py` computes the labels mechanically, from the untouched workspace
and the workspace with the reference fix applied; no person picks a label.

- **A defect site** is the innermost function, method, or class-level
  statement group whose code the fix changes: a line replaced or deleted
  inside it, or a line inserted strictly inside it. A changed line that
  is blank or only a comment, on both sides, doesn't count. A changed
  statement outside every function is a module-level site at that line.
- **A row names a site** by where it sits. A comment directly above a
  definition (decorators may sit between) belongs to that definition; any
  other comment belongs to the innermost function or class that holds its
  line. A row that belongs to a function names the sites inside it; a row
  that belongs to a class names the sites of the methods it holds. A row
  outside every function names a module-level site only when the site is
  on its line or the next code line. A module docstring names nothing.

Python spans come from Python's `ast`; brace-language spans from matching
braces.

## Threshold and metrics, frozen now

A row is **listed** when Jev's probability is at least 0.5 (v13's
`SUSPECT_P`), most likely first, 8 at most (v13's list).

Per arm, over the scored off-sample tasks:

- **Hit at k**, k = 3 and 8: the share of tasks where at least one of the
  top k listed rows names a site.
- **False positives per task**: listed rows (top 8) that name no site,
  averaged over tasks.
- **Site recall at 8**, secondary: sites named by a top-8 listed row, over
  all sites.
- **Jev cost**: dollars per task, at `jev-1.13.0`'s input price, and in
  total.

Shares carry 95% Wilson intervals over tasks. Each difference
(`lexicon-free` minus `keywords`) carries a 95% percentile interval from a
task-grouped bootstrap: 10,000 resamples of tasks with replacement, seed
9652.

## Decision rule, frozen now

The lexicon-free mode is **at least as good off-sample** when its hit at
3 and hit at 8 are each at least the keyword arm's (point estimates) and
its false positives per task are no more than the keyword arm's plus one.
If it is, the write-up proposes it as the default of a future pinned
policy; it doesn't enact that. Either way the keyword gate stays the
default, and the new mode stays off in every existing manifest.

## Budget

Jev spend stays under $0.10 in total, recorded. No Luna session and no
Terminal-Bench trial.
