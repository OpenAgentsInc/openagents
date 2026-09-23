# Terminal-Bench status

Reviewed through `3f0bdc6621` on 2026-09-23. This page summarizes committed
evidence; it does not report live execution-host progress.

## Latest status

**TB4 results need a quota audit.** Incident note `4b6c619770` reports 21 attempts
that hit Claude's usage limit but were graded anyway. It reports 19 moved
and rerun without reconciling their replacements or the remaining two.
The published scoreboard predates that note. Its totals below are a
historical snapshot, not corrected pass rates.

| TB4 arm | Last published passes / graded | Status |
| --- | ---: | --- |
| Coder One tunable v2 | 16/36 | Pending quota reconciliation. |
| Coder One tunable v3 | 4/5 | Selected tasks; pending reconciliation. |
| Coder One tunable v4 | 2/5 | Selected tasks; pending reconciliation. |
| Coder One tunable v5 | — | Persistence implemented; no graded results in the snapshot. |
| Coder One tunable v6 | — | Second executor only on failed checks, at most two persistence rounds; no published graded results. |
| Claude Code / Opus 5.5 | 4/13 | Pending quota reconciliation. |

See the [full 66-task matrix and corrected comparisons](tb4-results.md)
and [incident record and reconciliation requirements](data-quality.md).
The arms cover different task sets. The previous cost comparison used
full-suite leaderboard averages against a subset; matching the 36 v2
tasks changes reference costs to $6.74 and $13.38 per trial, from $9.90
and $18.92. Claims about relative capability await the quota audit.

**Eight-task development results remain promising, with limits:**

- **Luna plus the coverage packer: 24/24.** Recomputed cost is a **$0.1504
  lower bound**, with four unpriced dispatches; agent time is 385.0 s.
  Both figures sum eight per-task means, not all 24 trials. The prior
  “86% cheaper” claim is not established with missing charges.
- **Turning off monitor stops: 22/24.** Git recovery and vulnerability
  failures escaped the checks. Better check coverage remains the next
  requirement for reliable Luna-first execution.
- **Tunable Opus: 24/24**, $0.5219 and 253.3 s, versus direct Opus's
  $1.0863 and 306.4 s on the same basis. Routing selected Opus every time.
- **Coverage packing on log summaries: 3/3**, versus the older Jev-probe
  v3's 0/3. It preserves data records that the old briefing dropped.

Details and evidence: [tunable results](2026-09-23-tunable-results.md).
These are small development screens, not held-out benchmark estimates.

**Harness:** TB4 has 66 pinned tasks; CPU and RTX 4080 oracle checks pass.
`fp8-rmsnorm-gemm` requires an H100 and cannot run on the current Linux
host. A usage-limited trial is now its own outcome: Coder One exits 6,
the harness withholds the reward, the scheduler requeues the trial and
pauses the provider until the limit resets, and a host-wide cap runs at
most two Claude trials at once
([#9564](https://github.com/OpenAgentsInc/openagents/issues/9564);
[runbook](runbook.md#schedule-a-suite)). The trials graded before that
still need the quota audit. See [harness validation](tb4-results.md#harness-validation-and-remaining-coverage).

## Results and analysis

| Document | Contents |
| --- | --- |
| [TB4 results](tb4-results.md) | Full matrix, arm versions, matched comparisons, harness checks, and refresh limits. |
| [Coder One's task-level wins](2026-09-23-task-win-analysis.md) | Wins against Astra and Opus reference rows, possible causes, full-suite scenarios, and the experiment needed to establish an advantage. |
| [TB4 leaderboard](tb4-leaderboard.md) | Retained public reference: 27 rows, five trials per task, source inconsistencies. |
| [Tunable results, September 23](2026-09-23-tunable-results.md) | Coverage packing, routing, repair, escalation, and monitor experiments. |
| [Development results, September 22](development-results.md) | Four- and eight-task comparisons, cost rankings, individual trial tables. |
| [Original run analyses](2026-09-22-run-analyses.md) | Per-run cost, steps, tokens, time, and delegate behavior. |
| [Measurement and pricing](measurement.md) | Column definitions, aggregation, cost provenance, and historical rates. |
| [Data quality](data-quality.md) | Quota audit, invalid attempts, missing charges, and host differences. |
| [TB4 failure analysis](2026-09-23-tb4-failure-analysis.md) | First failures and the check, routing, and persistence gaps. |
| [Luna/Jev assessment and upgrade plan](2026-09-22-luna-jevprobe-upgrade.md) | Architecture diagrams, detailed probe failures, and proposed upgrades. |
| [Winning runs](winning-runs-analysis.md), [routing](2026-09-22-routing.md), [packing study](2026-09-22-pack-study.md) | Earlier mechanism analyses and replay studies. |

## Run, inspect, and extend

- Run: [operating notes](runbook.md), [harness guide](../coder/terminal-bench.md),
  [delegate runbook](coder-one-delegate-runbook.md), and [resilience](resilience.md).
- Inspect: [Gym TUI](../gym/terminal-bench-tui.md), [Gym CLI](../gym/terminal-bench-cli.md),
  [retained traces](../../bench/terminal-bench/traces/), and [evidence retention](runbook.md#retain-the-evidence).
- Extend: [tunable components](../optimization/coder-components.md),
  [implemented policies](../coder/guides/coder-one-tunable.md),
  [episode contract](../coder/terminal-bench-contract.md), and
  [Coder v0.5 design and goldens](../coder/design/coder-terminal-v05-algorithm-and-goldens.md).
- Inspect prompts: [Claude system prompt](claude-code-delegate-prompt/system-prompt.md),
  [captured request](claude-code-delegate-prompt/request.json), and
  [delegate prompts](delegate-prompts/).
- Track: [harness #9530](https://github.com/OpenAgentsInc/openagents/issues/9530),
  [Coder One #9531](https://github.com/OpenAgentsInc/openagents/issues/9531),
  [TB4 #9558](https://github.com/OpenAgentsInc/openagents/issues/9558), and
  [suite infrastructure #9559](https://github.com/OpenAgentsInc/openagents/issues/9559).

After a run, follow the [publication procedure](runbook.md#after-each-run).
Keep this page to current status and links; add detailed results and
analyses to the documents above or a new dated report.
