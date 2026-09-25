# Issue-flow default policy: sealed-scorer replication

Issue [#9624](https://github.com/OpenAgentsInc/openagents/issues/9624).

Two earlier comparisons are invalid and excluded: the [original three attempts](../2026-09-25-issue-flow-policies/protocol.md) exposed private history, and the [read-confined pair](../2026-09-25-issue-flow-policies-read-confined/protocol.md) exposed a missing evaluator grant and an unsealed host scorer. They remain retained. Their combined **$0.093755924** counts toward the original $5 budget.

Both arms use the same repaired harness. Tasks, policies, order, graders, and the decision rule below remain unchanged.

## Question

Which Microluna manifest the issue flow uses by default:
`issue-flow.json`, the requirements loop and the current default, or
`issue-flow-lean.json`, the lean loop.

## Arms

| Arm | Manifest | Loop | File SHA-256 |
| --- | --- | --- | --- |
| Requirements | `crates/coder-one/policies/issue-flow.json` (`coder-issue-flow`) | requirements | `de038ac0976d3aee7f441a7d68a7fd4fb4cbde9919a031366ed21e1d5bb0d82b` |
| Lean | `crates/coder-one/policies/issue-flow-lean.json` (`coder-issue-flow-lean`) | lean | `c593246d858ba17eba907954c82d2690a79e9bcc42b5d20b6d60af7f51bf8edf` |

Each run records the manifest's canonical digest (`Manifest::digest`) in
its `policy` field. Every run of an arm must record the same digest; a run
that records another is left out and reported.

## Entries

The development part of `crates/coder-one/issues-eval/` only (set digest
`b85ff8426d91c44f`): `9597-minitask-explanation`,
`9450-delegate-stale-bullet`, `9451-delegate-answer-channel`, and
`9446-coderdev-launcher`. The held-out part isn't run.

The requirements loop was tuned on #9597 over 14 attempts, so that entry
favors it. Results are reported per entry, and #9597 is named apart.

## How each run goes

```bash
coder-one issue-eval run ENTRY --policy MANIFEST
```

Sealed with the defaults: GitHub withheld, the network off, Jev live,
`gpt-6-luna`. One run at a time, and no other benchmark cohort running on
the host. Before the first run, `coder-one issue-eval verify --part
development` must show every development grader discriminating (the base
fails, the fix passes); if one doesn't, nothing runs.

## Repeats and order

Two runs per entry per arm, 16 runs, in two rounds. The arms alternate in
an ABBA order so neither arm always goes first:

| # | Round 1 | # | Round 2 |
| --- | --- | --- | --- |
| 1 | 9450 lean | 9 | 9450 requirements |
| 2 | 9450 requirements | 10 | 9450 lean |
| 3 | 9446 requirements | 11 | 9446 lean |
| 4 | 9446 lean | 12 | 9446 requirements |
| 5 | 9451 lean | 13 | 9451 requirements |
| 6 | 9451 requirements | 14 | 9451 lean |
| 7 | 9597 requirements | 15 | 9597 lean |
| 8 | 9597 lean | 16 | 9597 requirements |

## Spend cap

$5.00 in total, Luna and Jev together, as each run's `cost.total_usd`
records it. A run with a null cost counts at its `lower_bound_usd`, and
the report says so.

- Runs go in pairs (1 and 2, 3 and 4, and so on), so both arms always
  have the same entries.
- Before a pair starts, if the spend so far plus twice the largest run
  cost seen would pass $5.00, the pair doesn't start and the experiment
  stops there.
- If round 1 costs more than $2.50, round 2 doesn't run, and the result
  rests on one run per entry per arm. The report says so.

## Measures

**Primary.** Passed runs per arm: runs whose grade verdict is `passed`
and that aren't contaminated. Also reported: entries each arm passed at
least once.

**Secondary.**

- Checks passed, per run and summed per arm.
- Cost per run with Jev included (`cost.total_usd`), and its Luna and Jev
  parts.
- Wall time per run (`milliseconds` and `flow_milliseconds`).
- Sealing violations: attempts the seal blocked, and any run marked
  `contaminated`.
- Outcome (`finished`, `unfinished`, or `stuck`) and why each failure
  failed, read from its failing checks.

## Failures and reruns

Every run is recorded, including failures. A contaminated run counts as
not passed. A run that stops for a reason outside the arm, such as the
Codex login failing or the host running out of disk, is rerun once, and
both records are kept. A run the loop itself ended badly is a result, not
a reason to rerun.

## Decision rule

Let *d* be the lean arm's passed runs minus the requirements arm's, and
compare the mean cost per run with Jev included.

| *d* with two runs per cell | *d* with one run per cell | Decision |
| --- | --- | --- |
| 2 or more | 1 or more | The lean manifest becomes the default. |
| 0 or 1 | 0 | The lean manifest becomes the default only if its mean cost per run is at least 20% lower. Otherwise it's a tie, and nothing changes. |
| −1 | none | A tie within noise. Nothing changes. |
| −2 or less | −1 or less | The requirements manifest stays the default. Nothing changes. |

A lean default is also refused if a lean run is contaminated and no
requirements run is. With eight runs per arm, a one-run difference is
noise, and the report calls it that.

## Records

`records/` holds, for each run, its `manifest.json` and
`verification/grade.json`, named by run number, and `runs.jsonl`, one line
per run in the order they ran. The report is
`docs/coder/measurements/2026-09-25-issue-flow-policies.md`.

## Frozen implementation and preflight

The host remains macOS 26.4 arm64, with 128 GiB RAM and 18 logical CPUs.
[pins.json](pins.json) freezes the source, executable, policies, evaluation
set, and environment before inference. Python 3.13 is first on PATH;
both arms use six Cargo workers and no dev/test debug information.
The executable stays fixed, even if main advances.

The seal remains `candidate-and-toolchain-v1`. Session commands, the issue
flow's test gate, and the host's frozen scorer use that scope. Only the
host-selected evaluator is added read-only to a later lean session; sibling
artifacts, other attempts, history, and credentials remain unreadable.
The gate builds in the candidate's target directory; the independent
grader uses its own cache. The
[repair report](../../../../docs/coder/verification/2026-09-25-issue-eval-read-isolation.md)
records the scope and both invalid studies.

Before inference, all four graders must discriminate with the pinned
binary. The Rust gate must pass the two-session sealed-lean regression:
freeze a failing evaluator, fix the candidate, read and run that evaluator
without modifying it, refuse sibling reads, and let the passing score
satisfy the finish hook. The host scorer is tested with the same attempts
to read outside and bypass the GitHub stub. The earlier compiler preflight
also built the historical coder crate through the shared scope.
[records/preflight](records/preflight/) retains these checks.

[run.py](run.py) uses one exclusive lock and reserves each slot before
launch. A `STOP` marker stops it between attempts without replacement.
The driver checks the binary digest for every launch. It removes only
ignored `repo/target/` compiler output after a final receipt, recording the
cleanup; sources, retained candidates, commands, outputs, and traces remain.
No other local benchmark cohort runs at the same time.

[summarize.py](summarize.py) applies the unchanged decision rule.
[publish.py](publish.py) publishes every manifest, grade, diff, native and
ATIF transcript, costs, times, and archive/member digests after scanning for
configured credentials. Private content, if found, is quarantined locally
and invalidates the comparison rather than entering a public transcript.

Artifact success and the flow's `finished` status are separate measures.
Intervals are descriptive: four selected development entries repeated
twice are not eight independent samples from future GitHub issues. This
compares the complete policy configurations, whose session and tool bounds
differ; it does not isolate one algorithmic component. No held-out or
Terminal-Bench outcomes are used.

For descriptive uncertainty, the summary reports Wilson intervals for the
attempt counts and a paired task bootstrap: all 256 ways to sample four
tasks with replacement, keeping each task's two repeats together. It
reports the 2.5th and 97.5th percentiles of the lean-minus-requirements
pass, cost, and time differences. Four tasks still give weak coverage of
future work. These intervals do not change the registered decision rule.

## Operator stop: 2026-09-25 22:04:39 UTC

The operator requested ending the study and closing #9624 with its existing
results. Slots 1–8 completed; slot 9 was interrupted; slots 10–16 never
started. The complete first round is reported with the original one-repeat
decision table. This stop is a user-directed amendment, not the registered
budget stop, and the planned second round remains incomplete. Both arms
pass 1/4; lean is 17.7% cheaper, below the 20% tie threshold. Requirements
remains the default. The partial ninth attempt and both invalid studies
remain retained and counted separately. No live attempt resumes.
