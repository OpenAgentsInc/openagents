# Issue #002: Implement Vector Store with SQLite

**Component:** `component:embeddings`
**Priority:** `priority:p0`
**Status:** `status:proposed`
**Effort:** 3-4 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** #001

## Description

Implement persistent vector storage using SQLite with BLOB columns for Float32 arrays. Extend the existing `TinyvexDbLayer` schema to support embedding storage and brute-force cosine similarity search.

## Goals

1. Extend SQLite schema with `embeddings` table
2. Implement `VectorStore` actor for CRUD operations
3. Support brute-force cosine similarity search
4. Use Accelerate framework (vDSP) for fast dot products
5. Handle multi-collection organization

## Implementation Details

### Files to Create/Modify

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
├── Embeddings/
│   ├── VectorStore.swift                # NEW: Actor for vector operations
│   └── SimilaritySearch.swift           # NEW: Cosine search utilities
└── Tinyvex/
    └── DbLayer.swift                    # MODIFY: Add schema migration
```

### Schema Extension

```sql
CREATE TABLE IF NOT EXISTS embeddings (
    id TEXT NOT NULL,
    collection TEXT NOT NULL,
    embedding_blob BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model_id TEXT NOT NULL,
    metadata_json TEXT NULL,
    text TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (id, collection)
);

CREATE INDEX IF NOT EXISTS idx_embeddings_collection ON embeddings(collection);
CREATE INDEX IF NOT EXISTS idx_embeddings_model ON embeddings(model_id);
CREATE INDEX IF NOT EXISTS idx_embeddings_updated ON embeddings(updated_at);
```

### VectorStore Actor

```swift
public actor VectorStore {
    private let db: TinyvexDbLayer
    private let dimensions: Int
    private let modelID: String

    // Storage
    public func store(
        id: String,
        collection: String,
        embedding: [Float],
        metadata: [String: String]? = nil,
        text: String? = nil
    ) async throws

    // Search
    public func search(
        query: [Float],
        collection: String,
        limit: Int = 10,
        minSimilarity: Float? = nil
    ) async throws -> [SemanticSearchResult]

    // Utilities
    private func cosineSimilarity(_ a: [Float], _ b: [Float]) -> Float
}
```

## Acceptance Criteria

- [ ] Schema migration added to `TinyvexDbLayer.ensureVectorSchemaV2()`
- [ ] `VectorStore` actor implemented with proper isolation
- [ ] Embeddings stored as contiguous Float32 BLOB
- [ ] Metadata stored as JSON string
- [ ] Brute-force search returns top-K by cosine similarity
- [ ] vDSP used for dot products (performance)
- [ ] Dimension validation on store/search
- [ ] Unit tests:
  - `testVectorStorageRoundTrip()`
  - `testCosineSimilarity()`
  - `testSearchTopK()`
  - `testMultiCollection()`
  - `testMetadataFilter()`
- [ ] Performance: search 1,000 vectors in <100ms

## Dependencies

**DB Layer:**
- Existing `TinyvexDbLayer` actor
- SQLite with WAL mode

**Frameworks:**
- `Accelerate` for vDSP
- `Foundation` for Data/JSON encoding

## References

- [Embeddings Plan](../../../plans/embeddings-implementation-plan.md) § 4.4-4.5
- [SearchKit Spec](../../spec-v0.2.2.md) § 5 (Storage Schema)

## Example Usage

```swift
let store = VectorStore(db: tinyvex, dimensions: 384, modelID: "bge-small")

// Store embedding
try await store.store(
    id: "file1.swift",
    collection: "files",
    embedding: [0.1, 0.2, ...],  // 384 dims
    metadata: ["path": "src/file1.swift", "language": "swift"],
    text: "File summary text"
)

// Search
let results = try await store.search(
    query: queryEmbedding,
    collection: "files",
    limit: 10,
    minSimilarity: 0.7
)

for result in results {
    print("\(result.id): \(result.score)")
}
```

## Performance Targets

- Store single vector: <5ms
- Store 100 vectors (batch): <50ms
- Search 1,000 vectors: <100ms (brute-force)
- Search 10,000 vectors: <500ms (brute-force)
- Search 100,000 vectors: <5s (brute-force, future: ANN)

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/VectorStoreTests
```

## Notes

- Vectors are L2-normalized before storage (assumes provider normalized)
- Cosine similarity = dot product for unit vectors
- Brute-force search is sufficient for <100k vectors
- Future: Add ANN index (HNSW) for >100k vectors

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Performance targets met
- [ ] Documentation complete
- [ ] Merged to main branch
