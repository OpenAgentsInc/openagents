# Runtime/Khala Restart-Reconnect Chaos Drills

This runbook defines the mandatory restart/reconnect rehearsal lane for OA-RUST-070.

## Objective

Continuously verify that runtime and Khala recover without replay gaps under restart/reconnect stress and token churn.

## Scenario Matrix

1. `runtime_executor_restart_recovery`
- Simulates executor-loss / runtime worker restart via janitor recovery.
- Contract: run resumes and stream cursor replay has no gaps.
- Test anchor: `apps/runtime/test/openagents_runtime/load/runtime_shape_load_test.exs` (`@tag :chaos_drill`)

2. `khala_reconnect_replay`
- Simulates forced socket drop and reconnect with replay bootstrap.
- Includes stale cursor detection and reconnect behavior.
- Contract: replay returns only missing updates; stale cursor emits deterministic error contract.
- Test anchor: `apps/runtime/test/openagents_runtime_web/channels/sync_channel_test.exs` (`@tag :chaos_drill`)

3. `sync_token_expiry_guard`
- Simulates reconnect with expired token followed by fresh token.
- Contract: expired token is rejected; fresh token reconnect resumes correctly.
- Test anchors:
  - `apps/runtime/test/openagents_runtime_web/channels/sync_channel_test.exs` (`@tag :chaos_drill`)
  - `apps/runtime/test/openagents_runtime/sync/jwt_verifier_test.exs` (`@tag :chaos_drill`)

## Success Criteria

1. All drill cases pass.
2. No replay gap regression (`topic+seq` monotonic behavior maintained).
3. Expired sync tokens are rejected on reconnect.
4. Drill artifacts are saved and linked in issue/release evidence.

## Command

From repo root:

```bash
apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh
```

Optional fixed report output path:

```bash
REPORT_FILE=apps/runtime/docs/reports/2026-02-21-runtime-khala-restart-reconnect-chaos-report.md \
apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh
```

## Output Artifacts

Per run:

- `apps/runtime/docs/reports/restart-reconnect-chaos/<timestamp>/results.jsonl`
- `apps/runtime/docs/reports/restart-reconnect-chaos/<timestamp>/summary.json`
- `apps/runtime/docs/reports/restart-reconnect-chaos/<timestamp>/SUMMARY.md`
- `apps/runtime/docs/reports/restart-reconnect-chaos/<timestamp>/logs/*.log`

## Operational Usage

1. Run before runtime/Khala release promotion.
2. Attach `SUMMARY.md` and any failing logs to deployment/issue comments.
3. If any case fails, block promotion until remediation and rerun.
