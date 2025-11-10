# Issue #007: Implement Hybrid Search with RRF Fusion

**Component:** `component:searchkit`
**Priority:** `priority:p1`
**Status:** `status:proposed`
**Effort:** 3-4 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** #003, #006

## Description

Implement hybrid search that combines lexical (FTS5 BM25) and semantic (embedding cosine similarity) search using Reciprocal Rank Fusion (RRF). This provides the best of both worlds: keyword precision and semantic understanding.

## Goals

1. Implement RRF fusion algorithm
2. Coordinate FTS5 and embedding searches
3. Support configurable lexical/semantic weights
4. Add MMR (Maximal Marginal Relevance) for diversity (optional)
5. Return unified ranked results

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── SearchKit/
    └── Search/
        ├── HybridSearch.swift           # Main hybrid search actor
        ├── RRFFusion.swift              # Reciprocal Rank Fusion
        └── SemanticSearch.swift         # Semantic search wrapper
```

### HybridSearch Actor

```swift
public actor HybridSearch {
    private let fts5Engine: FTS5Engine
    private let embeddingService: EmbeddingService
    private let config: HybridSearchConfig

    public func search(
        query: String,
        workspace: String,
        limit: Int = 20,
        lexicalWeight: Float = 0.4,
        semanticWeight: Float = 0.6
    ) async throws -> [HybridSearchResult]
}
```

### RRF Algorithm

```swift
public struct RRFFusion {
    public static func fuse(
        lexicalResults: [LexicalSearchResult],
        semanticResults: [SemanticSearchResult],
        k: Int = 60  // RRF constant
    ) -> [FusedResult] {
        // RRF score = Σ(1 / (k + rank))
        var scores: [String: Float] = [:]

        for (rank, result) in lexicalResults.enumerated() {
            scores[result.id, default: 0] += 1.0 / Float(k + rank + 1)
        }

        for (rank, result) in semanticResults.enumerated() {
            scores[result.id, default: 0] += 1.0 / Float(k + rank + 1)
        }

        return scores
            .sorted { $0.value > $1.value }
            .map { FusedResult(id: $0.key, score: $0.value) }
    }
}
```

### Result Types

```swift
public struct HybridSearchResult: Codable, Sendable {
    public var path: String
    public var score: Float              // Fused RRF score
    public var lexicalScore: Float?      // Original BM25 score
    public var semanticScore: Float?     // Original cosine score
    public var snippet: String?          // From lexical search
    public var chunkId: Int?
    public var startLine: Int?
    public var endLine: Int?
}
```

## Acceptance Criteria

- [ ] `HybridSearch` actor coordinates FTS5 + embeddings
- [ ] RRF fusion algorithm implemented correctly
- [ ] Configurable lexical/semantic weights
- [ ] Returns top-K results with fused scores
- [ ] Includes both lexical and semantic scores for debugging
- [ ] De-duplicates results (same file chunk appears once)
- [ ] Optional MMR for diversity (P2 feature)
- [ ] Unit tests:
  - `testRRFFusion()`
  - `testHybridSearchCoordination()`
  - `testWeightedFusion()`
  - `testDeduplication()`
- [ ] Integration test with real corpus
- [ ] Performance: <700ms p95 for hybrid search

## Dependencies

- Issue #003 (EmbeddingService)
- Issue #006 (FTS5Engine)

## References

- [SearchKit Spec](../../spec-v0.2.2.md) § 8 (Hybrid Search)
- [RRF Paper](https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf)
- [IssueAgent Architecture](../../../plans/issue-agent-architecture.md) § 4.1 (Retrieval)

## Example Usage

```swift
let hybridSearch = HybridSearch(
    fts5Engine: fts5,
    embeddingService: embeddings
)

let results = try await hybridSearch.search(
    query: "authentication middleware",
    workspace: "/path/to/workspace",
    limit: 10,
    lexicalWeight: 0.4,
    semanticWeight: 0.6
)

for result in results {
    print("\(result.path): \(result.score)")
    print("  Lexical: \(result.lexicalScore ?? 0)")
    print("  Semantic: \(result.semanticScore ?? 0)")
    print("  Snippet: \(result.snippet ?? "")")
}
```

## RRF Example

```swift
let lexical = [
    LexicalSearchResult(id: "file1", rank: 1, score: 0.9),
    LexicalSearchResult(id: "file2", rank: 2, score: 0.7),
    LexicalSearchResult(id: "file3", rank: 3, score: 0.5),
]

let semantic = [
    SemanticSearchResult(id: "file2", rank: 1, score: 0.95),
    SemanticSearchResult(id: "file3", rank: 2, score: 0.85),
    SemanticSearchResult(id: "file4", rank: 3, score: 0.75),
]

let fused = RRFFusion.fuse(lexicalResults: lexical, semanticResults: semantic)
// file2 likely ranks highest (appears in both)
```

## Weighted Fusion (Optional)

```swift
public struct WeightedRRF {
    public static func fuse(
        lexicalResults: [LexicalSearchResult],
        semanticResults: [SemanticSearchResult],
        lexicalWeight: Float = 0.4,
        semanticWeight: Float = 0.6,
        k: Int = 60
    ) -> [FusedResult] {
        // Apply weights to RRF scores
        var scores: [String: Float] = [:]

        for (rank, result) in lexicalResults.enumerated() {
            scores[result.id, default: 0] += lexicalWeight / Float(k + rank + 1)
        }

        for (rank, result) in semanticResults.enumerated() {
            scores[result.id, default: 0] += semanticWeight / Float(k + rank + 1)
        }

        return scores.sorted { $0.value > $1.value }.map { FusedResult(id: $0.key, score: $0.value) }
    }
}
```

## Performance Targets

- Hybrid search (10k chunks): <700ms p95
- RRF fusion (100 results): <5ms
- Memory: <50 MB overhead

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/HybridSearchTests
```

## Golden Test Corpus

Create test queries with expected results:

```
tests/golden/hybrid-search/
├── query-001-authentication.json
├── query-002-database-migration.json
└── query-003-error-handling.json
```

Each query file:
```json
{
  "query": "authentication middleware",
  "expectedFiles": ["src/auth/middleware.swift", "src/auth/jwt.swift"],
  "minRecall": 0.8,
  "minPrecisionAt5": 0.6
}
```

## Notes

- RRF is rank-based, not score-based (more robust)
- k=60 is recommended default (from paper)
- MMR adds diversity but increases latency (optional P2)
- Weights can be tuned based on user feedback

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Golden tests passing
- [ ] Performance targets met
- [ ] Tested on OpenAgents repo
- [ ] Documentation complete
- [ ] Merged to main branch
