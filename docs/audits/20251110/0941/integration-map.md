# Integration Map — What to Change Where

## RPC and Bridge (Embeddings)

- Add ACPRPC method constants (dot naming):
  - `embedding.generate`, `embedding.store`, `embedding.search`, `embedding.store_batch`
  - File: ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:1

- Implement Desktop bridge handlers:
  - New: `DesktopWebSocketServer+Embeddings.swift` with `registerEmbeddingMethods()` and handlers
  - Mirror the plan’s handler shapes, but adopt dot naming
    - Reference: docs/plans/embeddings-implementation-plan.md:741
  - Gate via advertised capabilities similar to orchestrate.explore.*

## Database (Tinyvex)

- Add embeddings schema migration and helpers to TinyvexDbLayer:
  - Add `ensureVectorSchemaV2()` and call from `migrate()`
  - Implement `storeEmbedding(...)` and `fetchEmbeddings(collection:)`
  - Reference schema: docs/plans/embeddings-implementation-plan.md:414
  - File to update: ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1

## Embedding Service + Store

- Add `VectorStore.swift` (actor) and `EmbeddingService.swift` (actor) under Embeddings/:
  - Reference: docs/plans/embeddings-implementation-plan.md:497,606
  - Ensure `EmbeddingService.semanticSearch(...)` wraps store.search + provider.embed

## SearchKit Consumption of Embeddings

- Ensure SearchKit’s semantic side calls EmbeddingService, not a bespoke embedder:
  - Update compute doc to reflect EmbeddingService rather than “Core ML embeddings” component
    - docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:47,83
  - When implementing `HybridSearch`, depend on `EmbeddingService` for semantic scores.

## IssueAgent Integration

- Keep IssueAgent as ACP tools (interactive) and leverage orchestrate/coordinator for automation:
  - Tools per plan: docs/plans/issue-agent-architecture.md:776
  - Streaming via `session/update`: docs/plans/issue-agent-architecture.md:791
  - Use SearchKitService (hybrid) which in turn uses EmbeddingService

## Collections and IDs

- Define stable collection names and ID schemes to enable reuse across subsystems:
  - `collection="files"` for whole-file embeddings (or `chunks` for chunk-level indexing)
  - For chunk embeddings, consider IDs like `chunk:<path>#<chunkId>` with metadata containing `path`, `startLine`, `endLine`.

