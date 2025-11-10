# Actions — Concrete Next Steps

## Short Term (this week)

- RPC constants: add `embedding.*` to `ACPRPC`
  - Edit: ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:1

- Bridge handlers: add `DesktopWebSocketServer+Embeddings.swift`
  - New file under DesktopBridge; register 4 methods and route to service
  - Reference handler shapes: docs/plans/embeddings-implementation-plan.md:741

- DB layer: integrate vector schema + helpers
  - Edit: ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1
  - Add: `ensureVectorSchemaV2()` and call from `migrate()`

- Service/Store: add `VectorStore.swift` and `EmbeddingService.swift`
  - New files under Embeddings/
  - Reuse `EmbeddingTypes.swift` request/response types

## Medium Term

- SearchKit: implement FTS5 and HybridSearch; wire semantic leg to EmbeddingService
  - Create `SearchKit/` modules per engine issues (006, 007)
  - Define chunk-level `collection` and ID strategy

- IssueAgent actors: implement Enhancement, Retrieval, Delegation, PR
  - Expose as ACP tools per plan; stream via `session/update`
  - Align with orchestrate/coordinator for scheduled runs

## Docs and Specs

- Update compute doc wordings to remove direct “Core ML embeddings” component
  - Edit: docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:47
- Add an “Embeddings Storage Conventions” doc (collections, IDs, metadata)

