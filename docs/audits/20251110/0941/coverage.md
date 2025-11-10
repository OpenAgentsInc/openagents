# Coverage — Tests to Add/Adjust

## Embeddings

- Service integration tests
  - Generate → dimensions/time; Store → round-trip; Search → top-K
  - Reference: docs/plans/embeddings-implementation-plan.md:606

- Bridge tests
  - JSON-RPC handlers for `embedding.generate`, `embedding.search`, `embedding.store(_batch)`
  - Error cases (service unavailable, bad params)

- Tinyvex DB tests
  - Schema migration; store/fetch; BLOB serialization correctness

## SearchKit

- Hybrid search coordination test hits EmbeddingService for semantic leg
  - Weighted fusion; deduplication; performance envelope

## IssueAgent

- Tool plumbing and streaming
  - Tool call starts; `session/update` progress steps; end results
  - Golden tests for a few real issues

