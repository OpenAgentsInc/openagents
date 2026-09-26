# Frozen knowledge comparisons

`microcoder kb study` runs one fixed, signed knowledge snapshot against no
knowledge base. It follows NIP-OPT's separation of frozen plans, assignments,
actual execution receipts, and retained results. Its local
`openagents.kb-study.v1` profile is narrower than the complete NIP-OPT wire
contract: it does not implement search, arbitrary optimization strategies,
automatic promotion, or remote study admission. It is infrastructure for separately
reviewed studies. It does **not** implement or replace the screen-and-confirm
schedule registered in [#9683](https://github.com/OpenAgentsInc/openagents/issues/9683)
or its [frozen protocol](../../terminal-bench/2026-09-26-out-of-sample-study.md).

The [historical evidence reader](knowledge-evidence.md) cannot supply a
prospective comparison. This runner freezes the executable, candidate bytes,
task and environment source trees, settings, task families, partitions, bounds,
and assignment order before dispatch. It retains a start receipt before each
child process and never replaces a started assignment.

## Prepare and freeze

Create a signed snapshot with `microcoder kb snapshot` as described in the
[knowledge guide](../guides/knowledge-base.md), then draft a plan:

```sh
microcoder kb study draft snapshot.json /absolute/path/to/tasks plan.json task-a task-b task-c task-d
```

Drafting invokes no model. Review the generated plan before freezing it:

- Choose task families independently of the outcomes being measured. The draft
  uses task names as provisional group labels; correct those labels when tasks
  share a source family.
- List every task considered while constructing the snapshot in `source_tasks`.
  The validator also reads `written_from` directly from the verified snapshot.
  Source tasks cannot enter confirmation, and groups cannot cross partitions.
- The supported provider is `codex`, with `list_price` as the cost basis. This
  helper forces lexical retrieval, uses the hosted TypeSafe decision endpoint,
  and pins its requested decision model. It does not call OpenRouter. Choose
  the exact model, effort, step/time/spend limits, command and test
  deadlines, network, and prompt. Both arms use the same values. This profile
  freezes stronger-model routing to `never`.
- Review the aggregate reservation. The draft uses one pair per task, 30 steps,
  ten agent minutes, and $0.25 per assignment. The budget is model-reported
  spend with possible in-flight-call overshoot, not a hard wallet ceiling.
- Keep the pinned executable unchanged for the cohort. Rebuilding it changes
  its digest and prevents further dispatch.

```sh
microcoder kb study freeze plan.json snapshot.json /new/absolute/study-directory
```

An existing directory refuses. The new directory contains the exact candidate,
canonical plan, complete assignment schedule, and freeze receipt. Publish or
otherwise retain the freeze commitment before reading any new outcomes.

## Run and inspect

```sh
microcoder kb study run /absolute/study-directory
microcoder kb study report /absolute/study-directory
```

Only `run` starts model work. It checks the pinned executable and task source
trees again, then acquires one execution-owner lock. Subject assignments use
`--kb candidates --kb-snapshot <frozen file>`; baseline assignments use
`--kb off`. Each subject gets a fresh embedding-cache path. Both arms get new
output directories and explicit container identities. No ambient local or
synced entries enter the subject snapshot.

The shared subprocess supervisor bounds each harness child, including startup
and verifier allowance. The runner confirms that the assignment's container is
absent before proceeding. A cleanup failure stops the cohort. A process crash
leaves the started assignment unknown; restarting the runner refuses to
silently run it again. Investigate and retain that interruption instead of
replacing the attempt with a successful retry.

Every finish receipt binds the exact start receipt, summary, event log, observed
model names, exit and ending, elapsed time, rewards, and known or unknown costs.
Summary provider identity must agree with the frozen provider; a missing or
mismatched identity cannot count as a success under that configuration.
The report includes every frozen assignment, including tasks not started and
attempts interrupted before a summary. A missing outcome is never changed to a
pass or discarded. Missing billing fields or zero-priced model calls without
proof of a true zero charge remain unknown. Each task and the development and
confirmation partitions have separate denominators. Missing-outcome bounds
include all assignments; Wilson intervals describe only graded attempts and
assume independent trials. Repeated tasks and shared source families limit
that assumption.

## What the result establishes

The result reports the assigned cohort's outcomes. Its verdict remains
`inconclusive` for automatic admission. A small fixed cohort, even one with
several wins, does not establish reliable transfer across tasks or an isolated
entry's effect. A snapshot can contain several entries; a snapshot comparison
does not award each one the credit.

The source manifests bind task and environment files. They do not attest that
remote package repositories, Docker base tags, or a provider deployment stayed
identical. A requested model alias is a configuration pin, not an immutable provider
artifact identity. A served model name that differs from the frozen request is reported
as a discrepancy. Source-family independence is an operator declaration and
needs review. These limitations belong in the published assessment alongside
all failed runs, unknown charges, full transcripts, and timing receipts.

Do not describe the local profile as complete NIP-OPT host conformance. Full wire-contract and independent-admission support remain outside this
helper. [#9670](https://github.com/OpenAgentsInc/openagents/issues/9670) records
the original knowledge work; [#9683](https://github.com/OpenAgentsInc/openagents/issues/9683)
owns the separately registered out-of-sample cohort. No measured improvement
or entry admission follows from building this runner.
