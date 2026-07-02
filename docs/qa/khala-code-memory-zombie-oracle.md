# Khala Code Memory And Zombie Oracle

Date: 2026-07-02
Status: Q2.4 memory and shutdown oracle implemented.

The memory and zombie-process oracle lives in
`packages/khala-qa-harness/src/memory-oracle.ts`. It is wired into the seeded
monkey night and the scenario runner shutdown boundary.

## Budgets

| Budget | Threshold | Scope |
| --- | ---: | --- |
| `memory.rss_after_monkey_night.v1` | 1,500,000,000 bytes | RSS after a seeded monkey night |
| `memory.js_heap_after_monkey_night.v1` | 536,870,912 bytes | JS heap used after a seeded monkey night |
| `memory.rss_monotonic_growth_after_monkey_night.v1` | no strict monotonic increase of 64 MiB or more | RSS after-run samples across the monkey night |
| `process.orphan_after_driver_shutdown.v1` | 0 orphan processes | Every QA driver `shutdown()` |

## Monkey Night

`packages/khala-qa-harness/src/monkey-night.ts` samples process memory before
and after each seeded monkey run, then once more after the whole night. It
writes:

- `monkey-night-report.json`
- `monkey-night-coverage-ledger.json`
- `monkey-night-memory-oracle.json`

The monkey-night CLI exits nonzero when the memory oracle fails or any
scenario shutdown reports an orphan process. This makes the existing nightly
step fail without needing a separate command.

## Shutdown Oracle

`runKhalaCodeQaScenario(...)` now evaluates driver shutdown as part of the
scenario verdict. A driver may return a `shutdownOracle` in its artifacts. If
it does not, the runner treats the shutdown as a pass with zero orphans. If
shutdown fails or reports any orphan processes, the scenario status becomes
`fail`, and run-pass commitments are refuted.

## Nightly Wiring

`scripts/qa-nightly-matrix.ts` expects the memory oracle artifact from the
monkey-night step. It also reads `monkey-night-report.json`; if the report,
memory oracle, or shutdown oracle status is `fail`, the `monkey-night` step is
marked failed even when a command runner exits 0.

## Verification

Focused checks:

```bash
bun test packages/khala-qa-harness/src/memory-oracle.test.ts packages/khala-qa-harness/src/scenario-runner.test.ts scripts/qa-nightly-matrix.test.ts
```

Pinned issue verify:

```bash
bun run --cwd packages/khala-qa-harness monkey:smoke
```
