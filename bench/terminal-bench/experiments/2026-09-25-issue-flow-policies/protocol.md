# Issue-flow default policy: protocol

Issue [#9624](https://github.com/OpenAgentsInc/openagents/issues/9624).
Written on 2026-09-25 at commit `70fa97a2b5`, before any run of either arm
on the issue-flow evaluation set.

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

**Primary.** Passed runs per arm: runs whose grade verdict is `pass`
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

## Execution host and preflight, before live attempts

The comparison runs on the operator's macOS 26.4 arm64 machine, with
128 GiB of memory and 18 logical CPUs. Coderos is running separately owned
fire-loop experiments. No other benchmark cohort runs on this Mac during
the comparison. This selects the host; it changes no task, arm, order,
budget rule, grader, or decision rule above.

[`pins.json`](pins.json) records the source commit, executable digest,
policy file and canonical digests, set digest, and build environment.
Both arms use six Cargo build workers and no development or test debug
information. The grader uses a dedicated target directory outside each
candidate. The issue flow retains its existing target-directory behavior.
The executable stays fixed throughout the study, even if main advances.

The first grader preflight failed because the Mac lacked GNU `timeout`.
After installing Coreutils 9.12, all four graders discriminate: the base
fails and the known fix passes. Both preflights are retained in
[`records/preflight/`](records/preflight/). The 31 issue-flow tests pass,
including the scripted full flow, lean turn, and enforced offline gate;
18 subprocess tests pass on macOS. These are environment and integration
checks, not live model outcomes.

[`run.py`](run.py) runs the registered order under an exclusive lock,
records a launch before each attempt, and retains every final manifest
and both output streams. An interrupted attempt or missing manifest stops
the driver for inspection; it never silently replaces that attempt.
Recorded costs follow the budget rule above. A missing total remains a
lower bound and cannot establish a cheaper-policy claim. Full native and
ATIF transcripts and candidate diffs are retained beside each run; the
report links the published evidence.
