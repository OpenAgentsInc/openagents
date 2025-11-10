# Duplication Matrix

## Embeddings

- Provider API and MLX integration
  - Plan: docs/plans/embeddings-implementation-plan.md:318
  - Engine Issue #001: docs/engine/issues/embeddings/001-mlx-embedding-provider.md:1
  - Code: ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/MLXEmbeddingProvider.swift:1

- VectorStore and schema
  - Plan schema/actor: docs/plans/embeddings-implementation-plan.md:410,497
  - Engine Issue #002: docs/engine/issues/embeddings/002-vector-store-sqlite.md:1
  - Code today: Tinyvex DbLayer exists but lacks vector schema
    - ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift:1

- EmbeddingService
  - Plan: docs/plans/embeddings-implementation-plan.md:606
  - Engine Issue #003: docs/engine/issues/embeddings/003-embedding-service.md:1
  - Code: Not yet present

## SearchKit

- Hybrid search (FTS5 + embeddings + RRF)
  - Engine Issue #007: docs/engine/issues/searchkit/007-hybrid-search-rrf.md:1
  - Compute doc (MVP): docs/compute/issues/phase-4-searchkit/024-searchkit-mvp.md:49,57
  - Note: Compute doc mentions “Core ML embeddings,” duplicating embedding concerns; should reuse EmbeddingService.

## IssueAgent

- System architecture, data flow, and actors
  - Plan: docs/plans/issue-agent-architecture.md:145
  - Engine Issues: 016/017/018/020 under docs/engine/issues/issue-agent
  - Code: Not yet present; overlaps are intentional but should reference a single canonical module layout.

