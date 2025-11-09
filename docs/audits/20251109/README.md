# OpenAgents Codebase Audit — 2025-11-09

This audit focuses on architectural risks, code smells, stubs/placeholders, and test coverage improvements across iOS/macOS app and OpenAgentsCore. It also evaluates orchestration programmability and adds a minimal programmatic test harness for orchestration runs.

See also:
- Detailed findings: findings.md

## Executive Summary

The Swift-only architecture and ACP-first transport are solid. Recent macOS chat refactor (NavigationSplitView root, OATheme black) and Tinyvex-as-source-of-truth improved cohesion. However, several oversized units, residual stubs, and environment-sensitive tests reduce confidence. Logging is inconsistent. Orchestration scheduling previously had status/reload stubs only; this audit introduces concrete RPCs and a local test harness to programmatically trigger runs, closing a major testability gap.

Top risks (with priority):

1) Oversized server/UI modules (P0)
   - DesktopWebSocketServer(+extensions) and ExploreOrchestrator remain large, increasing review and change risk. Split handlers and orchestration concerns into smaller units with focused tests.

2) Stubs and partially implemented flows (P0)
   - Scheduler was stub-only (reload/status). We added run_now + status w/ active config cache and a local adapter; full background scheduler still missing.

3) Environment-coupled tests (P0→P1)
   - External CLI tests (Claude) fail without local env. This masks regressions elsewhere. Gate or mark as optional.

4) Logging inconsistency (P1)
   - Mixed print/os.Logger patterns; privacy levels inconsistent. Centralize and gate verbose logs.

5) Docs drift risk (P1)
   - Bridge and orchestration docs can lag code changes. Keep ADR/Docs aligned (e.g., scheduler capabilities).

## What Changed Since Last Audit (This PR set)

- Programmatic orchestration control (macOS, local adapter):
  - RPCs: `orchestrate/scheduler.run_now`, `orchestrate/scheduler.advance` (alias), enhanced `orchestrate/scheduler.status` with next_wake_time.
  - Server: cache `activeOrchestrationConfig` on `config.activate`; compute status from `SchedulePreview`.
  - Local helpers: `localConfigSet`, `localConfigActivate`, `localSchedulerStatus`, `localSchedulerRunNow` (emits immediate ACP update for observability).
  - LocalJsonRpcClient: implements config.set/activate + scheduler.status/run_now using server local helpers (no socket).
  - Tests: `OrchestrationSchedulerTests` validates set → activate → status → run_now → receives `session/update`.
- Tinyvex titles: add clear title (DB + RPC + UI) and Issue #29 doc.

## Quick Wins (1–2 days)

- Split server concerns into subtypes (Router, History, Orchestration, Sessions) and move long handler bodies out of files with 500+ LOC blocks.
- Add CI (GitHub Actions) to build iOS/macOS, run tests, and report failures distinctly (mark env-dependent tests as optional).
- Centralize logging via `OpenAgentsLog` everywhere; replace stray prints.
- Add unit tests for Tinyvex title clear/get/set and export transcript helpers (JSON/Markdown) with fixtures.

## Medium-Term (1–3 weeks)

- Implement `SchedulerService` actor with injectable clock:
  - Reads `activeOrchestrationConfig`, computes next wake with `SchedulePreview`, sleeps, triggers `run_now` pathway.
  - Test with a mock clock to deterministically “advance time” in unit tests.
- Extract orchestration summary/section building into a shared helper to avoid duplication between local and server paths.
- Reduce UI view responsibilities by moving non-UI transforms to view models or `OpenAgentsCore`.

## Longer-Term (1–2 months)

- Bridge pairing/auth (token or QR) and/or clear Tailscale-only guidance.
- Broaden integration tests for end-to-end orchestration plans (when Foundation Models are available in CI) and history queries at scale.
- Logging schema + sampling for chat/agent updates; richer diagnostics in DeveloperView.

## Test Coverage Improvements (target +20% for touched areas)

- Orchestration
  - [x] Programmatic run (status/run_now): new tests in `OpenAgentsTests/OrchestrationSchedulerTests.swift`.
  - [ ] `SchedulerService` loop with mocked clock (future work).
  - [ ] History APIs: recentSessions and sessionTimeline edge cases (empty, large, bad ids).
- Tinyvex Title Persistence
  - [x] Clear title RPC + UI path landed; add a core unit test for set/get/clear round-trip.
- Export/Import
  - [ ] JSON/Markdown export functions (content and ordering); optional import validation for future feature.
- App commands / menus
  - [ ] Focused scene values wired actions (export/copy) smoke tests if feasible.
- External CLI
  - [ ] Mark environment-coupled tests optional (or guard with availability) so CI doesn’t redline the suite by default.

## How to Run Orchestration Programmatically (macOS)

- Build tests: `cd ios && xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -sdk macosx -configuration Debug test`
- Run only orchestration test: `xcodebuild -project OpenAgents.xcodeproj -scheme OpenAgents -sdk macosx -configuration Debug -only-testing:OpenAgentsTests/OrchestrationSchedulerTests test`
- Live (within app): use `LocalJsonRpcClient` to call `orchestrate/config.set` → `orchestrate/config.activate` → `orchestrate/scheduler.status` → `orchestrate/scheduler.run_now`; subscribe to `DesktopWebSocketServer.notificationPublisher` for `session/update`.

## Acceptance Checks

- macOS build green (app + tests) without requiring external CLIs.
- Orchestration run_now produces at least one ACP `session/update` (heartbeat) and persists to Tinyvex.
- Status returns active_config_id and a plausible next_wake_time from the schedule.
- No new stubs without tracking issues.

