# Code Updates — Concrete Changes

## 1) Add Embedding RPC constants

- File: ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:1
- Add:
  - `embedding.generate`, `embedding.store`, `embedding.search`, `embedding.store_batch`

## 2) Add Desktop bridge for Embeddings

- New File: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Embeddings.swift
- Register handlers for the four methods; route to `EmbeddingService`
- Adopt dot naming and add capability gating
- Reference handler shapes in plan: docs/plans/embeddings-implementation-plan.md:741

## 3) Extend Tinyvex DbLayer with vector storage

- File: ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1
- Add `ensureVectorSchemaV2()`
- Call it from `migrate()`
- Add helper methods:
  - `storeEmbedding(id:collection:embedding:dimensions:modelID:metadata:text:)`
  - `fetchEmbeddings(collection:) -> [(id:String, embedding:[Float], metadata:[String:String]?)]`
- Reference schema: docs/plans/embeddings-implementation-plan.md:414

## 4) Implement EmbeddingService and VectorStore actors

- New Files:
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/VectorStore.swift
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/EmbeddingService.swift
- Ensure API matches plan
  - Plan: docs/plans/embeddings-implementation-plan.md:497,606

## 5) Update SearchKit design to depend on EmbeddingService

- Update compute spec verbiage to reference `EmbeddingService` instead of “Core ML embeddings” component
  - docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:47,83
- When implemented, `HybridSearch` should call `EmbeddingService` for semantic scores

## 6) IssueAgent ACP tools

- Keep IssueAgent exposed as ACP tools per plan
  - docs/plans/issue-agent-architecture.md:769
- Ensure streaming via `session/update` is consistent with existing types
  - docs/plans/issue-agent-architecture.md:791

