# Actions (Concrete Next Steps)

1) Coordinator RPCs (server)
- Implement in `DesktopWebSocketServer+Orchestration.swift`:
  - `orchestrate/coordinator.run_once` → new session, call AgentCoordinator.runCycle(config:), stream updates
  - `orchestrate/coordinator.status` → return metrics from AgentCoordinator
  - `orchestrate/scheduler.bind` → set active config and scheduler trigger → coordinator.run_once

2) Connect scheduler to coordinator
- Update `handleSchedulerReload` and `handleSchedulerRunNow` to use coordinator (not ExploreOrchestrator).

3) UI entry points
- iOS Chat: add a “Run Plan” action calling `coordinator.run_once`.
- macOS Console: bind + run_now using new RPCs; display coordinator status.

4) Mapping consolidation
- Add a translator utility for `SessionUpdate` → UI items (centralize between TimelineViewModel and ToolCall renderers).

5) Tests
- Add E2E tests for `coordinator.run_once` that assert: returned session_id; plan/tool updates; agent mode matches config.
- Unit tests for scheduler bind/reload/status around the new coordinator integration.

