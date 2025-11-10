# Issue #003: Implement EmbeddingService Actor

**Component:** `component:embeddings`
**Priority:** `priority:p0`
**Status:** `status:proposed`
**Effort:** 2-3 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** #001, #002

## Description

Implement the main `EmbeddingService` actor that coordinates between the `EmbeddingProvider` (MLX) and `VectorStore` (SQLite). This service provides high-level APIs for generating embeddings, storing them, and performing semantic search.

## Goals

1. Coordinate provider and storage
2. Provide high-level APIs for embed + store operations
3. Support batch operations for efficiency
4. Handle provider swapping (future: multiple providers)
5. Track performance metrics

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── Embeddings/
    └── EmbeddingService.swift           # NEW: Main service actor
```

### Service API

```swift
public actor EmbeddingService {
    private let db: TinyvexDbLayer
    private var provider: EmbeddingProvider
    private var vectorStore: VectorStore?
    private let config: EmbeddingConfig

    // Initialization
    public init(db: TinyvexDbLayer, config: EmbeddingConfig = .default) async throws

    // Embedding generation
    public func generateEmbeddings(_ request: EmbedRequest) async throws -> EmbedResponse

    // Storage + generation
    public func storeEmbedding(
        id: String,
        collection: String,
        text: String,
        metadata: [String: String]? = nil
    ) async throws

    // Batch storage
    public func storeBatch(
        items: [(id: String, text: String, metadata: [String: String]?)],
        collection: String
    ) async throws

    // Search
    public func semanticSearch(_ request: SemanticSearchRequest) async throws -> SemanticSearchResponse
}
```

## Acceptance Criteria

- [ ] `EmbeddingService` actor implemented with proper isolation
- [ ] Initializes provider and vector store on startup
- [ ] `generateEmbeddings()` delegates to provider
- [ ] `storeEmbedding()` generates embedding + stores in vector store
- [ ] `storeBatch()` batch-generates embeddings and stores efficiently
- [ ] `semanticSearch()` embeds query + searches vector store
- [ ] Tracks `processingTimeMs` for all operations
- [ ] Error handling for provider/storage failures
- [ ] Integration tests:
  - `testServiceInitialization()`
  - `testEndToEndStorageAndSearch()`
  - `testBatchOperations()`
  - `testErrorHandling()`
- [ ] Performance: <50ms for single embed, <2s for batch of 100

## Dependencies

- Issue #001 (MLXEmbeddingProvider)
- Issue #002 (VectorStore)

## References

- [Embeddings Plan](../../../plans/embeddings-implementation-plan.md) § 4.6
- [SearchKit Spec](../../spec-v0.2.2.md) § 8 (Search Primitives)

## Example Usage

```swift
let service = try await EmbeddingService(db: tinyvex)

// Generate embeddings only
let request = EmbedRequest(texts: ["Hello world", "Goodbye world"])
let response = try await service.generateEmbeddings(request)
print("Generated \(response.embeddings.count) embeddings in \(response.processingTimeMs)ms")

// Store with automatic embedding
try await service.storeEmbedding(
    id: "doc1",
    collection: "docs",
    text: "This is a test document",
    metadata: ["type": "test"]
)

// Search
let searchRequest = SemanticSearchRequest(
    query: "test document",
    collection: "docs",
    limit: 5
)
let searchResponse = try await service.semanticSearch(searchRequest)
for result in searchResponse.results {
    print("\(result.id): \(result.score)")
}
```

## Error Handling

```swift
public enum EmbeddingError: Error {
    case modelNotLoaded
    case modelUnavailable(reason: String)
    case storageError(underlying: Error)
    case invalidDimensions(expected: Int, got: Int)
}
```

## Performance Targets

- Single embedding: <50ms p50, <100ms p95
- Batch 100 embeddings: <2s
- Semantic search (1k vectors): <200ms

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/EmbeddingServiceIntegrationTests
```

## Notes

- Service is macOS-only (`#if os(macOS)`)
- Provider is pluggable (future: support NaturalLanguage, Cloud APIs)
- All operations are async and properly isolated via actor
- Service handles provider warmup on first use

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Integration tests passing
- [ ] Performance targets met
- [ ] Documentation complete
- [ ] Merged to main branch
