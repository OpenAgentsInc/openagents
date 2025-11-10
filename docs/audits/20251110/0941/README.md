# Audit: Embeddings + IssueAgent Plans (2025-11-10 09:41)

Scope: Reviewed embeddings implementation plan, IssueAgent architecture, engine/compute issues, and recent commits. Identified duplication, inconsistencies, and integration gaps; implemented high‑value fixes.

Key findings
- RPC naming mismatch: plans used `embedding/...`; code and other plans prefer dot naming (e.g., `embedding.generate`).
- Duplication: Provider/storage/service specs repeated across plans and engine docs; SearchKit compute doc redundantly specifies a bespoke embedder instead of using EmbeddingService.
- Integration gaps: Tinyvex lacked embeddings schema; bridge lacked handlers; SearchKit not wired to EmbeddingService.
- GPT‑OSS: Download/UI progress used file counts, causing `0.0/0.0 GB` UX issues; default provider selection not aligned when GPT‑OSS is installed.

Changes implemented
- RPC constants and handlers: Added dot‑named RPCs and bridge handlers for embeddings.
  - Files: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Embeddings.swift
  - RPCs: `embedding.generate`, `embedding.store`, `embedding.search`, `embedding.store_batch` (ACPRPC)
- Tinyvex DB: Added embeddings schema and blob bind helper (SQLITE_TRANSIENT); vector helpers to store/fetch.
  - Files: ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/DbLayer.swift
- VectorStore + Service: Implemented `VectorStore` actor (cosine via Accelerate) and service coordination (EmbeddingService) with batch store and semantic search.
  - Files: ios/OpenAgentsCore/Sources/OpenAgentsCore/Embeddings/
- GPT‑OSS download UX: Byte‑based progress logs; fallback totals when Hub reports file counts; robust pause handling (no false "Ready").
  - Files: ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSModelManager.swift, ios/OpenAgents/Views/macOS/GPTOSS/*
- Default provider: Server now prefers GPT‑OSS 20B for new sessions if installed; `session/set_mode` will transparently upgrade a `default_mode` request to `gptoss_20b` when available.
  - Files: ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift (+Session.swift)

Recommendations (future)
- SearchKit: Depend on EmbeddingService for semantic leg; remove duplicated “Core ML embeddings” block from compute docs.
- ANN path: Keep brute‑force up to ~100k vectors; add optional sqlite‑vec adapter for larger sets.
- GPT‑OSS streaming: Implement token streaming via ACP (`agentMessageChunk`), filter Harmony channels (hide `analysis`).
- Routing: Add `gptoss.generate` FM tool and routing heuristics; prefer GPT‑OSS when installed and request exceeds FM token/complexity thresholds.
- Tests: Round‑trip tests for embeddings RPC; GPT‑OSS template compliance and streaming; UI state tests for pause/resume/persisted installs.

Context links
- Plans: docs/plans/embeddings-implementation-plan.md, docs/plans/issue-agent-architecture.md
- Engine: docs/engine/spec-v0.2.2.md, docs/engine/sqlite-cosine-vector-search.md
- GPT‑OSS: docs/gptoss/research.md, docs/gptoss/issues/00x-*.md

Status
- P0 changes landed; build passes on macOS via xcodebuild.
- Next: streaming (Issue 004), routing (006/007), and SearchKit integration.

