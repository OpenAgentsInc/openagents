# Issue #006: Implement FTS5 Lexical Search

**Component:** `component:searchkit`
**Priority:** `priority:p1`
**Status:** `status:proposed`
**Effort:** 4-5 days
**Assignee:** TBD
**Created:** 2025-11-10
**Depends On:** None

## Description

Implement lexical (keyword-based) search using SQLite's FTS5 (Full-Text Search) extension with BM25 ranking. This provides fast keyword search over code files and complements semantic search for hybrid retrieval.

## Goals

1. Create FTS5 schema for file chunks
2. Implement chunking strategy (block-aware, ~500 tokens)
3. Index files with FTS5
4. Implement BM25-ranked search
5. Support highlight offsets for snippets
6. Handle incremental updates

## Implementation Details

### Files to Create

```
ios/OpenAgentsCore/Sources/OpenAgentsCore/
└── SearchKit/
    ├── Indexing/
    │   ├── Chunker.swift                # Block-aware text chunker
    │   ├── TokenEstimator.swift         # Estimate token count
    │   └── FileScanner.swift            # Scan workspace files
    ├── Search/
    │   ├── FTS5Engine.swift             # FTS5 search engine
    │   └── SearchTypes.swift            # Request/response types
    └── Storage/
        └── SearchKitDbLayer.swift       # Extends TinyvexDbLayer
```

### Schema

```sql
-- Files table
CREATE TABLE IF NOT EXISTS file (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    workspace_id TEXT NOT NULL,
    language TEXT,
    size_bytes INTEGER,
    mtime_ns INTEGER,
    sha256 TEXT,
    indexed_at INTEGER,
    FOREIGN KEY (workspace_id) REFERENCES workspace(id)
);

-- Chunks table
CREATE TABLE IF NOT EXISTS chunk (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    byte_range_start INTEGER NOT NULL,
    byte_range_end INTEGER NOT NULL,
    text TEXT NOT NULL,
    fingerprint TEXT,
    tokens INTEGER,
    created_at INTEGER,
    FOREIGN KEY (file_id) REFERENCES file(id)
);

-- FTS5 virtual table
CREATE VIRTUAL TABLE IF NOT EXISTS chunk_fts USING fts5(
    chunk_id UNINDEXED,
    text,
    tokenize='porter unicode61'
);
```

### FTS5Engine Actor

```swift
public actor FTS5Engine {
    private let db: SearchKitDbLayer

    // Indexing
    public func indexFile(
        path: String,
        content: String,
        workspace: String
    ) async throws

    // Search
    public func search(
        query: String,
        workspace: String,
        limit: Int = 20,
        offset: Int = 0
    ) async throws -> [LexicalSearchResult]

    // Management
    public func rebuildIndex(workspace: String) async throws
    public func deleteFile(path: String) async throws
}
```

### Chunking Strategy

- **Target size**: 500 tokens (~2000 chars)
- **Strategy**: Block-aware (respect function/class boundaries)
- **Overlap**: 50 tokens between chunks
- **Languages**: Swift, TypeScript, Python, Rust (detect via extension)

## Acceptance Criteria

- [ ] Schema created with `file`, `chunk`, `chunk_fts` tables
- [ ] `Chunker` implements block-aware splitting
- [ ] `TokenEstimator` provides quick token counts
- [ ] `FileScanner` walks workspace respecting `.gitignore`
- [ ] `FTS5Engine` indexes files with BM25 ranking
- [ ] Search returns ranked results with highlight offsets
- [ ] Incremental updates (only re-index changed files)
- [ ] Unit tests:
  - `testChunking()`
  - `testTokenEstimation()`
  - `testFTS5Indexing()`
  - `testBM25Ranking()`
  - `testHighlightOffsets()`
- [ ] Performance: Index 1,000 files in <30s, search <300ms p95

## Dependencies

- Existing `TinyvexDbLayer` pattern
- SQLite with FTS5 enabled (default on macOS)

## References

- [SearchKit Spec](../../spec-v0.2.2.md) § 5, § 8 (Lexical Search)
- [SQLite FTS5 Documentation](https://www.sqlite.org/fts5.html)

## Example Usage

```swift
let engine = FTS5Engine(db: searchKitDb)

// Index a file
try await engine.indexFile(
    path: "src/main.swift",
    content: fileContent,
    workspace: "/path/to/workspace"
)

// Search
let results = try await engine.search(
    query: "authentication middleware",
    workspace: "/path/to/workspace",
    limit: 10
)

for result in results {
    print("\(result.path): \(result.score)")
    print("  Snippet: \(result.snippet)")
}
```

## Chunking Example

```swift
let chunker = Chunker(targetTokens: 500, overlapTokens: 50)
let chunks = try chunker.chunk(
    content: swiftFileContent,
    language: .swift
)

for chunk in chunks {
    print("Lines \(chunk.startLine)-\(chunk.endLine): \(chunk.tokens) tokens")
}
```

## Performance Targets

- Index 1,000 files (~5 MB): <30s
- Search (100k chunks): <300ms p95
- Highlight extraction: <10ms per result

## Testing

```bash
cd ios
xcodebuild test -workspace OpenAgents.xcworkspace \
  -scheme OpenAgents -sdk macosx \
  -only-testing:OpenAgentsCoreTests/FTS5EngineTests
```

## Notes

- Use FTS5 `highlight()` function for snippet extraction
- BM25 parameters: k1=1.2, b=0.75 (SQLite defaults)
- Store original text in `chunk` table (not FTS5) to save space
- Use `mtime_ns` + `sha256` to detect file changes

## Definition of Done

- [ ] Code implemented and reviewed
- [ ] Unit tests passing
- [ ] Performance targets met
- [ ] Tested on real codebase (OpenAgents repo)
- [ ] Documentation complete
- [ ] Merged to main branch
