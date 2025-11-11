# Detailed Findings — 2025-11-10

## Architecture & Modularity
- DesktopWebSocketServer: large surface area across multiple extensions (sessions, threads, filesystem, terminal, orchestration). While split by files, handlers still hold substantial logic.
  - Recommendation: Extract services (SessionService, HistoryService, OrchestrationService) with narrow APIs; adapt router registrations to delegate. Add unit tests per service.
- Orchestration: ExploreOrchestrator remains large; summary/ACP emission can be extracted into a helper for reuse and easier testing.
- UI: SwiftUI composition is strong. Keep non-UI transforms (export formatting, timeline transforms) out of views — export code already extracted.

## Smells & Stubs
- Stubs: Scheduler previously stubbed; now implemented with SchedulerService and RPCs. Track future expansion (e.g., better policies, backoff, budgets) as non-critical enhancements.
- Placeholders: Ensure any right-inspector or developer-only toggles are behind clear flags and not dead code.

## Concurrency
- SessionUpdateHub actor boundary is a strong pattern. Maintain actor isolation for DB/streaming state. New SchedulerService is an actor as well — good.
- Continue to avoid mixing DispatchQueues with actor-based modules.

## Logging & Observability
- OpenAgentsLog centralization is used in core; replace any stray prints in non-test code. Consider adding structured metadata (plan_id/session_id) on orchestration logs for correlation.

## Bridge Protocol & Security
- JSON-RPC handshake via initialize is correct. Extension capabilities documented.
- Security: LAN-only, no auth — acceptable for local-dev; consider pairing/token or documented Tailscale usage before broader distribution.

## Database & Persistence
- Tinyvex: conversation_titles set/get/clear wired and tested. History API usage robust; add tests for large timelines and invalid ids.
- Consider pagination helpers when timelines get large (non-blocking).

## Tests — Gaps & Opportunities
- Router/handlers: add tests for error paths (missing params, DB unavailable). Validate JSON-RPC error codes.
- History: tests for recentSessions and sessionTimeline edges (empty, bad session id, very large timelines).
- BridgeManager (mac): tests for new session, set mode, prompt dispatch, title sync.
- UI/Commands: smoke tests for export actions (Focus scene value wiring), sidebar selection, delete flow (confirmation logic can remain UI-only; assert state changes/method calls under a test harness).
- Export: snapshot-like markdown checks for several mixed update sequences (user, assistant, plan, tool calls).

## Docs
- Keep docs/ios-bridge, orchestration docs, and ADRs in sync with current RPCs (scheduler.run_now/status/reload and local adapter pattern).

