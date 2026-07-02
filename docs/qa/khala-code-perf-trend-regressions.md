# Khala Code Perf Trend Regressions

Date: 2026-07-02
Status: ROADMAP_QA Q2.5 / issue #8021 implemented.

`bun run qa:nightly` persists per-budget latency trend data in the public-safe
nightly report and exposes it through the Q1.5 status surface.

## Persisted Trend Series

Each run scans its current artifact directory for `qaMetrics` snapshots and
writes a `latencyBudgetRun` object inside `qa-nightly-report.json`.

For every budget in
`clients/khala-code-desktop/src/shared/qa-metrics.ts`, the run records:

- budget ID, metric, unit, threshold, and percentile
- actual value, evaluation status, and sample count
- up to five highest matching sample evidence rows
- repo-relative source snapshot refs

The trend series persists across nights because
`scripts/qa-nightly-matrix.ts` reads prior public `qa-nightly-report.json`
files from the artifact root and compares the current `latencyBudgetRun` with
the latest previous actual value for the same budget.

## Status Surface

`qa-status-surface.json` and `qa-status-surface.md` include:

- `latencyBudgets.status: "trend_series_active"`
- `latencyBudgets.trends[]` with latest value, previous value, delta,
  classification, sample count, and sample evidence
- `latencyBudgets.regressionCount`
- `issueStatuses.latencyBudgetRegression`

Trend classifications are `no_samples`, `first_sample`, `flat`, `improved`,
and `regressed`. Sampleless rows stay explicit instead of inventing a value.

## Regression Issue Filing

Issue creation is env-armed on the owned runner:

```sh
OA_QA_NIGHTLY_FILE_PERF_ISSUE=1 bun run qa:nightly
```

When any budget regresses against the latest previous persisted sample, the
runner writes `qa-nightly-latency-budget-regression-issue.md` in the current
artifact directory and files a strict-form issue through `gh issue create`.

The issue body contains only public-safe evidence:

- budget IDs and metric names
- previous value, latest value, delta, threshold, and unit
- timestamps from the `qaMetrics` samples
- repo-relative report, status-surface, and source snapshot refs

It does not include raw logs, local absolute paths, account identifiers,
credentials, private prompts, or provider payloads.

## Verification

Focused Q2.5 check:

```bash
bun test scripts/qa-nightly-matrix.test.ts
```

Pinned issue verify:

```bash
bun run --cwd packages/khala-qa-harness test
```
