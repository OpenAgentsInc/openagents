# OpenAgents Codebase Audit — 2025-11-10 (Post‑Fix)

This audit updates the 2025‑11‑09 findings after implementing the recommended actions. The focus is on orchestration scheduling, export/testability, and making environment‑dependent tests less brittle.

## Executive Summary

Major recommendations are now addressed:
- Orchestration: real programmatic control and a background scheduler service (with status/reload) are in place.
- Testing: new unit tests for orchestration harness and Tinyvex title flows; export helpers tested.
- CI: macOS build/test workflow added.
- Env‑dependent tests: Claude CLI test soft‑skips when the environment can’t run it.

Remaining work is largely structural (module boundaries and logging consistency) and can be iterated without blocking feature delivery.

## Implemented Items

- SchedulerService actor
  - Background loop computes next wake using `SchedulePreview.nextRuns`, sleeps, and triggers runs via the same local path as manual `run_now`.
  - Exposed through RPCs:
    - `orchestrate/scheduler.reload`: starts (or restarts) the scheduler with the active config.
    - `orchestrate/scheduler.status`: now reports `running` and `next_wake_time` along with a human‑readable schedule.

- Orchestration RPCs and local adapter
  - `LocalJsonRpcClient` supports `orchestrate/config.set`, `orchestrate/config.activate`, `orchestrate/scheduler.status`, `orchestrate/scheduler.run_now`, and `tinyvex/history.clearSessionTitle`.

- Test coverage improvements
  - `OrchestrationSchedulerTests`: set → activate → status → run_now, validates ACP `session/update` via Combine publisher.
  - `TinyvexTitleTests`: set/get/clear persistence path.
  - `ExportFormattingTests`: exercises transcript export JSON/Markdown via new shared helper.

- CI
  - GitHub Actions workflow to build and run macOS tests on push/PR.

- UX/Tooling
  - Export logic extracted to `TranscriptExport`; both Chat and Inspector can use the same helper.
  - Claude CLI discovery test soft‑skips on non‑runnable environments.

## Remaining Recommendations

- Modularization (server/UI)
  - Continue splitting `DesktopWebSocketServer` concerns into smaller services (Session, Threads, History, Orchestration) with focused unit tests.
  - Extract orchestration summary construction into a shared utility (partial duplication still exists; not blocking).

- Logging
  - Replace remaining `print` with `OpenAgentsLog` and ensure consistent privacy annotations and DEBUG gating.

- Docs
  - Keep `docs/ios-bridge` and orchestration docs aligned with current RPC surface and local adapter patterns.

## Status

- Build: green locally; CI workflow added.
- Tests: orchestration harness/exports/titles covered; environment‑dependent tests won’t fail healthy builds.
- Orchestration: both on‑demand and scheduled runs are available and observable via ACP.

