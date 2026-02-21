# Runtime/Khala Restart-Reconnect Chaos Drill Report

- Timestamp: 20260221T132815Z
- Overall status: passed
- Case pass/fail: 3 passed / 0 failed

## Case Results

| Case | Status | Duration (s) | Contract outcome |
| --- | --- | ---: | --- |
| `runtime_executor_restart_recovery` | passed | 2 | Janitor recovery resumed execution and cursor resume replay stayed gap-free. |
| `khala_reconnect_replay` | passed | 1 | Forced reconnect replay emitted only missing events; stale cursor contract remained deterministic. |
| `sync_token_expiry_guard` | passed | 1 | Expired token reconnect was rejected; fresh token reconnect resumed replay correctly. |

## Commands

- `runtime_executor_restart_recovery`: `mix test test/openagents_runtime/load/runtime_shape_load_test.exs --only chaos_drill `
- `khala_reconnect_replay`: `mix test test/openagents_runtime_web/channels/sync_channel_test.exs --only chaos_drill `
- `sync_token_expiry_guard`: `mix test test/openagents_runtime/sync/jwt_verifier_test.exs --only chaos_drill `

## Drill Notes

- Raw harness logs were captured under `/tmp/openagents-runtime-chaos-20260221T132815Z/logs/` for this rehearsal run.
- Follow-up action: rerun `apps/runtime/scripts/run-restart-reconnect-chaos-drills.sh` before each runtime/Khala promotion and attach the new summary artifact.
