# Khala Code Lag Profiling Sweep

Date: 2026-07-02
Status: Q2.3 repeatable sweep harness implemented.

The sweep harness lives in
`packages/khala-qa-harness/src/lag-profiling-sweep.ts`. It consumes
`qaMetrics` snapshots from instrumented Khala Code sessions, computes a p95
row for every Q2 latency budget, ranks p95 offenders by threshold ratio, and
writes both a run report and child-issue body files with sample evidence.

## Inputs

Real runs should export either a raw `qaMetrics` snapshot or this wrapped
record:

```json
{
  "label": "mode-p-long-transcript",
  "mode": "mode_p_preview_bridge",
  "workload": ["long_transcript", "thread_switch", "streaming_turn"],
  "snapshot": {
    "ok": true,
    "schema": "openagents.khala_code.qa_metrics.v1",
    "observedAt": "2026-07-02T00:00:00.000Z",
    "definitions": [],
    "budgets": [],
    "evaluations": [],
    "samples": []
  }
}
```

Supported modes are `mode_p_preview_bridge`, `mode_d_built_webview`,
`packaged_electrobun`, and `fixture`. The fixture mode is for deterministic
unit tests and local smoke checks only; optimization child issues should be
filed from real Mode P, Mode D, or packaged snapshots.

## Running

Fixture smoke:

```bash
bun run --cwd packages/khala-qa-harness lag:sweep
```

Real sweep after exporting snapshots:

```bash
bun run --cwd packages/khala-qa-harness lag:sweep -- \
  --snapshot artifacts/qa/mode-p-qaMetrics.json \
  --snapshot artifacts/qa/mode-d-qaMetrics.json \
  --snapshot artifacts/qa/packaged-qaMetrics.json \
  --out-dir artifacts/qa/lag-profiling-sweep
```

The harness writes:

- `lag-profiling-sweep-report.json`
- `lag-profiling-sweep-report.md`
- `offender-issues/*.md` when any p95 exceeds its budget

Issue filing is explicit and env-armed:

```bash
OA_QA_LAG_SWEEP_FILE_ISSUES=1 \
bun run --cwd packages/khala-qa-harness lag:sweep -- \
  --snapshot artifacts/qa/mode-p-qaMetrics.json \
  --snapshot artifacts/qa/mode-d-qaMetrics.json \
  --snapshot artifacts/qa/packaged-qaMetrics.json \
  --file-issues
```

That command shells out to `gh issue create` once per offender, labels each
child issue `qa` and `roadmap`, and includes the top sample evidence table in
the issue body.

## Report Contract

The JSON report schema is `openagents.khala_code.lag_profiling_sweep.v1`.
Every known budget appears in `budgets`, including `no_samples` rows, so a
run can distinguish "budget passed" from "workload did not collect evidence".
Rows with `status: "offender"` are copied into `offenders` in descending
p95/threshold ratio order.

## Verification

Focused checks:

```bash
bun test packages/khala-qa-harness/src/lag-profiling-sweep.test.ts
```

Pinned issue verify:

```bash
bun run --cwd packages/khala-qa-harness test
```
