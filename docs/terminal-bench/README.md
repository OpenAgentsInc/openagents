# Terminal-Bench

Use this index to find retained results, comparison limits, runbooks, and full
traces. Updated September 26, 2026. Repository reports describe recorded
attempts; they do not establish a remote host's live queue.

## Current evidence

| Topic | Read this | What the evidence supports |
| --- | --- | --- |
| Microcoder development | [Per-task ledger](tb4-results.md#microcoder-development-runs-in-sample) and [Gym highlights](../gym/terminal-bench-cli.md#beat-the-winner) | Selected knowledge-assisted wins on tasks used to develop the entries. Keep each provider, cost basis, population, and source-provenance label; these are not an out-of-sample result. |
| Prospective knowledge transfer | [Frozen study](2026-09-26-out-of-sample-study.md), [retained results](2026-09-26-out-of-sample-study-results.md), and [follow-up knowledge](2026-09-26-round2-knowledge.md) | A separately owned study. Read its current reports for completed coverage, exclusions, and uncertainty; development wins cannot substitute for it. |
| Earlier fire loop | [In-sample ledger](tb4-results.md#fire-loop-development-runs-in-sample) and [guide](../coder/guides/fire-loop.md) | Microluna v19-fire passed embedding 5/5 at about $0.0153 per run, excluding the fire-loop judge. Its 5:26 median was slower than Fable low's 2:55. The task was used for development. |
| Matched controller test | [Ten-task Opus experiment](2026-09-23-matched-controller-targeted.md) | Coder One v8 passed 18/30 versus plain Claude Code's 15/30 under the same executor configuration, while costing 68% more and taking 2.2 times the agent time. The difference in passes was not established statistically. |
| Negative family result | [Microluna v18](2026-09-25-microluna-v18-family.md) | 0/9 confirmation and 0/9 development. Setup changes and restart make the strict protocol result inconclusive; no policy was promoted. |
| Truthful checks | [72-candidate confirmation](2026-09-25-archive-check-confirmation.md), [literal checks](2026-09-25-literal-artifact-checks.md), and [later protocol](../../bench/terminal-bench/experiments/2026-09-25-literal-confirmation/protocol.md) | The archive result missed its joint precision/recall improvement requirement. The later protocol and launch records establish a plan and launch, not a completed measurement or current queue state. |
| Historical TB4 scoreboard | [Full ledger](tb4-results.md) and [data quality](data-quality.md) | Preserved totals and later per-task records have different populations. The historical full scoreboard still carries its quota-reconciliation caveat. |

[#9584](https://github.com/OpenAgentsInc/openagents/issues/9584#issuecomment-5841989077)
and [#9607](https://github.com/OpenAgentsInc/openagents/issues/9607#issuecomment-5841988664)
closed when development moved from the Microluna component stack to Microcoder.
Closure is not evidence that either issue's original scientific claim passed.
The [report catalog](reports.md) preserves the negative studies, rejected rules,
known false positives, full version arc, and earlier comparisons. The former
long status page is retained as a [dated snapshot](2026-09-25-status-snapshot.md).

## Inspect and compare

- [Head-to-head replay](../gym/head-to-head.md): load public Fable and local
  transcripts, inspect timing quality, and navigate recorded evidence.
- [Gym TUI](../gym/terminal-bench-tui.md) and [Gym CLI](../gym/terminal-bench-cli.md):
  browse runs, costs, grades, and retained comparisons.
- [Traces](../../bench/terminal-bench/traces/) and
  [retention requirements](runbook.md#retain-the-evidence): find the original
  events and artifacts behind a result.
- [Measurement and pricing](measurement.md), [data quality](data-quality.md),
  and [leaderboard reference](tb4-leaderboard.md): distinguish whole-trial time,
  agent time, list-price estimates, billed costs, missing costs, and incomparable
  configurations.

A lower-cost successful run demonstrates that recorded attempt. To measure the
added effect of Coder, hold the executor model, effort, tools, budgets, task,
and environment fixed. Keep failures and missing evidence in the population.
Use the [matched experiment](2026-09-23-matched-controller-targeted.md) as the
reference for that distinction.

## Operate and extend

| Need | Documentation |
| --- | --- |
| Harness setup and operations | [Harness guide](../coder/terminal-bench.md), [runbook](runbook.md), [resilience](resilience.md) |
| New repeated comparison | [Targeted experiment template](targeted-experiment-template.md), [measurement rules](measurement.md) |
| Agent integration | [Episode contract](../coder/terminal-bench-contract.md), [tunable components](../optimization/coder-components.md), [policy guide](../coder/guides/coder-one-tunable.md) |
| Delegate configuration | [Delegate runbook](coder-one-delegate-runbook.md), [Claude prompt](claude-code-delegate-prompt/system-prompt.md), [captured request](claude-code-delegate-prompt/request.json), [other captured prompts](delegate-prompts/README.md) |
| Failure analysis and task selection | [Capability gaps](capability-gaps.md), [task anatomy](2026-09-24-task-anatomy.md), [task-fit study](../coder/measurements/2026-09-22-terminal-bench-chat-fit.md) |
| Current knowledge and contribution work | [Knowledge guide](../coder/guides/knowledge-base.md), [quest board](quest-board.md), [separately maintained contributor plan](../coder/beat-fable-together.md) |
| General repository task host | [Microcoder adapter](../coder/runtime/microcoder-repository.md) and [stopped acceptance record](../coder/verification/2026-09-26-repository-adapter/README.md); these are product-contract fixtures, not Terminal-Bench results |

Publish detailed analyses in a dated report and link them from
[the catalog](reports.md). Keep this page to navigation and bounded summaries.
