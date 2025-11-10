# Key Findings (TL;DR)

- ACP implementation is solid and test-backed; JSON-RPC transport and `session/update` streaming via `SessionUpdateHub` are correct. Initialize handshake advertises extension capabilities with `orchestrate_explore` gating.
- Orchestration is present and works (Explore, Setup, Config, Scheduler). UI can start exploration and receives ACP updates with tool calls and plan states. Scheduler exists and exposes status/run_now.
- Gap: The JSON-RPC scheduler/run_now path and orchestration entry points currently drive ExploreOrchestrator, not the full AgentCoordinator decision → delegate-to-agent loop. Delegation to Codex/Claude happens elsewhere and isn’t exposed as a cohesive “orchestration run” RPC yet.
- Duplicate ACP content modeling risks drift: `AgentClientProtocol/client.swift` vs `ACP/` content types. SessionUpdate uses `ACPToolCallWire`/`ACPToolCallUpdateWire`, while UI timeline uses `ACPToolCall`/`ACPToolResult` (converted). Recommend unifying or documenting mapping.
- SetupOrchestrator is well factored and conversational, but completion persistence notes a TODO to reload the scheduler. Recommend wiring a scheduler reload on save/activate.
- Desktop server is thoughtfully split, but main server still holds multiple domains. Further extractions are already underway with `+Session`, `+Threads`, `+Orchestration`, etc. Keep going to reduce cognitive load.
- Tests are strong overall. Some older comprehensive tests reference legacy `ACP.Client.ToolUse`/`TextBlock` shapes; ensure alignment with the current Swift ACP modeling or add type aliases to preserve parity.

