# Khala Runtime/Codex Staging Drill Report

Date: 2026-02-20  
Issue: KHALA-022  
Operator lane: Runtime/Codex sync cutover rehearsal

## Scope

Validated staged rollout controls, parity gating, and rollback switches for runtime/Codex Khala cutover using staging-equivalent verification harnesses and surface-specific checks.

## Checklist Results

1. Parity auditor enabled path validated: PASS
   - Added and verified `OpenAgentsRuntime.Sync.ParityAuditor`.
   - Verified parity telemetry metrics compile and emit in tests.

2. Web Khala lane flag behavior: PASS
   - `VITE_KHALA_SYNC_ENABLED` path implemented and linted.
   - Web summary lane fallback behavior preserved when flag is disabled.

3. Mobile Khala lane without Convex boot: PASS
   - `EXPO_PUBLIC_KHALA_SYNC_ENABLED` path implemented.
   - Convex provider boot bypass confirmed in app startup code when flag enabled.

4. Desktop Khala connectivity lane flag behavior: PASS
   - `OA_DESKTOP_KHALA_SYNC_ENABLED` path implemented.
   - Desktop connectivity state now tracks generic sync provider/reachability.

5. Rollback switch rehearsal (code/config level): PASS
   - Runbook rollback toggles documented and validated in code paths:
     - web/mobile/desktop flag-off fallback behavior
     - runtime sink path remains configurable for Convex-only fallback

## Evidence

Runtime parity and sync tests:

- `cd apps/openagents-runtime && mix test test/openagents_runtime/sync/parity_auditor_test.exs test/openagents_runtime/sync/projector_sink_test.exs`
- `cd apps/openagents-runtime && mix test --only sync_parity`
- Runtime full lane checks also passed via commit hook runtime suite.

Mobile lane checks:

- `cd apps/mobile && bun run compile`
- `cd apps/mobile && bun run test`

Desktop lane checks:

- `cd apps/desktop && npm run typecheck`
- `cd apps/desktop && npm test`

Web lane checks:

- `cd apps/openagents.com && npx eslint resources/js/pages/admin/index.tsx`

## Artifacts

1. Runbook:
   - `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
2. Dashboard spec:
   - `docs/sync/PARITY_DASHBOARD.md`
3. Architecture/surface updates:
   - `docs/ARCHITECTURE.md`
   - `docs/sync/SURFACES.md`

## Residual Risks

1. Full production rollout still requires live non-local dashboard confirmation for mismatch and lag thresholds.
2. Remaining Convex-dependent lanes (notably Lightning control-plane) are still outside this drill scope.
