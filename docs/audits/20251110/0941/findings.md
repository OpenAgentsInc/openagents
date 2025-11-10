# Findings — Embeddings + IssueAgent Audit

## Duplication (Plans and Issues)

- Embedding provider/service/storage are specified in both the plan and engine issues:
  - Provider API and MLX choice appear in both the plan and Issue #001.
    - docs/plans/embeddings-implementation-plan.md:318
    - docs/engine/issues/embeddings/001-mlx-embedding-provider.md:1
  - Vector storage schema and actor defined in both plan and Issue #002.
    - docs/plans/embeddings-implementation-plan.md:410
    - docs/engine/issues/embeddings/002-vector-store-sqlite.md:1
  - EmbeddingService contract in both plan and Issue #003.
    - docs/plans/embeddings-implementation-plan.md:606
    - docs/engine/issues/embeddings/003-embedding-service.md:1
- Hybrid Search is specified in both SearchKit engine issues and compute docs:
  - Engine issue (RRF fusion): docs/engine/issues/searchkit/007-hybrid-search-rrf.md:1
  - Compute spec still names “Core ML embeddings” directly:
    - docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:47
- IssueAgent layering and roles are described in both the plan and the engine issues set (#016–#020).
  - High-level flow: docs/plans/issue-agent-architecture.md:145
  - End-to-end orchestration issue: docs/engine/issues/issue-agent/020-end-to-end-orchestration.md:1

## Inconsistencies

- RPC naming style mismatch for embeddings vs the rest of the bridge:
  - Plan uses slash naming for embeddings (e.g., "embedding/generate").
    - docs/plans/embeddings-implementation-plan.md:752
  - Existing code favors dot-names for “search.*”, “index.*”, “orchestrate.*”.
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationPlan.swift:248
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:36
  - Recommendation: adopt dot form for embeddings: `embedding.generate`, `embedding.store`, `embedding.search`, `embedding.store_batch`.

- SearchKit compute doc references a direct “Core ML embeddings” component, which duplicates the EmbeddingService and MLX provider plan.
  - docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:47,83
  - Recommendation: SearchKit should depend on EmbeddingService (provider-agnostic) instead of defining a parallel embedding generator.

- IssueAgent ACP exposure vs orchestration:
  - Plan presents IssueAgent operations as ACP “tools” (tool calls in-session), not bridge-level JSON-RPC.
    - docs/plans/issue-agent-architecture.md:769
  - Existing orchestration RPCs already define coordinator endpoints (`orchestrate/coordinator.run_once`).
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:33
  - Recommendation: keep IssueAgent as ACP tools and orchestrate/coordinator for scheduled/automation entry points. Align tool names and streaming with existing `session/update` patterns.

## Integration Gaps (Code)

- Bridge methods for embeddings are not implemented yet:
  - Missing DesktopWebSocketServer+Embeddings handlers (generate/store/search/store_batch) per plan.
    - docs/plans/embeddings-implementation-plan.md:741
  - No corresponding ACPRPC constants for embeddings.

- Tinyvex DB schema lacks embeddings table and helper methods:
  - The plan’s schema migration isn’t integrated into `TinyvexDbLayer`.
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1
    - Plan schema: docs/plans/embeddings-implementation-plan.md:414

- Service/Store not present:
  - `EmbeddingService.swift` and `VectorStore.swift` are not in the repo yet (provider/types/tests exist).
    - Current code has: EmbeddingProvider.swift, EmbeddingTypes.swift, MLXEmbeddingProvider.swift

- SearchKit should consume EmbeddingService:
  - Engine issues for FTS5/Hybrid search exist but no code yet; ensure SearchKit’s semantic path uses `EmbeddingService.semanticSearch()` and that chunk embeddings are stored via the same `embeddings` table with a stable `collection` convention.

- IssueAgent code surface:
  - Types/actors from plan (IssueEnhancer, RetrievalOrchestrator, AgentDelegator, PRBuilder, IssueAgentService) not yet created; ensure ACP tool definitions and streaming align with existing `session/update` wire types.
    - docs/plans/issue-agent-architecture.md:769

## What’s Already Landed (Commits)

- Embedding core primitives exist and compile:
  - Provider and types:
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/EmbeddingProvider.swift:1
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/EmbeddingTypes.swift:1
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/MLXEmbeddingProvider.swift:1
  - Tests cover types and MLX provider behavior:
    - ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/EmbeddingsTests.swift:1

## High-Value Alignments

- Unify RPC naming: add `embedding.*` to ACPRPC and implement corresponding server handlers.
- Centralize embeddings through `EmbeddingService` so SearchKit and IssueAgent reuse the same provider/storage.
- Extend Tinyvex DB with embeddings schema and helper methods referenced by both SearchKit and IssueAgent.
- Keep IssueAgent as ACP tools (interactive) while using orchestrate/coordinator for scheduled/automation runs; both should stream via `session/update`.

