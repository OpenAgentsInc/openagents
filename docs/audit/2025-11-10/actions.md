# Actionable Tasks — 2025-11-10

## Highest Priority (P0)
1) Router/Handlers tests
   - Add tests for missing/invalid params and DB-unavailable conditions on tinyvex/history.* and orchestrate/*.
   - Validate JSON-RPC error codes/messages.
2) Server modularization — HistoryService
   - Extract history/title handlers into HistoryService with unit tests for empty DB, invalid ids, large timelines.
3) BridgeManager (mac) tests
   - Tests for new session → set mode → prompt → observe updates (LocalJsonRpcClient). Title sync tests for persisted titles.

## High Priority (P1)
4) OrchestrationSummaryBuilder
   - Extract summary construction; add unit tests with fixtures.
5) UI command smoke tests
   - Export actions (JSON/Markdown) and sidebar selection/delete via a simplified view harness.
6) Logging sweep
   - Replace any remaining print in non-test code with OpenAgentsLog; DEBUG-gate noisy logs.

## Medium Priority (P2)
7) Docs
   - Keep docs/ios-bridge and orchestration docs current with new scheduler and local adapter flows.
8) Coverage in CI (optional)
   - Enable xcodebuild coverage flags and upload summary.

