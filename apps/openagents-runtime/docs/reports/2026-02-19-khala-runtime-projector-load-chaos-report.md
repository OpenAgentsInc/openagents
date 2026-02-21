# Khala Runtime + Projector Load/Chaos Report

Date: 2026-02-19  
Scope: Gate G7 hardening for runtime + Khala projector path (`#1765`)

## Pass/Fail Criteria

1. Runtime write path remains available under Khala sink failures.
2. Projection checkpoints converge back to runtime sequence truth after replay.
3. Load scenarios keep projection lag bounded and deterministic in local validation.
4. Monitoring assets include actionable alert thresholds and dashboard panels for lag/drift/error/replay signals.

## Validation Commands and Outcomes

1. `cd apps/runtime && mix test test/openagents_runtime/load/runtime_shape_load_test.exs`
- Result: PASS
- Runtime: 4 tests, 0 failures, finished in 4.0s.

2. `cd apps/runtime && mix test test/openagents_runtime/load/khala_projection_load_chaos_test.exs`
- Result: PASS
- Runtime: 3 tests, 0 failures, finished in 1.6s.
- Scenario evidence:
  - run burst: 180 runtime events + terminal event with `lag_events` max <= 1.
  - worker burst: 140 heartbeats + lifecycle events with checkpoint convergence to worker `latest_seq`.
  - chaos: 61 run events persisted while sink returned errors; replay restored checkpoint to latest runtime seq.

3. `cd apps/runtime && mix test test/openagents_runtime/ops/monitoring_assets_test.exs`
- Result: PASS
- Runtime: 2 tests, 0 failures, finished in 0.07s.

4. `cd apps/runtime && mix ci`
- Result: PASS
- Runtime: contract check passed; 314 tests, 0 failures, finished in 37.9s.

## Thresholds Applied

- `OpenAgentsRuntimeKhalaProjectionLagP95High`: p95 lag > 25 events for 10m.
- `OpenAgentsRuntimeKhalaProjectionWriteFailureRatioHigh`: write failures > 1% for 10m.
- `OpenAgentsRuntimeKhalaProjectionDriftIncidentsHigh`: > 3 drift incidents over 10m.
- `OpenAgentsRuntimeKhalaProjectionReplayFailures`: any replay error over 15m.

## Decision

- Status: PASS (local pre-prod hardening gate evidence complete for load/chaos + monitoring assets).
- Remaining Gate G7 work: production-like rollback drill, staged cohort rollout, and on-call handoff execution.
