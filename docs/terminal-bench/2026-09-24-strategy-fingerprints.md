# Strategy fingerprints: what Fable's winners do that Luna and Coder One don't

2026-09-24, for issue
[#9586](https://github.com/OpenAgentsInc/openagents/issues/9586), part of the
[Luna pivot](../coder/design/luna-pivot.md). This document compares
trajectories step by step rather than by outcome alone. Each difference that
repeats across tasks is a *candidate move*: a hypothesis for a System One
algorithm that code and Jev could give GPT-6 Luna as structure. None of them
is a measured improvement yet.

The data is in
[`2026-09-24-strategy-fingerprints.json`](2026-09-24-strategy-fingerprints.json).

## Summary

- **Scope.** 505 trajectories on the 14 tasks of the
  [Luna baseline subset](2026-09-24-luna-tb4-baseline.md): 350 public Fable 5.1
  attempts (243 passed), 28 Luna attempts (20 graded, none passed so far,
  8 still running in #9583), 85 Coder One attempts, and 42 Claude Code
  attempts. The same code also fingerprinted all 419 readable Coder One runs
  on this machine, across 65 tasks.
- **Phases.** Rules over tool names and command text placed 12,362 of the
  18,781 subset steps (66%). Jev placed the other 6,419 with one Choice and
  three Nouls per step.
- **Jev cost.** 8,781 step answers, 19,192,048 input tokens, **$0.8061** at
  `jev-1.13.0`'s $0.042 per million input tokens. Every answer is cached by
  step digest, so rerunning the analysis costs nothing until new steps arrive.
- **The strongest candidate move is to read longer before the first edit.**
  Fable's winners make their first edit later than Coder One's executor in
  10 of 11 tasks, with a typical 5.7 minutes against 2.1, and later than
  Luna in 7 of 10 tasks, with 5.1 minutes against 2.3. Within Fable, winners
  also start editing later than losers in 7 of 11 tasks.
- **Luna stops after fewer rounds.** Fable's winners edit in more separate
  rounds than Luna in 8 of 11 tasks, with 3 rounds against 2, and take more
  steps in 8 of 11, with 21 against 14. Across all agents, winners edit in
  more rounds than losers in 11 of 14 tasks.
- **Fable's winners test more per edit than its losers** in 8 of 11 tasks.
- **Some expected moves didn't separate winners from losers:** retries,
  written plans, verification at the end of the run, and Jev's
  "checks an assumption" and "uses earlier evidence" judgments.

## How it works

Every trajectory loads through the same reader head-to-head replay uses
([head to head](../gym/head-to-head.md)): Harbor's ATIF for Claude Code,
Codex, and the public Fable attempts; Coder One's episode log with its Claude
Code and Codex streams; and Microluna's per-session ATIF logs. Each tool call
becomes one numbered step.

### Phases

`crates/gym/src/runs_phases.rs` places each step in one of eight phases:
orient, read, plan, edit, build, test, verify, and finish.

- **Rules first.** Tool names decide most non-shell calls: `Read` is read,
  `Edit` and `apply_patch` are edits, and `TodoWrite` is a plan. A shell
  command is split into its pieces, heredoc bodies are skipped, and each
  piece is classified by its program: `pytest`, `cargo test`, and
  `make test` are tests; compilers and installers are builds; `cat`, `sed
  -n`, and `grep` are reads; `ls` and `find` are orienting; a write to the
  task's files is an edit. A script the agent wrote under `/tmp`, or one
  named like a test or a reproduction, is a test. After the first edit,
  reading an edited file or running `git diff` is verification. Rules are
  versioned as `runs-phases-rules-v2`.
- **Jev for the rest.** A command that runs a program the rules can't name,
  such as `python3 solve.py` or an inline `python3 - <<EOF`, is unplaced.
  For each unplaced step, one Jev request carries the task, the step's
  command and output, and the three steps before it with their rule phases.
  It asks a Choice over the eight phases and three Nouls: does the step
  check an assumption, does it act on evidence an earlier step revealed, and
  is it a retry of a failed step. Answers are stored in
  `~/.openagents/gym/fingerprints/step-answers.jsonl`, keyed by the digest
  of the state and the question set `runs-phases-v1`.

A spot check of 30 Jev-placed steps found sensible placements: an inline
Python script that rewrites a source file is an edit, `python3 -m
drift_monitor data/…` on the task's data is a test, and `vncdo … capture` on
a remote desktop is orienting. Several screen-automation steps had low
confidence, below 0.4, which is fair for steps that both look and act.

### Fingerprints

`crates/gym/src/runs_fingerprint.rs` summarizes one trajectory:

- Steps, and the step and elapsed time of the first edit.
- Tests, and tests per edit.
- Verification (test and verify steps) by quarter of the run, before the
  first edit, and after the last edit. Coder One's controller checks are
  counted apart, as `controller_checks`, because they're the harness's
  verification, not the executor's.
- How many separate rounds of edits there were, and the share of edit rounds
  followed by a test or verification before the next round.
- Retries: a command identical to an earlier failed one, plus the steps Jev
  judged to be retries.
- Files touched in the task's workspace.
- Whether it ran a test, script, or program before its first edit, and
  whether it ran the task's own example: a command or script the task names
  in backticks.
- The phase sequence, compressed into runs, such as `R4 T1 E2 T2`.

### Moves

`crates/gym/src/runs_moves.rs` compares two groups of fingerprints within
each task and measures each feature with Cliff's delta: the chance that a
trajectory from the first group has the larger value, minus the chance that
it has the smaller one. A difference is a candidate move when it points the
same way in at least 3 tasks and in two-thirds of the tasks with data on
both sides, and its mean delta across tasks is at least 0.20. "Typical"
values below are the median over tasks of each group's per-task median.

| Comparison | Tasks with both sides | First group | Second group |
| --- | ---: | ---: | ---: |
| Fable winners against Luna | 11 | 180 | 20 |
| Fable winners against Coder One | 11 | 180 | 81 |
| Fable winners against Fable losers | 11 | 168 | 106 |
| All winners against all losers | 14 | 314 | 178 |

Luna has no passes yet, so it appears only as a comparison group. Its 20
graded attempts are 13 Codex CLI runs of `gpt-6-luna` (`codex-gpt-6-luna`),
6 Coder One runs with `tunable-luna-pack-solo` (`luna-jev`), and 1
`microluna-v1` run.

## Example: Fable against Luna on `mvcc-lsm-compaction`

A Fable 5.1 xhigh attempt that passed:

```text
Fingerprint · 36f015a8-ef13-41da-b0e7-970064be6026
mvcc-lsm-compaction · Fable 5.1 xhigh · passed · $4.46
15 steps over 14m 15s · first edit at step 6 (33% of steps), 4m 37s in
Tests: 7 (0.47 a step, 1.8 an edit) · verification by quarter 0/2/2/3, 3 after the last edit
Ran something before the first edit: yes, step 5
Sequence: R4 T1 E2 T2 E1 T1 E1 T3

    1  00:00:04  read     rule  Bash cat /app/crash_report.txt; …; find /app -type f …
    2  00:00:10  read     rule  Bash for f in include/mvcc_lsm/types.h include/mvcc_lsm/version.h …
    3  00:00:15  read     rule  Bash for f in src/snapshot_context.cc src/version_store.cc …
    4  00:00:21  read     rule  Bash … cat -n /app/tests/regression_test.cc …
    5  00:03:33  test     rule  Bash make test 2>&1 | tail -n 5; … make repro 2>&1 | tail -n 5 …
    6  00:04:42  edit     rule  Bash cat > /app/src/snapshot_context.cc <<'EOF'
    7  00:05:01  edit     rule  Bash cat > /app/src/flush_builder.cc <<'EOF'
    8  00:05:05  test     rule  Bash cat > /app/tests/regression_test.cc <<'EOF'
    9  00:05:58  test     rule  Bash make clean >/dev/null && make test …; make repro …
   10  00:10:20  edit     rule  Bash cat > /app/include/mvcc_lsm/snapshot_context.h <<'EOF'
   …
   13  00:11:53  test     rule  Bash mkdir -p /tmp/fuzz && cat > /tmp/fuzz/fuzz.cc <<'EOF'
   14  00:13:11  test     rule  Bash cd /app; echo "=== fuzz vs original snapshot_context (should fail)"; …
```

A Codex CLI attempt of GPT-6 Luna on the same task, which failed:

```text
Fingerprint · tb4--codex-gpt-6-luna--mvcc-lsm-compaction--luna-tb4-9583-r1/mvcc-lsm-compaction__6kUsHLf
mvcc-lsm-compaction · Codex · GPT-6 Luna · failed · $0.005
10 steps over 1m 27s · first edit at step 4 (30% of steps), 32s in
Tests: 4 (0.40 a step, 4.0 an edit) · verification by quarter 0/1/3/2, 6 after the last edit
Ran something before the first edit: no
Sequence: R3 E1 T4 V2

    1  00:00:09  read     rule  exec_command pwd && rg --files -g 'AGENTS.md' -g 'Makefile' …
    2  00:00:13  read     rule  exec_command cat crash_report.txt && sed -n '1,260p' include/mvcc_lsm/version_store.h …
    3  00:00:16  read     rule  exec_command cat src/lsm_db.cc && cat src/flush_builder.cc && cat src/snapshot_context.cc …
    4  00:00:41  edit     rule  apply_patch /app/src/snapshot_context.cc
    5  00:00:59  test     rule  apply_patch /app/tests/regression_test.cc
    6  00:01:04  test     rule  exec_command make -C /app test
    7  00:01:04  test     rule  exec_command make -C /app repro
    8  00:01:08  test     jev   exec const results = await Promise.allSettled([
    9  00:01:17  verify   rule  exec_command git diff --check && git diff -- src/snapshot_context.cc …
```

Both read the same files. Fable then took three minutes before running the
task's own `make test` and `make repro`, before changing anything; changed
three source files and the regression test in three rounds, testing after
each; and finished by fuzzing the fix against the original code. Luna
patched one file 32 seconds in, ran the tests and the reproduction once,
reviewed its diff, and stopped. All
13 of Fable's winners on this task run a test before their first edit
(Cliff's delta +1.00 against Luna on this task).

Print any fingerprint with `gym runs fingerprint RUN`, where RUN is a
local `job/trial` or a public Fable trial ID.

## Candidate moves, ranked

The ranking weighs how many comparisons a move shows up in, not only its
score in one. Each entry states the proposed algorithm for Luna in the
pivot's terms.

### 1. Read longer before the first edit

| Comparison | Tasks agreeing | Mean delta | Typical minutes before the first edit |
| --- | ---: | ---: | --- |
| Fable winners against Coder One | 10 of 11 | +0.53 | 5.71 against 2.07 |
| Fable winners against Luna | 7 of 10 | +0.33 | 5.08 against 2.32 |
| Fable winners against Fable losers | 7 of 11 | +0.20 | 5.71 against 2.32 |

Fable's winners also spend a larger share of their steps before the first
edit than Coder One's executor (9 of 11 tasks, +0.47, 0.29 against 0.17),
more of their steps reading (9 of 11, +0.44, 0.27 against 0.21), and fewer
orienting with `ls` and `find` (8 of 11, −0.25).

Per task, Fable winners against Luna, in minutes (medians):

| Task | Fable winners | Median | Luna | Median | Delta |
| --- | ---: | ---: | ---: | ---: | ---: |
| `cad-model` | 13 | 10.91 | 1 | 4.03 | +0.54 |
| `embedding-drift-monitor` | 25 | 3.64 | 2 | 1.02 | +0.64 |
| `fin-saccr-rwa` | 22 | 4.45 | 1 | 2.67 | +0.36 |
| `heat-pump-warranty` | 14 | 3.42 | 1 | 9.44 | −0.57 |
| `ks-solver-cpp` | 9 | 6.40 | 2 | 1.97 | +0.67 |
| `legacy-utility-triage` | 12 | 0.52 | 1 | 3.16 | −0.67 |
| `mvcc-lsm-compaction` | 13 | 5.71 | 2 | 0.39 | +1.00 |
| `nextjs-performance` | 15 | 5.83 | 2 | 1.04 | +0.93 |
| `sound-change-cascade` | 25 | 1.84 | 1 | 15.42 | −0.60 |
| `wal-recovery-ordering` | 1 | 7.20 | 2 | 1.12 | +1.00 |

Citations (the first edit in each run):

- `mvcc-lsm-compaction`: Fable `951056e1-ba9f-4d5f-8610-fdb93b5fc759` step
  7, 11.9 minutes in, against `tb4--luna-jev--mvcc-lsm-compaction--luna-tb4-9583-r1/mvcc-lsm-compaction__J3pVbur`
  step 2, 0.2 minutes in.
- `nextjs-performance`: Fable `92c9ad47-88d3-46e2-a2e7-794324e561ce` step
  24, 21.9 minutes in, against `tb4--luna-jev--nextjs-performance--luna-tb4-9583-r1/nextjs-performance__ZEDjfNL`
  step 5, 0.9 minutes in.
- `ks-solver-cpp`: Fable `2fc22c0b-2fb9-478c-8ea2-062e4c1e6a55` step 6,
  16.7 minutes in, against `tb4--luna-jev--ks-solver-cpp--luna-tb4-9583-r1/ks-solver-cpp__J3takNd`
  step 1, at the start.
- `coq-block-bound`, against Coder One: Fable
  `237e4516-0b73-4b08-89f3-d62200765e4d` step 4, 62 minutes in, against
  `tb4--coder-one-tunable-v6--coq-block-bound/coq-block-bound__Mu8ygpJ`
  step 2.

**Algorithm for Luna: a read-first session.** Microluna's first session on a
requirement gets read and run tools only; code withholds `apply_patch` and
`write_file` until a Jev Noul judges that the steps so far have read the
code and data the requirement depends on. This is the "evidence for every
requirement" algorithm of the pivot, enforced in the loop as well as in the
briefing.

**Caveat.** Coder One's executor and `luna-jev` start from a briefing that
already carries evidence, so some of their reading happened before the
first step. Luna in Codex CLI has no briefing and still edits early.

### 2. Edit in more rounds, and don't stop after one

| Comparison | Feature | Tasks agreeing | Mean delta | Typical |
| --- | --- | ---: | ---: | --- |
| Fable winners against Luna | Edit rounds | 8 of 11 | +0.40 | 3 against 2 |
| Fable winners against Luna | Steps | 8 of 11 | +0.37 | 21 against 14 |
| All winners against all losers | Edit rounds | 11 of 14 | +0.25 | 3 against 2.25 |
| Fable winners against Fable losers | Steps | 8 of 11 | +0.22 | 27 against 28.5 |

The typical step counts within Fable are close; the per-task delta is
positive because losers include both very short runs and very long ones.

Citations:

- `ks-solver-cpp`: Fable `2fc22c0b-2fb9-478c-8ea2-062e4c1e6a55`, 11 edit
  rounds over 36 steps, against `tb4--luna-jev--ks-solver-cpp--luna-tb4-9583-r1/ks-solver-cpp__J3takNd`,
  2 rounds over 8 steps.
- `nextjs-performance`: Fable `89f3d00b-a0d5-4c1b-bf68-adca5dbc999e`, 9
  rounds from step 11, against `tb4--codex-gpt-6-luna--nextjs-performance--luna-tb4-9583-r1/nextjs-performance__QLVwaid`,
  3 rounds from step 7.
- `sound-change-cascade`: Fable `39f73a68-eed0-425c-b124-32128c708e66`, 10
  rounds, against `tb4--luna-jev--sound-change-cascade--luna-tb4-9583-r1/sound-change-cascade__Wzgx9tp`,
  2 rounds.

**Algorithm for Luna: done detection with evidence.** When a Luna session
says it's done, code checks each requirement with a truthful check, and Jev
judges whether the session's own evidence supports the claim. An unmet
requirement starts another short session with only that requirement and
the check's output. The one `microluna-v1` run in the data shows why:
on `coq-block-bound`
(`tb4--microluna-v1--coq-block-bound--9585-r1/coq-block-bound__3wJAvgQ`),
six sessions read `Main.v` and finished as "blocked" or "done" without an
edit (steps 2, 5, 9, 13, 16, and 19).

### 3. Test after each edit

| Comparison | Tasks agreeing | Mean delta | Typical tests per edit |
| --- | ---: | ---: | --- |
| Fable winners against Fable losers | 8 of 11 | +0.20 | 1.00 against 1.00 |
| Fable winners against Luna | 6 of 10 | +0.27 | 0.90 against 0.75 |

The typical values tie because both groups often run one test per edit; the
difference is in the tails. On `cad-model`, a Fable winner
(`77574d31-e75e-43bd-9fd3-11eedc0dd697`, first test at step 14) ran 9 tests
per edit, and the Luna attempt
(`tb4--codex-gpt-6-luna--cad-model--luna-tb4-9583-r1/cad-model__ARS62UT`)
ran none. On `mvcc-lsm-compaction`, Fable winners ran a median of 4 tests
per edit against 1 for its losers.

**Algorithm for Luna: next-step choice after an edit.** After an edit, code
proposes running the task's tests or the session's own probe, and the
typed next-step choice can't pick another edit until a test result is in
the context.

### 4. Reproduce before editing (weaker)

Fable's winners test or verify before their first edit in 60% of attempts,
against 45% of Fable's losers and 40% of Luna's and Coder One's attempts.
Per task, the difference is inconsistent: 7 of 11 tasks against Luna, with a
mean delta of +0.13, below the candidate threshold. It is decisive on some
tasks: +1.00 on `mvcc-lsm-compaction` and +0.93 on `nextjs-performance`
against Luna. Keep it as a task-family move rather than a default: a
"reproduce first" session that writes and runs a failing probe before any
edit, for bug-fix tasks that ship a crash report or a reproduction target.

### 5. Keep probes and helpers as files (low confidence)

Fable's winners touch more files than Luna in all 11 tasks (+0.84, 4 files
against 1). Part of this is how the tools count: Fable writes its solution,
its tests, and its helper scripts with heredocs into the task's workspace,
while Luna often patches one file or produces its output from a script
under `/tmp`, which the rules count as scratch. The honest reading is
narrower: Fable keeps its probes as files it can rerun. For Luna, persist
each session's reproduction or probe script so the next session reruns it
rather than rewriting it.

## What didn't separate winners from losers

- **Retries.** Retry rates differ little: −0.14 against Luna in 5 of 11
  tasks, and +0.04 within Fable. Luna doesn't lose by looping.
- **Verification at the end.** Most trajectories in every group test or
  verify after their last edit, and Fable's winners do so slightly less
  often than Luna (−0.13) and Coder One (−0.14). A final check isn't what
  Luna lacks.
- **Written plans.** Fable used no to-do tool in these runs, winners or
  losers.
- **Jev's step Nouls.** The shares of steps Jev judged to check an
  assumption or to act on earlier evidence moved by less than 0.15 in every
  comparison. On the steps the rules leave, these judgments don't
  distinguish strategies.
- **The task's example.** Only 3 of the 14 tasks name a runnable command or
  script in backticks, too few to test "run the example before editing".

## Limits

- **Luna is thin.** 20 graded Luna attempts, 1 to 2 per task, and none
  passed. Luna appears only as a comparison group, and a single attempt
  decides its side of a task. Rerun `gym runs moves` as #9583 finishes; the
  report is cached for `coder-one ask`.
- **Nothing here is causal.** Fable, Luna, Coder One, and Claude Code differ
  in model, harness, effort, and budget. A candidate move is worth an
  experiment, and the experiment decides.
- **Rules see commands, not their effects.** A program that writes a file is
  not an edit to the rules, so first-edit times and file counts miss
  outputs generated by scripts. Coder One's briefing does some reading
  before the executor's first step.
- **Fable's public trajectories time whole steps.** Harbor timestamps each
  ATIF step, not each tool result.

## Reproduce

Fingerprints read the public Fable collection from
`~/.openagents/terminal-bench/public-replays/`
([head to head](../gym/head-to-head.md#public-fable-collection)) and local
jobs from `~/.openagents/terminal-bench/jobs/`.

```sh
CARGO_TARGET_DIR=~/.cache/openagents/gym-target \
  cargo build --release -p gym --bin gym
GYM=~/.cache/openagents/gym-target/release/gym
$GYM runs moves
$GYM runs fingerprints --task mvcc-lsm-compaction --agent luna
$GYM runs fingerprint 36f015a8-ef13-41da-b0e7-970064be6026
$GYM runs fingerprints --agent coder-one --json
```

`gym runs moves` fingerprints the subset, asks Jev about unplaced steps it
hasn't seen, and keeps its report in `~/.openagents/gym/fingerprints/moves.json`.
Add `--no-jev` to read stored answers only. `coder-one ask` reads the
fingerprints of the tasks a question names, and the cached moves report
when the question asks about Fable, strategies, moves, phases, or
fingerprints. Head-to-head replay shows each event's phase beside its
timing label, with `(Jev)` when Jev placed it.
