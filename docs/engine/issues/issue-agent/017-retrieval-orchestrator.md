# Issue #017: Implement Retrieval Orchestrator

**Component:** `component:issue-agent`
**Priority:** `priority:p1`
**Status:** `status:proposed`
**Effort:** 3-4 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** #007, #016

## Description

Implement the `RetrievalOrchestrator` actor that coordinates hybrid search across multiple query variations, deduplicates results, applies filtering (file extensions, metadata), and ranks files for patch generation.

This is the bridge between issue enhancement and patch generation - it selects the "right" files to modify.

## Goals

1. Run hybrid search for multiple query variations
2. Deduplicate results by file path
3. Apply filters (extensions, metadata, recency)
4. Re-rank results using heuristics
5. Return top-K files with confidence scores

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── IssueAgent/
    └── Retrieval/
        ├── RetrievalOrchestrator.swift  # Main orchestrator
        ├── FileRanker.swift             # Post-retrieval ranking
        └── RetrievalTypes.swift         # RankedFile, filters
```

### RetrievalOrchestrator Actor

```swift
public actor RetrievalOrchestrator {
    private let hybridSearch: HybridSearch
    private let embeddingService: EmbeddingService

    public func findRelevantFiles(
        for enhancement: EnhancedIssue,
        workspace: String,
        topK: Int = 20
    ) async throws -> [RankedFile]
}
```

### Algorithm

```
1. For each search query in enhancement:
   a. Run hybrid search with query
   b. Collect top-K*2 results

2. Deduplicate by path (keep highest score)

3. Apply filters:
   a. File extension filter (if specified)
   b. Minimum similarity threshold
   c. Exclude test files (optional)

4. Re-rank using heuristics:
   a. Recency (prefer recently modified)
   b. Size (prefer smaller files for focused changes)
   c. Depth (prefer files closer to root)
   d. Prior success (learn from history)

5. Return top-K
```

### RankedFile Type

```swift
public struct RankedFile: Codable, Sendable, Identifiable {
    public var id: String { path }
    public var path: String
    public var score: Float              // Final score after re-ranking
    public var hybridScore: Float        // Original hybrid search score
    public var lexicalScore: Float?
    public var semanticScore: Float?
    public var snippet: String?
    public var metadata: FileMetadata?

    public struct FileMetadata: Codable, Sendable {
        public var language: String?
        public var sizeBytes: Int?
        public var mtimeNs: Int64?
        public var depth: Int?
    }
}
```

### FileRanker

```swift
public struct FileRanker {
    public static func rerank(
        _ files: [HybridSearchResult],
        workspace: String,
        preferences: RankingPreferences = .default
    ) async throws -> [RankedFile]
}

public struct RankingPreferences: Codable, Sendable {
    public var recencyWeight: Float = 0.2
    public var sizeWeight: Float = 0.1
    public var depthWeight: Float = 0.1
    public var hybridScoreWeight: Float = 0.6

    public var preferSmallFiles: Bool = true  // Focused changes
    public var preferShallowFiles: Bool = true  // Core files
    public var preferRecentFiles: Bool = true  // Active development

    public static let `default` = RankingPreferences()
}
```

## Acceptance Criteria

- [ ] `RetrievalOrchestrator` runs hybrid search for all queries
- [ ] Results deduplicated by file path
- [ ] Extension filter applied when specified
- [ ] Re-ranking combines hybrid score + heuristics
- [ ] Returns top-K with confidence scores
- [ ] Handles empty results gracefully
- [ ] Unit tests:
  - `testMultiQueryRetrieval()`
  - `testDeduplication()`
  - `testExtensionFilter()`
  - `testReranking()`
  - `testEmptyResults()`
- [ ] Integration test with real issues
- [ ] Performance: <5s for retrieval + ranking

## Dependencies

- Issue #007 (HybridSearch)
- Issue #016 (IssueEnhancer)

## References

- [IssueAgent Architecture](../../../plans/issue-agent-architecture.md) § 4.3 (Retrieval)
- [Pierrebhat Spec](../../../../pierrebhat/docs/SPEC.md) § "Retrieval"

## Example Usage

```swift
let enhanced = EnhancedIssue(
    issue: issue,
    keyFocus: "Add dark mode toggle to settings",
    allowedExtensions: [".swift"],
    searchQueries: [
        "settings dark mode",
        "theme switcher",
        "appearance preference"
    ],
    estimatedComplexity: .moderate
)

