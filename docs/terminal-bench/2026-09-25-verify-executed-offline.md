# `verify.executed` offline: rerun the task's commands, reject a regression

2026-09-25. Issue
[#9636](https://github.com/OpenAgentsInc/openagents/issues/9636), part of
Microluna v18 ([#9640](https://github.com/OpenAgentsInc/openagents/issues/9640)),
change 5 of the [v18 design](../coder/design/microluna-v18.md).

## Result

**Built and wired behind a switch, off by default. On every retained
workspace this machine can still restore, the rule never fired.**

- 69 workspaces on 10 tasks ran the rule; 59 have an official reward (49
  failures, 10 passes).
- The rule rejected **0** of them. Precision is undefined (no rejection).
  Recall of "a baseline command regressed" as a fail signal is **0 of 49
  (95% Wilson interval 0–7%)** pooled: 0 of 31 (0–11%) on the
  `accept offline` set and 0 of 18 (0–18%) on the fresh #9584 trials.
- It rejected no passing workspace either: 0 of 10 passes.
- **Keep-best decisions changed: 0 of 20** retained lean loops.
- All 8 fresh #9584 trials are now measured (26 workspaces). The
  `accept offline` set still covers only 43 workspaces on 2 of its 23
  tasks, including all 12 retained v13 lean-loop candidates.
  [What wasn't measured](#what-wasnt-measured) says why.

The expected result was high precision and low recall. The low recall
holds; precision couldn't be measured, because nothing regressed. The cost
is small where the rule has something to run: a median of 2.5 s per
candidate on `embedding-drift-monitor`, 0.5 s on `session-window-debug`,
and 7 s on `vpp-loss-divergence`, against 7 to 19 s for the frozen score.
The exception is `pretrain-shard-corruption`, at 47 to 48 s, spent almost
entirely waiting out one command's 45 s network timeout. On 5 of the 8
fresh tasks the rule had nothing to run at all. The rule is cheap and, on
this data, harmless, but no measurement here shows that it improves an
outcome.

This made no Luna session, no Jev request, and no Terminal-Bench trial.

## What was built

`verify.executed` is in
[`crates/coder-one/src/checks/contract/executed.rs`](../../crates/coder-one/src/checks/contract/executed.rs),
on #9628's runner and extractor. It's wired into the lean loop
(`crates/coder-one/src/micro/lean.rs`) behind
`executor.microluna.lean.executed` (`command_sec`, default 60;
`budget_sec`, default 300), which needs `keep_best` and is absent from
every existing manifest.

- **What runs.** After every session, in a scratch copy of the workspace:
  the baseline commands `evidence.baseline` (#9633) ran to their own exit
  (`crate::baseline::Baseline::commands`); every command the instruction
  names, from #9628's extractor with no model, less those stated to fail
  and those that install something; and a compile or import of the
  package (`python3 -m compileall` over the top-level packages and scripts
  and `python3 -c 'import <package>'` for each top-level package,
  `cargo check --offline`, or `go build ./...`).
- **The untouched outcome.** A baseline command's outcome on the
  untouched workspace is read from its `stage: "baseline"` record. Every
  other command runs once on the untouched copy before session 1 and is
  recorded the same way.
- **The rule, by code.** A command that exited 0 on the untouched
  workspace and doesn't on the candidate (a non-zero exit or a signal) is
  `regressed`. One that didn't exit 0 on the untouched workspace either is
  `not_a_regression`. A timeout, or a command the host couldn't run, is
  `unknown`. A candidate with any `regressed` command is rejected: it
  can't be kept whatever it scores, the previous kept candidate stays
  kept, the next session's state names the command and its output tail,
  and the loop finishes on the kept candidate. Jev isn't consulted.
- **Records.** Every run is one line of
  `artifacts/lean-<n>/executed-commands.jsonl`, beside
  `evidence.baseline`'s, in the shape
  [`docs/gym/run-card.md`](../gym/run-card.md) documents
  (`openagents.coder-one.executed-command.v1`, `stage: "after_session"`,
  `session`, `candidate` such as `lean-1/session-2`, `verdict`, `rule`).
  `gym runs characterize` reads them: on a copy of the retained v13 trial
  `embedding-drift-monitor__6zRjd9n` with the offline run's records for
  its two sessions, the card reports "Host-executed commands: 9" and
  pairs the records by session ("Executed checks before and after: 3 and
  3 records").
- **Where commands run.** Inside a writing boundary on the copy, or, in a
  task container, which can't enforce one, unconfined in the copy (a new
  `Contained` host), as the frozen score runs there. A command that names
  the workspace's path is moved to the copy's.
- **Fixtures** (`crates/coder-one/fixtures/executed/`): a synthetic Python
  task and five candidates, run for real: no regression (kept), a crash
  (rejected), an import failure (rejected), a command that failed on the
  untouched workspace too (`not_a_regression`, kept), and a timeout
  (`unknown`, kept). A lean-loop test shows a candidate that scores as
  well as the kept one but regresses `evidence.baseline`'s command is
  rejected, and the kept candidate is restored.

## Method

`coder-one checks contract executed TASK...` reuses #9628's offline
harness. For one task, it copies the image's working directory out of a
fresh container to find `evidence.baseline`'s entry points with
`checks::contract::entry::find`, plans the named and compile commands
inside the container, and runs every command once there as the untouched
outcome. It then restores each retained workspace that `accept offline`
reads into its own networkless container, runs the same commands (60 s
each, 300 s together), and judges each against its untouched outcome.
[`measure.py`](../../bench/terminal-bench/experiments/2026-09-25-verify-executed/measure.py)
joins the records with official rewards and replays each retained lean
loop's `selection.json` through lean.rs's keep and restore rules, with and
without the rejections.

The rule has no threshold and nothing was tuned, so no split is needed.
The records are in
[`bench/terminal-bench/experiments/2026-09-25-verify-executed/records/`](../../bench/terminal-bench/experiments/2026-09-25-verify-executed/records/),
with `summary.json`.

## Results

### The commands

| Task | Commands (kind) | Untouched outcome |
| --- | --- | --- |
| `embedding-drift-monitor` | `python3 -m drift_monitor` on the four data files (module) | exit 1, after `RuntimeWarning: invalid value encountered in divide` at `normalize.py:18` |
| | `python3 -m compileall -q 'drift_monitor'` (compile) | exit 0 |
| | `python3 -c 'import drift_monitor'` (compile) | exit 0 |
| `session-window-debug` | `python3 -m compileall -q 'app'` (compile) | exit 0 |
| | `python3 -c 'import app'` (compile) | exit 0 |
| `pretrain-shard-corruption` | `bash /app/run_pretrain.sh` (named) | exit 1 after 45 s: `torch.distributed` rendezvous times out resolving the container's own host name |
| | `python3 train_pretrain.py` (script) | exit 1: `RANK` isn't set outside `torchrun` |
| | `python3 -m compileall -q 'train_pretrain.py' 'validate_data.py'` (compile) | exit 0 |
| `vpp-loss-divergence` | `python3 pretrain.py` (script) | exit 0, 8 s |
| | `python3 -m compileall -q 'config.py' 'pretrain.py'` (compile) | exit 0 |
| `shadow-relay`, `distributed-dedup`, `formal-crypto`, `math-eval-grader`, `freecad-impeller`, `freecad-spring-clip` | none: no entry point that `evidence.baseline` finds, no named command, and no Python, Cargo, or Go package in the working directory | — |

`distributed-dedup` is an sbt project, which the compile step doesn't
cover.

### By task

| Set | Task | Workspaces | Graded | Failures | Passes | Rejected | Recall, 95% Wilson |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `accept offline` | `embedding-drift-monitor` | 22 | 21 | 11 | 10 | 0 | 0 of 11 (0–26%) |
| `accept offline` | `session-window-debug` | 21 | 20 | 20 | 0 | 0 | 0 of 20 (0–16%) |
| `accept offline` | all | 43 | 41 | 31 | 10 | 0 | 0 of 31 (0–11%) |
| #9584 fresh | `distributed-dedup` | 3 | 3 | 3 | 0 | 0 | 0 of 3 (0–56%) |
| #9584 fresh | `formal-crypto` | 4 | 4 | 4 | 0 | 0 | 0 of 4 (0–49%) |
| #9584 fresh | `freecad-impeller` | 3 | 0 | — | — | 0 | no official reward |
| #9584 fresh | `freecad-spring-clip` | 4 | 0 | — | — | 0 | no official reward |
| #9584 fresh | `math-eval-grader` | 3 | 2 | 2 | 0 | 0 | 0 of 2 (0–66%) |
| #9584 fresh | `pretrain-shard-corruption` | 3 | 3 | 3 | 0 | 0 | 0 of 3 (0–56%) |
| #9584 fresh | `shadow-relay` | 3 | 3 | 3 | 0 | 0 | 0 of 3 (0–56%) |
| #9584 fresh | `vpp-loss-divergence` | 3 | 3 | 3 | 0 | 0 | 0 of 3 (0–56%) |
| #9584 fresh | all | 26 | 18 | 18 | 0 | 0 | 0 of 18 (0–18%) |
| Pooled | | 69 | 59 | 49 | 10 | 0 | 0 of 49 (0–7%) |

A fresh workspace is graded when it's the final workspace or its files
are the submitted workspace's, since #9584's retained traces carry the
official label only for the submission. Both FreeCAD trials ended in a
`RuntimeError` and have no official reward. Every official fresh label is
0, so no fresh pass exists to check for a false rejection. Verdicts over
all 123 runs: 116 `ok`, 7 `not_a_regression`, 0 `regressed`, 0 `unknown`.
The six new `not_a_regression` verdicts are the two
`pretrain-shard-corruption` commands that failed on the untouched
workspace, on each of its three workspaces.

The 12 retained v13 lean-loop candidates are 12 of the
`accept offline` workspaces (6 per task); 10 have a reward, all
`ok` on every command. The other `accept offline` workspaces are the
Microluna finals and the `candidate-evidence-9607` candidates of the same
tasks and the v12 reconstruction.

### Keep-best decisions

20 retained lean loops (6 v13, 6 `candidate-evidence-9607`, and the 8
fresh trials) were replayed through the keep and restore rules. Without
the rule, the replay reproduces the recorded selection in 18 of 20. The
other two, `shadow-relay` and `vpp-loss-divergence`, record session 2
where the replay picks session 1; in both, the two sessions' files are
identical and the retained record names the later match. With the rule,
**no decision changed**, since no candidate was rejected.

### Why it never fired

- **The one command that matters wasn't clean on the untouched
  workspace.** On `embedding-drift-monitor`, the task's program exits 1
  before any change, so a candidate that crashes it can only be
  `not_a_regression`. This is the task where Luna reran the CLI in every
  session. On `pretrain-shard-corruption`, the training launcher fails on
  the untouched workspace too, but for a reason the offline harness
  causes: the workspace runs in a networkless container, where
  `torch.distributed` can't resolve the container's host name. Whether
  the launcher exits 0 on the untouched workspace in a task container
  with a network, where the lean loop would run it, wasn't measured.
- **Serious candidates compile and run.** Every graded candidate on the
  tasks with commands compiles and imports, and on `vpp-loss-divergence`
  every candidate's `pretrain.py` still exits 0; the failures are wrong
  results, not crashes.
- **Most tasks give it nothing to run.** 6 of the 10 tasks have no entry
  point that `evidence.baseline` finds, name no command, and have no
  package the compile step covers.

## What wasn't measured

- **Most of the `accept offline` set.** #9628 measured 163 workspaces on
  23 tasks. On this machine today, the Coder One snapshots under
  `~/.openagents/terminal-bench/replay-jobs` are empty directories, and
  the Microluna manual trials (v3 to v17) are no longer on disk. What
  remains restorable is the retained traces under
  `bench/terminal-bench/traces/` for `embedding-drift-monitor` and
  `session-window-debug`.
- **Official rewards for 8 of the 26 fresh workspaces.** Both FreeCAD
  trials have no official reward, and one `math-eval-grader` session's
  files differ from the submission's. The rule ran on all eight
  workspaces and rejected none.
- **Mini-tasks against `microluna-v15`.** Not run. With no rejection on
  any retained workspace, a matched run could measure only the rule's
  cost, which the offline records already give.

Every fresh #9584 trial is measured. Each task's image was built as
`accept-env/<task>:latest` from its public `environment/`, one at a time,
measured with `coder-one checks contract executed TASK --out DIR --jobs
DIR --image IMAGE` over a tree where each trial's episode lists its
lean-loop sessions (the post-executor snapshot hidden), and removed with
its build cache before the next build. The two FreeCAD tasks share one
image, since their environments are identical. `measure.py` then joined
all ten tasks' records.

## Supervise on this host

Every command here ran with `SUPERVISE_MEMORY_MAX=off`: on this host,
`supervise`'s default memory cap makes every spawn fail with `EINVAL`, in
this component's tests and in the existing lean-loop tests alike.
