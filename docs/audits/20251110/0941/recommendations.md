# Recommendations — Priority Order

## P0 — Unblock End-to-End Semantics

- Add ACPRPC constants and Desktop bridge for embeddings (dot naming).
- Add Tinyvex embeddings schema + helpers; implement VectorStore and EmbeddingService.
- Adopt stable collection/ID scheme for files/chunks.

## P1 — Align SearchKit and IssueAgent

- Make SearchKit depend on `EmbeddingService` (remove “Core ML embeddings” duplication in compute doc; keep provider-agnostic).
- Stand up `HybridSearch` and ensure semantic leg calls `EmbeddingService.semanticSearch` to reduce duplication.
- Implement IssueAgent actors with ACP tool exposure; stream via `session/update`.

## P2 — Consistency and UX

- Unify RPC naming to dot form for embeddings; ensure capability gating mirrors orchestrate.explore.*.
- Add local (in-process) embedding calls on macOS similar to existing local orchestration mappings.
- Document collections, metadata keys, and ID formats (files vs chunks) so both SearchKit and IssueAgent share the same storage conventions.

## P3 — Tests and Docs

- Add bridge tests for embeddings RPCs and service integration tests.
- Update compute SearchKit MVP doc to reference EmbeddingService and shared schema.
- Cross-link engine issues to code locations once files are added, to reduce future drift.

