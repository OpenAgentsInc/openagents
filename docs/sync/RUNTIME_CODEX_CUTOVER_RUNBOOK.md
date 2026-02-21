# Khala Runtime/Codex Cutover Runbook

Owner: Runtime lane (`apps/runtime`)  
Scope: Runtime/Codex summary sync cutover from legacy reactive lanes to Khala WS lanes.

This runbook defines rollout stages, gates, rollback, and drill requirements for KHALA-022.

## 1. Preconditions

1. Dual-path publish/validation is active for runtime/Codex summary projections.
2. Parity auditor is enabled in target environment (`khala_sync_parity_enabled=true`).
3. Parity dashboard is available (`docs/sync/PARITY_DASHBOARD.md`).
4. Client feature flags are independently switchable:
   - Web: `VITE_KHALA_SYNC_ENABLED`
   - Mobile: `EXPO_PUBLIC_KHALA_SYNC_ENABLED`
   - Desktop: `OA_DESKTOP_KHALA_SYNC_ENABLED`

## 2. Rollout Stages

1. Stage A (internal only)
   - Enable Khala flags for internal operators.
   - Keep legacy lane available for all external users.
2. Stage B (small cohort)
   - Enable Khala for a bounded cohort (recommended 5-10%).
   - Watch mismatch and catch-up metrics continuously.
3. Stage C (broad cohort)
   - Expand to 25-50% after SLO gates remain green for one business day.
4. Stage D (full lane + rollback window)
   - Move all runtime/Codex client surfaces to Khala flags on.
   - Keep rollback switches available for defined rollback window.

## 3. SLO Gates

All gates must be green before moving to the next stage.

1. Parity mismatch rate
   - `openagents_runtime.sync.parity.cycle.mismatch_rate < 0.01` (1h rolling)
2. Missing-document mismatch trend
   - `reason_class="khala_missing"` trends to zero and remains stable
3. Lag drift
   - `openagents_runtime.sync.parity.entity.abs_lag_drift` p95 near `0` in steady state
4. Replay catch-up
   - `openagents_runtime.sync.replay.catchup_duration_ms` p95 within lane target
5. Stale cursor rate
   - stale cursor errors below incident threshold for active cohort

## 4. Rollback Switches

Use the smallest rollback scope first.

1. Client rollback (preferred first step)
   - `VITE_KHALA_SYNC_ENABLED=false`
   - `EXPO_PUBLIC_KHALA_SYNC_ENABLED=false`
   - `OA_DESKTOP_KHALA_SYNC_ENABLED=false`
2. Runtime rollback
   - Reconfigure runtime to legacy-only sink path.
   - Keep parity auditor on until drift root cause is understood.
3. Validation after rollback
   - Confirm client fallback behavior is healthy.
   - Confirm error rates recover and mismatch alerts stop growing.

## 5. Staging Drill Checklist

1. Enable parity auditor in staging and verify metrics emit.
2. Enable web Khala flag and verify live worker summary updates.
3. Enable mobile Khala flag and verify worker summary updates.
4. Enable desktop Khala flag and verify status lane updates.
5. Exercise rollback toggles and confirm fallback behavior.
6. Re-enable flags and confirm recovery.
7. Capture artifacts:
   - dashboard screenshots,
   - command/test logs,
   - commit and issue references.

## 6. Drill Artifacts

Required output per drill:

1. Drill report in `docs/sync/status/`.
2. Explicit pass/fail for each checklist item.
3. Metric snapshots for mismatch rate and lag drift.
4. Rollback evidence (before/after states).

Current drill evidence:

- `docs/sync/status/2026-02-20-khala-runtime-codex-staging-drill.md`
