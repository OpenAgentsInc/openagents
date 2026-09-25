# Issue-flow default policy: read-confined replication

Execution status: **invalid, stopped after two attempts**. The new scope
denied lean sessions access to their own frozen evaluator; the host scorer
also omitted the evaluation seal. [Issue #9663](https://github.com/OpenAgentsInc/openagents/issues/9663)
repairs both paths. These attempts are retained, with no default decision.

Issue [#9624](https://github.com/OpenAgentsInc/openagents/issues/9624).

This is a new comparison after [the original study](../2026-09-25-issue-flow-policies/protocol.md) stopped invalid after three attempts. Those attempts are retained and excluded here. Both arms use the repaired read boundary from #9661. No task, policy, order, grader, or decision rule changes. The original $0.047668704 counts toward the combined $5 budget.

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

## Frozen implementation and host

The host remains macOS 26.4 arm64 with 128 GiB RAM and 18 logical CPUs.
The executable, source, policies, evaluation set, and environment are
pinned in [pins.json](pins.json) before inference. Both arms use six Cargo
workers and no development or test debug information. Python 3.13 is
first on PATH for the cancellation fixtures. GNU timeout is Coreutils 9.12.

The seal is `candidate-and-toolchain-v1`: commands and the issue-flow gate
can read only the candidate, owned scratch and Cargo metadata, installed
tools, and prefetched registry sources. The gate uses the candidate's own
Cargo target. The independent grader retains its separate build directory.
The [repair report](../../../../docs/coder/verification/2026-09-25-issue-eval-read-isolation.md)
records the exact grants and verification. This boundary is common to both
arms. The executable remains unchanged for all 16 slots.

Before inference, all four graders must discriminate again. A scripted,
zero-model-cost preflight must also build a real development candidate
through the new read boundary. These checks and the manual Rust gate
are retained in [records/preflight](records/preflight/).

[run.py](run.py) reserves every slot before launch and takes an exclusive
lock. The driver checks for an operator `STOP` marker between attempts;
it does not silently replace an interrupted attempt. Completed attempts
must name the pinned policy and the repaired read boundary.
[summarize.py](summarize.py) applies the unchanged decision rule.
[publish.py](publish.py) retains every run's manifest, grader, diff, native
and ATIF traces, costs, times, and per-file digests. It scans for configured
credentials before publication. Any private-data exposure requires a local
quarantine and an invalidation report, not a public transcript.

An artifact passing its grader and the flow declaring itself finished are
reported separately. Confidence intervals are descriptive only: these are
four selected development entries with repeated attempts, not independent
samples from future GitHub issues. No held-out or Terminal-Bench outcomes
are opened or used by this study.

After a final receipt is retained, the driver removes only that attempt's
ignored `repo/target/` compiler output and writes a cleanup receipt. The
source diff, retained candidates, commands, output streams, and transcripts
remain. This bounds disk use on the Mac without sharing candidate builds.

Preflight passed: all four graders discriminate, and the scripted historical
`coder` crate check finished successfully through the confined command path
in 46.474 seconds. Its intentionally unsolved issue grade is a failure,
not a model outcome. [Compiler receipt](records/preflight/compiler-receipt.json)
and [full preflight trace](records/preflight/compiler-traces.tar.gz) retain
that distinction. The preflight binary precedes the unrelated fire-loop
soft-stop commit integrated into the pinned build; [build identities](records/preflight/build.json)
retain that source identity. No issue-flow, boundary, or grader code changed
between the verified repair and the pinned build.
