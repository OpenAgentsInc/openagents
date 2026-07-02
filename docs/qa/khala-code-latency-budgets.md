# Khala Code Latency Budgets

Date: 2026-07-02
Status: Q2.2 budget catalog and Q2.5 trend reporting implemented.

The Khala Code QA budget family lives in
`clients/khala-code-desktop/src/shared/qa-metrics.ts`. The `qaMetrics` RPC
returns the same budget records with every snapshot, and the
`packages/khala-qa-harness` scenario `perf` oracle evaluates those records
from the snapshot samples.

## Budget Catalog

| Budget | Metric | Threshold | Unit | Percentile |
| --- | --- | ---: | --- | --- |
| `budget.khala_code.startup_interactive.v1` | `startup.interactive_ms` | 3000 | `ms` | n/a |
| `budget.khala_code.thread_switch.optimistic.v1` | `thread_switch.optimistic_render_ms` | 100 | `ms` | n/a |
| `budget.khala_code.thread_switch.full.v1` | `thread_switch.full_render_ms` | 400 | `ms` | n/a |
| `budget.khala_code.turn_start.first_event.v1` | `turn_start.first_event_ms` | 400 | `ms` | n/a |
| `budget.khala_code.composer.keystroke_echo.p95.v1` | `composer.keystroke_echo_ms` | 16 | `ms` | p95 |
| `budget.khala_code.panel.open.v1` | `panel.open_ms` | 150 | `ms` | n/a |
| `budget.khala_code.sse.event_to_ui.p95.v1` | `sse.event_to_ui_ms` | 250 | `ms` | p95 |
| `budget.khala_code.transcript.scroll_dropped_frames.v1` | `transcript.scroll_dropped_frames_pct` | 5 | `percent` | n/a |
| `budget.khala_code.app_server.spawn_ready.v1` | `app_server.spawn_ready_ms` | 2000 | `ms` | n/a |
| `budget.khala_code.cockpit_render.50_cards.v1` | `cockpit.render_ms` | 100 | `ms` | n/a |
| `budget.khala_code.lifecycle_event_to_card.p95.v1` | `lifecycle_event_to_card.ms` | 500 | `ms` | p95 |
| `budget.khala_code.supervisor_tick.25_target.v1` | `supervisor.tick_ms` | 1000 | `ms` | n/a |

## Evaluation Path

Scenario phases that include a `perf` oracle automatically read `qaMetrics`
from the active driver. The oracle can match by `budgetId` (`match`) or by
metric name (`metric`) and evaluates the selected budget against the snapshot
samples.

The nightly matrix includes the full catalog in `qa-status-surface.json` and
`qa-status-surface.md` under `latencyBudgets`. Catalog rows are public-safe and
show the budget ID, metric, threshold, unit, percentile, sample count, and the
current catalog evaluation status. Runs with no samples are intentionally
`inconclusive`.

The Q2.6 sample-coverage burn-down slice wires real UI samples for the Q2.2
paths that previously could not become green in a nightly report:
startup-to-interactive, composer keystroke echo, preview/native chat
event-to-UI, first visible turn event, transcript scroll dropped-frame percent,
and app-server spawn-ready time.

Each nightly also persists a `latencyBudgetRun` object inside
`qa-nightly-report.json`. It records each budget's actual value, evaluation
status, sample count, and up to five offending public-safe sample refs collected
from `qaMetrics` snapshots in the current artifact directory. The Q1.5 status
surface compares that object with prior nightly reports and exposes
`latencyBudgets.trends` plus `latencyBudgets.regressionCount`.

When a budget regresses and `OA_QA_NIGHTLY_FILE_PERF_ISSUE=1` is armed, the
owned runner files a strict-form issue with the regressed budget IDs and sample
evidence. See
[`khala-code-perf-trend-regressions.md`](./khala-code-perf-trend-regressions.md).

Q2.3 adds a repeatable lag profiling sweep over the same catalog:
[`khala-code-lag-profiling-sweep.md`](./khala-code-lag-profiling-sweep.md).
The sweep consumes Mode P, Mode D, and packaged `qaMetrics` snapshots, ranks
p95 offenders across every budget, and can env-arm child issue creation from
the generated sample evidence.

## Verification

Focused checks:

```bash
bun test clients/khala-code-desktop/tests/qa-metrics.test.ts packages/khala-qa-harness/src/scenario-runner.test.ts scripts/qa-nightly-matrix.test.ts
```

Pinned issue verify:

```bash
bun run --cwd clients/khala-code-desktop test
```
