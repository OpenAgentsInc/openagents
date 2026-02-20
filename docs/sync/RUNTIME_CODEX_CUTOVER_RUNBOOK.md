# Khala Runtime/Codex Cutover Runbook

Owner: Runtime lane (`apps/openagents-runtime`)  
Scope: Runtime/Codex summary sync cutover from Convex-client lanes to Khala WS lanes.

This runbook defines rollout stages, gates, rollback, and drill requirements for KHALA-022.

## 1. Preconditions

1. Dual-publish is active for runtime/Codex summaries (Convex + Khala sink path).
2. Parity auditor is enabled in target environment:
   - `khala_sync_parity_enabled=true`
3. Parity dashboard is available:
   - `docs/sync/PARITY_DASHBOARD.md`
4. Client feature flags exist and are independently switchable:
   - Web: `VITE_KHALA_SYNC_ENABLED`
   - Mobile: `EXPO_PUBLIC_KHALA_SYNC_ENABLED`
   - Desktop: `OA_DESKTOP_KHALA_SYNC_ENABLED`

## 2. Rollout Stages

1. Stage A (internal only)
   - Enable Khala flags for internal operators.
   - Keep Convex lane available for all external users.

2. Stage B (small cohort)
   - Enable Khala for a bounded cohort (recommended 5-10%).
   - Monitor parity mismatch and catch-up metrics continuously.

3. Stage C (broad cohort)
   - Expand to 25-50% after SLO gates hold for at least one business day.

4. Stage D (full lane + rollback window)
   - Move all runtime/Codex client surfaces to Khala flags on.
   - Keep rollback switches available for the defined rollback window.

## 3. SLO Gates

All gates must be green before moving to the next stage.

1. Parity mismatch rate:
   - `openagents_runtime.sync.parity.cycle.mismatch_rate < 0.01` (1h rolling)

2. Missing-document mismatches:
   - `reason_class="khala_missing"` trends to zero and stays near zero.

3. Lag drift:
   - `openagents_runtime.sync.parity.entity.abs_lag_drift` p95 at or near `0` in steady state.

4. Replay catch-up:
   - `openagents_runtime.sync.replay.catchup_duration_ms` p95 stays within lane target.

5. Stale cursor rate:
   - Stale cursor errors remain below incident threshold for the cohort.

## 4. Rollback Switches

Use the smallest rollback surface needed first.

1. Client rollback (preferred first step)
   - Disable Khala flags per surface:
     - `VITE_KHALA_SYNC_ENABLED=false`
     - `EXPO_PUBLIC_KHALA_SYNC_ENABLED=false`
     - `OA_DESKTOP_KHALA_SYNC_ENABLED=false`

2. Runtime rollback to Convex-only projection sink
   - Reconfigure runtime sink path to Convex-only (disable Khala fanout target).
   - Keep parity auditor enabled until drift is understood, then disable if required.

3. Rollback validation
   - Confirm client lane fallback behavior (legacy polling/Convex paths).
   - Confirm error rates recover and parity mismatch alerts stop increasing.

## 5. Staging Drill Checklist

1. Enable parity auditor in staging and verify cycle/entity metrics emit.
2. Enable Khala flag on web admin and confirm live worker summary updates.
3. Enable Khala flag on mobile and confirm worker summaries update without Convex provider boot.
4. Enable Khala flag on desktop and confirm sync connectivity lane uses Khala path.
5. Simulate rollback:
   - disable each client flag,
   - verify lane fallback behavior,
   - re-enable and verify recovery.
6. Record artifacts:
   - dashboard screenshots,
   - command logs / test outputs,
   - links to commits and issue references.

## 6. Drill Artifacts

Required output per drill:

1. Completed drill report file under `docs/sync/status/`.
2. Explicit pass/fail for each checklist item.
3. Metric snapshots for mismatch rate and lag drift.
4. Rollback exercise evidence (before/after state).

Current drill evidence:

- `docs/sync/status/2026-02-20-khala-runtime-codex-staging-drill.md`
