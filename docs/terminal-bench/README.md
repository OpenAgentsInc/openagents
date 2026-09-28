# Terminal-Bench

Use this index to find retained results, comparison limits, runbooks, and full
traces. Updated September 27, 2026. Repository reports describe recorded
attempts; they do not establish a remote host's live queue.

## Current evidence

| Topic | Read this | What the evidence supports |
| --- | --- | --- |
| Microcoder development | [Per-task ledger](tb4-results.md#microcoder-development-runs-in-sample) and [Gym highlights](../gym/terminal-bench-cli.md#beat-the-winner) | Selected knowledge-assisted wins on tasks used to develop the entries. Keep each provider, cost basis, population, and source-provenance label; these are not an out-of-sample result. |
| Out-of-sample cost on TB2.1 | [Pre-registration](2026-09-26-tb21-oos-study.md), [results](2026-09-26-tb21-oos-results.md), [retained records](../../bench/terminal-bench/microcoder-runs/coderos-4080-tb21/), and [JSON report](../../bench/terminal-bench/studies/2026-09-26-out-of-sample/t1-report.json) | TB2.1, knowledge off, list price: 30 confirmed out-of-sample wins on 65 tasks against Fable 5 xhigh's cost per trial; median pass at 2.9% of it. First runs passed 48% of tasks, against Fable 5 xhigh's 92% of trials. TB2.1 is older and easier than TB4. |
| Prospective knowledge transfer | [Frozen study](2026-09-26-out-of-sample-study.md), [retained results](2026-09-26-out-of-sample-study-results.md), and [follow-up knowledge](2026-09-26-round2-knowledge.md) | A separately owned study. Read its current reports for completed coverage, exclusions, and uncertainty; development wins cannot substitute for it. No held-out TB4 pass yet. |
| Coder One delegating to Fable 5.1 low | [Declaration and results](2026-09-27-fable-delegate.md) and the [winning attempt's trace](../../bench/terminal-bench/traces/tb4--coder-one-delegate-fable-low-kb-jev2--fin-saccr-rwa--9746-s7a1/) | One declared attempt in which Jev decided the briefing beat Fable 5.1 low on both cost and time: s7a1 passed `fin-saccr-rwa` for $0.9429 in 149.5 s, against Fable 5.1 low's cheapest win ($1.2246) and fastest win (222.5 s). Before delegation, Jev chose 5 of 12 knowledge candidates and flagged 3 of 6 requirements, and its usage record shows that 1 Jev decision. It is in-sample and knowledge-assisted (every kept entry was written from earlier runs on this task) and tuned (the question set and one briefing sentence were chosen after that task's earlier results). The earlier win, s5a2 ($0.8816, 174.1 s), had no Jev decision. Across 7 series, 2 of 13 attempts beat the bar. |
| The same Fable delegate on 14 more tasks | [Reproduction](2026-09-27-fable-delegate-repro.md) and its [per-attempt numbers](../../bench/terminal-bench/experiments/2026-09-27-fable-delegate-repro/attempts.json) | s7a1's frozen arm, pre-registered in #9776, on the 14 other TB4 tasks Fable 5.1 low passes outside the study pools, two passes. It beat Fable 5.1 low's cheapest and fastest wins on 4 of 28 attempts: 0 of 14 in pass 1 and 4 of 14 in pass 2 (`coq-block-bound`, `gsea-proteomics`, `mp-checkpoint-consolidation`, and `sound-change-cascade`). It passed 13 of 28. Every beat used knowledge written from that task (in-sample), two beat the cost bar by under 3%, and neither task without its own knowledge beat the bar. 20 delegates ran to their deadline, so their cost is unknown. |
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
- [Published results](../../bench/terminal-bench/published/): the Gym's
  generated leaderboard and scrubbed trace bundles for the #9776 and TB2.1
  studies ([spec](../verse/gym-leaderboard.md)). Regenerate with
  `cargo run -p gym-leaderboard -- build` after adding evidence.
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
