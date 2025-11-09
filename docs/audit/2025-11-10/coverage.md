# Test Coverage — Baseline & Plan (2025-11-10)

## Baseline (observed)
- Core unit/integration tests exist for ACP renderers, message classification, and bridge integration.
- New tests:
  - OrchestrationSchedulerTests (programmatic orchestration control via local RPC).
  - TinyvexTitleTests (set/get/clear title via local RPC).
  - ExportFormattingTests (JSON/Markdown transcript export helper).
- Environment-dependent tests (Claude CLI) can soft-skip when unrunnable.

## Gaps
- Router/handlers: limited explicit tests for JSON-RPC error paths and parameter validation.
- HistoryApi edges: empty DB, non-existent session ids, large timelines (performance characteristics).
- BridgeManager (mac): orchestration flows (startNewSession → status → run_now), title sync path.
- UI command wiring (mac): export/copy actions via focused scene values; sidebar selection and delete.

## Targets
- New/changed code: ≥ 80% line coverage.
- Core subsystems (routing/history/orchestration): ≥ 85%.
- UI commands: smoke tests (behavioral assertions) where feasible.

## Plan
1) Router/Handlers (1–2d)
   - Add unit tests for missing/invalid params, DB-unavailable errors.
   - Validate JSON-RPC error codes/messages.
2) HistoryApi (1–2d)
   - Add tests for empty DB, invalid ids, very large timelines (synthetic data).
3) BridgeManager (mac) (1–2d)
   - Tests for new session, set mode, prompt dispatch; validate notifications reach TimelineStore.
4) UI commands (1–2d)
   - Add smoke tests for export and sidebar interactions with a simplified view harness.
5) Orchestration (continuous)
   - Add tests around SchedulerService nextWake calculations and reload lifecycle.

## Tooling
- Add code coverage reporting in CI (xcodebuild with coverage + summary upload) — optional but recommended.

