# Detailed Findings — 2025-11-09

## Code Structure & Size
- `DesktopWebSocketServer` plus its extensions aggregate many responsibilities. While split across files, some handlers remain long. Recommendation: extract an OrchestrationService (status/run_now/activate), HistoryService, and SessionService with unit tests per service.
- `ExploreOrchestrator` is large; consider splitting analysis/tool execution from summarization/ACP emission.

## Stubs & Placeholders
- Scheduler previously stubbed; improved with `run_now` and concrete `status`. Background loop still missing. Track as a follow-up (SchedulerService with mockable clock).
- A few TODO comments around inspector/Export integrations; ensure issues exist in docs/chat-desktop for each.

## Test Coverage
- Added orchestration harness test (`OrchestrationSchedulerTests`).
- Gaps: history edge cases, export/import formatting, Tinyvex title persistence unit tests, and command wiring smoke tests.
- External CLI tests (Claude) fail if tool not installed; guard or mark optional to avoid masking other regressions.

## Logging & Observability
- Mixed `print`/`OpenAgentsLog` usage. Standardize on the latter; add DEBUG gate and privacy annotations.
- Consider structured metadata for orchestration runs (plan_id, session_id) to correlate in logs.

## Bridge Protocol & Security
- JSON-RPC over LAN without auth; pairing/token or Tailscale guidance is still needed before broader release.
- Active orchestration config is now cached server-side; ensure thread safety (actor isolation ok; server is class, but heavy state updates are on actor collaborators like TinyvexDbLayer and SessionUpdateHub).

## Concurrency & State
- SessionUpdateHub actor is a good boundary. Continue moving stateful subsystems behind actors where feasible (e.g., future SchedulerService actor).

## UI Composition (macOS)
- NavigationSplitView root is in place; right inspector optional. Keep non-UI transforms out of SwiftUI views (e.g., export helpers belong in a model/utility file).
- Theme application consistent (OATheme black); keep glass disabled per current direction.

## Database & Persistence
- Tinyvex conversation_titles table used for titles; clear/get/set now consistent. Add tests for corner cases (missing rows; overwrite semantics).
- HistoryApi queries should be tested for large timelines.

## Prioritized Actions
1) Add SchedulerService with test clock and unit tests.
2) Extract orchestration summary builder helper; test generated sections with fixed inputs.
3) Add core unit tests for Tinyvex title set/get/clear; add export markdown/json snapshot tests.
4) Gate/hide external CLI tests by default in CI; document how to enable locally.

