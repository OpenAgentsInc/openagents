# Code Smells and Risks

- Duplicate ACP content modeling
  - `AgentClientProtocol/client.swift` vs `ACP/ACPContent.swift` (and `ACPTool*` in `ACP/ACPTool.swift`). Wire → UI conversion exists in timeline view model and renderers. Risk of shape drift over time.
  - Action: centralize mapping in a single translator and add a brief doc.

- Desktop server surface breadth
  - `DesktopWebSocketServer` still owns several concerns despite partial extraction (`+Session`, `+Threads`, `+Orchestration`, `+FileSystem`, `+Terminal`). Keep splitting to maintainability units and enhance unit testability per sub-surface.

- TODOs around orchestration persistence
  - `saveCompletedConfig` notes where scheduler reload should occur. Wire this after `config.activate` and when setup completes.

- Conditional blocks and disabled sections
  - Occasional patterns like `if false` or guarded availability blocks that elide entire branches; avoid dead code by factoring into helpers and gating at call sites.

- Potential test drift
  - Older tests reference `ToolUse`/`TextBlock`. Align with current `SessionUpdate`-first tool_call/update design or provide compatibility types.

