# SearchKit MVP (Hybrid Search Engine)

**Phase:** 4 - SearchKit (Optional/Deferrable)
**Component:** macOS App + OpenAgentsCore
**Priority:** P2 (Nice-to-have - Advanced capability)
**Estimated Effort:** 6-8 weeks

## Summary

Implement SearchKit, a hybrid search engine (lexical + semantic) for codebase exploration, as specified in `docs/engine/spec-v0.2.2.md`. This enables advanced agent capabilities like "Explore Codebase" and semantic Q&A over large codebases.

## Motivation

**Why SearchKit matters**:
- **Better agents**: Agents can understand codebases semantically, not just grep
- **Marketplace differentiator**: Offer search jobs (kind:5200) that competitors can't
- **Foundation Models integration**: Power "Explore" orchestration with hybrid search

**Why deferrable**:
- MVP marketplace can work without it (basic jobs don't need search)
- Complex implementation (8-12 weeks vs 2-4 weeks for basic features)
- Can validate marketplace demand first, then add SearchKit

**Architecture** (from spec-v0.2.2):
```
SearchKitCore (SwiftPM)
├── Chunking (block-aware, safe spans)
├── FTS5 (BM25 scoring)
├── Embeddings (Core ML)
├── Vector Storage (L2-normalized blobs)
├── ANN Search (brute-force initially)
├── RRF Fusion (combine lexical + semantic)
└── Span Reading (bounded context)

SearchKitService (macOS)
├── Index Management
├── WebSocket Server (ACP methods)
├── Workspace Security (scoped bookmarks)
└── File Watcher (FSEvents)
```

## Acceptance Criteria (High-Level)

### SearchKitCore Library
- [ ] Chunking engine (block-aware, token estimation)
- [ ] FTS5 integration (SQLite full-text search, BM25)
- [ ] Embeddings via EmbeddingService (dim=384/768, provider-agnostic)
- [ ] Vector storage (normalized Float32 blobs)
- [ ] Hybrid search (RRF fusion of lexical + semantic)
- [ ] Span reading (bounded results, highlight offsets)
- [ ] SQLite schema (file, chunk, chunk_fts, chunk_vec, symbol)

### SearchKitService (macOS)
- [ ] Index management (add roots, build, status)
- [ ] Workspace scanning (respect .gitignore, binaries)
- [ ] Background indexing (out-of-session, subscription model)
- [ ] ACP method handlers:
  - `search.lexical`, `search.semantic`, `search.hybrid`
  - `content.get_span`
  - `index.add_root`, `index.status`, `index.rebuild`
- [ ] WebSocket server (JSON-RPC 2.0)
- [ ] File watcher (incremental updates via FSEvents)

### SearchKitClient SDK
- [ ] Typed client for SearchKit methods
- [ ] Async/await API
- [ ] Streaming support (plan snapshots)

### Integration with Marketplace
- [ ] SearchKit as job backend (kind:5200 - codebase search)
- [ ] NIP-90 service provider for search jobs
- [ ] Results published to Nostr (encrypted if requested)

## Technical Design (Summary)

**Full specification**: See `docs/engine/spec-v0.2.2.md` (~6000 words)

```swift
// SearchKitCore/Sources/SearchKitCore/

Chunker.swift                // Text chunking
FTS5Index.swift              // Full-text search
// EmbeddingService is the semantic provider (no bespoke generator here)
VectorStore.swift            // Vector storage + ANN
HybridSearch.swift           // RRF fusion
SpanReader.swift             // Bounded result extraction
SearchKitDatabase.swift      // SQLite schema
```

```swift
// Example API

let searchKit = SearchKitService()

// Index workspace
await searchKit.addRoot(path: "/Users/me/code/openagents")
await searchKit.buildIndex()

// Hybrid search
let results = await searchKit.search(
    query: "where is authentication handled?",
    mode: .hybrid,
    limit: 10
)

// results: [SearchResult]
// - file path
// - chunk content
// - score
// - highlight offsets
```

## Dependencies

- **Issue #007**: macOS Worker (for marketplace integration)
- **ADR-0006**: Foundation Models (for Explore orchestration)
- **docs/engine/spec-v0.2.2.md**: Full SearchKit specification

### External Dependencies
- **SQLite** (FTS5 extension)
- **Core ML** (embedding models)
- **SwiftNIO** (WebSocket server)

## Testing (From Spec)

### Unit Tests
- [ ] Chunker (block-aware, safe spans)
- [ ] FTS5 (BM25 scoring, highlighting)
- [ ] Embeddings (generation, normalization)
- [ ] RRF fusion (score combination)
- [ ] MMR diversity (optional)

### Integration Tests
- [ ] WebSocket server (JSON-RPC methods)
- [ ] ACP tool call streaming
- [ ] Index build (scan, chunk, embed, store)
- [ ] Incremental updates (file changes)

### E2E Tests
- [ ] Explore Codebase on openagents repo
- [ ] Lexical search: "search.lexical" method
- [ ] Semantic search: "search.semantic" method
- [ ] Hybrid search: "search.hybrid" method
- [ ] Content spans: "content.get_span" method

### Performance Tests (from spec)
- [ ] Indexing throughput: ≥5 MB/s text
- [ ] Lexical search: <300ms p95
- [ ] Semantic search: <500ms p95
- [ ] Hybrid search: <700ms p95
- [ ] Plan snapshot: <300ms p95 (first)

## Apple Compliance

**ASRG 2.5.2 (No Downloaded Code)**
- ✅ **Compliant**: SearchKit indexes code (data), doesn't execute it
- ✅ Embedding models are Core ML (on-device)

**Privacy**:
- ✅ All indexing on-device (no cloud API calls)
- ✅ Workspace access via security-scoped bookmarks
- ✅ User grants access explicitly

## Success Metrics (From Spec)

- [ ] Index 100k-chunk corpus in <5 minutes
- [ ] Search latencies meet targets (p95)
- [ ] Hybrid search quality > lexical or semantic alone
- [ ] Explore Codebase works on openagents repo
- [ ] Memory watermark < 2GB during indexing

## Why Defer This Issue?

**Arguments for Phase 4 (deferrable)**:
1. **MVP doesn't need it**: Basic marketplace jobs (summarization, code gen) work without search
2. **Validation first**: Prove marketplace demand before investing 6-8 weeks
3. **Complex implementation**: SearchKit is a large subsystem (see 6000-word spec)
4. **Foundation Models sufficient for MVP**: Simple agent tasks don't need hybrid search

**Arguments for Phase 1-2 (critical)**:
1. **Differentiator**: Hybrid search is unique capability vs competitors
2. **Foundation Models orchestration**: "Explore" already uses search concepts
3. **Marketplace job kind**: kind:5200 (search) is valuable service to offer

**Recommendation**: Defer to Phase 4. Build marketplace first, add SearchKit when demand validated.

## Implementation Plan (If Approved)

### Week 1-2: SearchKitCore Foundation
- Chunking engine
- FTS5 integration
- SQLite schema

### Week 3-4: Embeddings & Vector Storage
- Core ML embedding generation
- Vector storage (L2-normalized blobs)
- ANN search (brute-force)

### Week 5-6: Hybrid Search & Fusion
- RRF fusion
- Span reading
- Highlighting

### Week 7-8: Service & Integration
- WebSocket server (ACP methods)
- Index management
- Marketplace integration (kind:5200)

## Reference Links

### OpenAgents Docs
- **Engine Spec v0.2.2**: `/Users/christopherdavid/code/openagents/docs/engine/spec-v0.2.2.md` (MUST READ - full spec)
- **ACP Alignment**: `/Users/christopherdavid/code/openagents/docs/engine/acp-alignment.md`

### External
- **FTS5**: https://www.sqlite.org/fts5.html
- **RRF**: https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf
- **SwiftNIO**: https://github.com/apple/swift-nio

## Notes

- **Read the spec**: This issue is a summary. Full spec in `docs/engine/spec-v0.2.2.md`
- **Deferrable**: Can skip for MVP if marketplace validation is priority
- **Incremental**: Can implement search.lexical first, add semantic later
- **Marketplace integration**: kind:5200 jobs route to SearchKit backend

## Future Enhancements

- Quantized ANN (FAISS, HNSW)
- Multi-language embedding models
- Incremental indexing (watch mode)
- Remote index sharing (marketplace index as a service)
- Git integration (search across commits, branches)
