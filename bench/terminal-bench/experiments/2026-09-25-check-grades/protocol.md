# Check-line grades offline: protocol

Issue [#9635](https://github.com/OpenAgentsInc/openagents/issues/9635).
Written on 2026-09-25, after `accept.grade`'s code and before any Jev
answer on a retained score script.

## What is measured

`accept.grade` splits a frozen lean-loop score script into check lines
by code (`crates/coder-one/src/grade/mod.rs`) and asks Jev, for each
line, the three Nouls of `questions/expectation-support.json`: does the
expected result follow from the task's words, from the task's baseline
behavior, or from the standard definition of a named method? A line
whose best probability is at or above the threshold is graded `follows`;
otherwise `advisory`. A script that doesn't split is one advisory unit.

The baseline behavior (`evidence.baseline`, #9633) hasn't landed, so
every script here is graded on the task's words and the standard
definition only. The baseline question isn't asked.

Keep-best's graded key ranks a workspace by the fraction of `follows`
lines it passes first and by the full score second.

## Population

What `coder-one accept grade` finds under `bench/terminal-bench/traces`:
every Microluna trial, with #9584's sealed tasks and the
truth-confirmation and truth-control jobs excluded.

- **Scripts:** the frozen `artifacts/lean-*/evaluator/score.sh` of every
  trial that kept one: 12, six on `embedding-drift-monitor` and six on
  `session-window-debug` (the evidence-v1 and v13-retained arms). The v12
  controls didn't retain their evaluator.
- **Workspaces:** each trial's final workspace (`produced/app`) and each
  retained lean-loop candidate whose reward is known from a
  `candidate-grades` record matching its files or from being the
  submitted candidate. Workspaces with the same Python source are one;
  the first with a known reward stands for the rest.

The 48 graded Microluna workspaces of
`docs/terminal-bench/2026-09-24-acceptance-first.md` are on four tasks,
but the trial directories of the other three (`sound-change-cascade`,
`interleaved-vigenere`, `fin-saccr-rwa`) aren't on this host, and no
trial of theirs kept its frozen score script. They can't be measured
here, and the write-up says so.

Each script runs on every workspace of its own task in the task's image,
built from the task's `environment/Dockerfile`, with the workspace at
`/app` and no network. Each script's recorded host scores on its own
trial's candidates check that the rebuilt image reproduces the trial's.

## Threshold, frozen now

The held-out rules forbid tuning on `embedding-drift-monitor` and on any
task in `docs/terminal-bench/capability-gaps.md`, which includes
`session-window-debug`. Every retained script is on one of those two
tasks, so no permitted task is left to choose a threshold on. The
threshold is fixed at 0.5, the v13 suspect listing's value, before any
answer. A sweep over thresholds is shown for reading only; it chooses
nothing.

Before this protocol, a run with Jev off showed each script's raw scores
on the workspaces beside their rewards. Those are the raw-score floor,
not an input to the grade.

## Scoring, frozen now

Per script, over its task's deduplicated workspaces with a known reward:

- **Raw separation:** the share of (pass, fail) pairs the raw score
  orders correctly, ties counting half (AUC). It separates when the AUC
  is 1.
- **Graded separation:** the same with the graded key.
- A task with no pass or no failure has no separation to measure; it's
  reported as such.

Per trial, replaying its own candidates in session order with the
arm's tie rule (`protect_candidates` keeps the earlier of a tie): whether
the graded key keeps a different candidate than the raw score did.

## Admission, frozen now

`accept.grade` is admitted when, on every task with both passing and
failing workspaces:

1. the graded key separates for more scripts than the raw score does,
   and
2. no script's graded AUC is below its raw AUC.

Otherwise it's not admitted, and the manifest switch stays off. Jev runs
live, answers are saved with the records, and Jev's spend stays under
$0.05.
