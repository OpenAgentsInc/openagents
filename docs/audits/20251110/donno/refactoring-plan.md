# Refactoring Plan — 2025-11-10

## Goals
- Improve modularity and testability of the desktop server.
- Reduce orchestration surface; extract reusable helpers.
- Keep non-UI transforms out of SwiftUI views.

## Roadmap

### Phase A (Server modularization)
- Extract HistoryService: recentSessions, sessionTimeline, title get/set/clear (delegate to Tinyvex DbLayer). Add unit tests for edge cases.
- Extract SessionService/ThreadsService: session lifecycle and threads listing.
- OrchestrationService: wrap scheduler.reload/status/run_now and caching of active config.

### Phase B (Orchestration extraction)
- Extract OrchestrationSummaryBuilder used by ExploreOrchestrator and any local run_now paths; add tests for stable output given fixtures.

### Phase C (UI separation)
- Keep export formatting, timeline transforms, and title logic out of views (already started with TranscriptExport). Audit other view files and migrate similar logic.

### Phase D (Logging & diagnostics)
- Replace any remaining print with OpenAgentsLog; standardize privacy annotations.
- Add identifiers (session_id/plan_id) where useful for orchestration traces.

## Acceptance
- Unit tests exist for each new service, covering happy and error paths.
- No functionality regressions; builds green on macOS; CI green.

