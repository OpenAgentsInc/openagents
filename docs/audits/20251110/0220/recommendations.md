# Recommendations (Prioritized)

P0 — Expose coordinator‑backed orchestration
- Add JSON‑RPC: `orchestrate/coordinator.run_once`, `orchestrate/coordinator.status`, `orchestrate/scheduler.bind`.
- Wire `scheduler.reload/run_now` to coordinator not ExploreOrchestrator.
- Ensure ACP streaming uses the same update path (`SessionUpdateHub`).

P0 — Document and consolidate ACP content mapping
- Create a `Translators/SessionUpdateMapping.swift` (or similar) that converts from wire types to UI `ACPContentPart`/`ACPTool*` consistently.
- Add a short doc explaining the mapping to avoid future drift.

P1 — Split and unit‑test Desktop server surfaces
- Keep extracting `DesktopWebSocketServer` into focused `+*` files and add unit tests for each JSON‑RPC method family.

P1 — Scheduler reload on config save/activate
- Call `scheduler.bind` (new) after `config.activate` and when Setup completes.

P2 — Align legacy tests with current ACP types
- Either provide thin compatibility types for `ToolUse`/`TextBlock` (decode‑only) or update tests to assert the now‑canonical `tool_call`/`tool_call_update` path.

P2 — Prune stale demo views
- Remove or hard‑gate `Simplified*` views if no longer needed.

