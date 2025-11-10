# ACP & Bridge Alignment

## Naming Conventions

- Prefer dot-separated namespaces for higher-level services and search-like tools:
  - Already present: `search.semantic`, `index.rebuild`, `orchestrate.explore.start`
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationPlan.swift:248
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:36

- For embeddings, use dot naming to avoid mixing styles:
  - Replace plan’s `embedding/generate` with `embedding.generate`
    - docs/plans/embeddings-implementation-plan.md:752
  - Do the same for store, search, store_batch

## ACPRPC Additions

- Add the following constants:
  - `public static let embeddingGenerate = "embedding.generate"`
  - `public static let embeddingStore = "embedding.store"`
  - `public static let embeddingSearch = "embedding.search"`
  - `public static let embeddingStoreBatch = "embedding.store_batch"`
  - File: ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:1

## Bridge Handlers

- Create `DesktopWebSocketServer+Embeddings.swift` and register methods in `registerHandlers()`
  - Follow the request/response types defined in `EmbeddingTypes.swift`
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/EmbeddingTypes.swift:1
  - Bind to `EmbeddingService` actor methods once added

## Capability Gating

- Mirror orchestrate.explore.* gating using advertised extension capabilities
  - If `extCapabilities.embeddings` is false, respond with `-32601` (not supported)