let orchestrator = RetrievalOrchestrator(
    hybridSearch: hybridSearch,
    embeddingService: embeddingService
)

let files = try await orchestrator.findRelevantFiles(
    for: enhanced,
    workspace: "/path/to/workspace",
    topK: 10
)

for file in files {
    print("\(file.path): \(file.score)")
    print("  Hybrid: \(file.hybridScore)")
    print("  Snippet: \(file.snippet ?? "")")
}
```

## Deduplication Strategy

```swift
private func deduplicate(_ results: [HybridSearchResult]) -> [HybridSearchResult] {
    var seen: [String: HybridSearchResult] = [:]

    for result in results {
        if let existing = seen[result.path] {
            // Keep higher score
            if result.score > existing.score {
                seen[result.path] = result
            }
        } else {
            seen[result.path] = result
        }
    }

    return Array(seen.values).sorted { $0.score > $1.score }
}
```

## Ranking Heuristics

### Recency Score

```swift
func recencyScore(mtimeNs: Int64) -> Float {
    let now = Date().timeIntervalSince1970 * 1_000_000_000
    let ageNs = now - Double(mtimeNs)
    let ageDays = ageNs / (24 * 60 * 60 * 1_000_000_000)

    // Decay over 90 days
    return max(0, 1.0 - Float(ageDays) / 90.0)
}
```

### Size Score

```swift
func sizeScore(bytes: Int) -> Float {
    // Prefer smaller files (more focused)
    // Score: 1.0 for <1KB, 0.5 for 10KB, 0.1 for >100KB
    let kb = Float(bytes) / 1024.0
    return 1.0 / (1.0 + log10(max(1, kb)))
}
```

### Depth Score

```swift
func depthScore(path: String) -> Float {
    let depth = path.split(separator: "/").count
    // Prefer shallower files (core files)
    return 1.0 / Float(depth)
}
```

### Combined Score

```swift
func finalScore(
    hybridScore: Float,
    recency: Float,
    size: Float,
    depth: Float,
    preferences: RankingPreferences
) -> Float {
    return hybridScore * preferences.hybridScoreWeight +
           recency * preferences.recencyWeight +
           size * preferences.sizeWeight +
           depth * preferences.depthWeight
}
```

## Performance Targets

- Multi-query retrieval: <5s for 3-5 queries
- Deduplication: <10ms for 100 results
- Re-ranking: <50ms for 100 files

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/RetrievalOrchestratorTests
```

## Golden Tests

```
tests/golden/retrieval/
├── issue-001-dark-mode/
│   ├── enhanced.json
│   ├── expected_files.json
│   └── workspace/
└── issue-002-bug-fix/
    ├── enhanced.json
    ├── expected_files.json
    └── workspace/
```

Expected results:
```json
{
  "expectedFiles": [
    "src/Settings.swift",
    "src/Theme.swift",
    "src/AppearanceManager.swift"
  ],
  "minRecall": 0.8,  // At least 80% of expected files in top-20
  "minPrecision": 0.6  // At least 60% of top-10 are relevant
}
```

## Notes

- Query variations important for recall (synonyms, rephrasings)
- Deduplication prevents same file appearing multiple times
- Re-ranking balances search quality with practical heuristics
- Learn from PR outcomes to improve ranking over time

## Future Enhancements

- Machine learning ranking (learn from accepted PRs)
- Cross-file dependency analysis (e.g., if A imports B, rank B higher)
- User feedback loop (thumbs up/down on file selection)

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Golden tests passing (>80% recall, >60% precision)
- [ ] Performance targets met
- [ ] Tested on OpenAgents repo
- [ ] Documentation complete
- [ ] Merged to main branch
