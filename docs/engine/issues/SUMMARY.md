# OpenAgents Engine Issues - Summary

Created: 2025-11-10

## What Was Created

A comprehensive set of **10 GitHub-style implementation issues** covering the full stack from low-level embeddings to high-level issue automation.

## Directory Structure

```
docs/engine/issues/
├── README.md                                # Overview and labels
├── IMPLEMENTATION_ORDER.md                  # Dependencies and timeline
├── SUMMARY.md                              # This file
│
├── embeddings/                             # Phase 1 (Week 1-3)
│   ├── 001-mlx-embedding-provider.md       # MLX + BGE-small integration
│   ├── 002-vector-store-sqlite.md          # Persistent vector storage
│   └── 003-embedding-service.md            # High-level embed + search API
│
├── searchkit/                              # Phase 2 (Week 4-6)
│   ├── 006-fts5-lexical-search.md          # BM25 keyword search
│   └── 007-hybrid-search-rrf.md            # Fused lexical + semantic
│
└── issue-agent/                            # Phase 3-4 (Week 7-10)
    ├── 016-issue-enhancer.md               # FM-based issue analysis
    ├── 017-retrieval-orchestrator.md       # File selection via hybrid search
    ├── 018-agent-delegator.md              # Patch generation via agents
    └── 020-end-to-end-orchestration.md     # Complete workflow
```

## Architecture Layers

```
┌─────────────────────────────────────────────┐
│  IssueAgent (High-level)                    │  ← Issues #016-#020
│  - Issue enhancement (FM)                   │
│  - Retrieval orchestration                  │
│  - Agent delegation                         │
│  - PR creation                              │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│  SearchKit (Mid-level)                      │  ← Issues #006-#007
│  - FTS5 lexical search                      │
│  - Hybrid search (RRF fusion)               │
│  - Chunking, span reading                   │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│  Embeddings (Low-level)                     │  ← Issues #001-#003
│  - MLX embedding generation                 │
│  - Vector storage (SQLite)                  │
│  - Similarity search                        │
└─────────────────────────────────────────────┘
```

## Key Design Decisions

### 1. Local-First Intelligence
- **MLX embeddings** (not OpenAI API)
- **BGE-small-en-v1.5** (384 dims, 45 MB)
- **Foundation Models** for simple tasks
- **Strategic delegation** to Claude Code/Codex for complex patches

### 2. Hybrid Retrieval
- **FTS5 (BM25)** for keyword precision
- **Semantic search** for concept understanding
- **RRF fusion** for best-of-both

### 3. Agent Delegation Model
- **Simple edits** → Foundation Models (fast, local)
- **Moderate patches** → Claude Code (balanced)
- **Complex refactors** → Codex (specialized)

### 4. Human-in-the-Loop
- Default: return patches for review
- Optional: auto-create draft PRs
- Progress streaming via ACP

## Implementation Timeline

**Solo Developer:** 8-10 weeks
**Two Developers:** 5-6 weeks
**Three Developers:** 4-5 weeks

### Critical Path (24 days)
```
#001 → #002 → #003 → #007 → #017 → #018 → #020
```

### With Parallelization (13-15 days)
- #006 (FTS5) parallel to #001-#003
- #016 (Enhancer) parallel to #002-#007

## Quick Start

1. **Read the plans:**
   - [Embeddings Implementation Plan](../../plans/embeddings-implementation-plan.md)
   - [IssueAgent Architecture](../../plans/issue-agent-architecture.md)
   - [SearchKit Spec v0.2.2](../spec-v0.2.2.md)

2. **Review dependencies:**
   - Read [IMPLEMENTATION_ORDER.md](./IMPLEMENTATION_ORDER.md)
   - Understand critical path
   - Identify parallel work opportunities

3. **Start with Phase 1:**
   - Begin with #001 (MLX Provider)
   - Optionally start #006 (FTS5) in parallel
   - Follow acceptance criteria in each issue

4. **Test as you go:**
   - Each issue has unit tests
   - Phase-end integration tests
   - Golden test corpus for end-to-end

## Success Metrics

### Retrieval Quality
- **Recall@10:** ≥80%
- **Precision@10:** ≥60%
- **MRR:** ≥0.7

### Patch Quality
- **Syntax valid:** ≥95%
- **Apply cleanly:** ≥90%
- **Addresses issue:** ≥70% (manual review)

### PR Quality
- **Acceptance rate:** ≥40% (after review)
- **Time to merge:** Median <3 days

### Performance
- **Embedding:** <50ms p50
- **Search:** <700ms p95
- **Issue → Patches:** <60s
- **Full workflow:** <90s

## Labels & Status

Use these labels to track progress:

**Status:**
- `status:proposed` - Defined, not started
- `status:in-progress` - Being worked on
- `status:blocked` - Waiting on dependencies
- `status:review` - Needs code review
- `status:done` - Merged

**Priority:**
- `priority:p0` - Critical, blocking
- `priority:p1` - High priority
- `priority:p2` - Medium priority

**Component:**
- `component:embeddings`
- `component:searchkit`
- `component:issue-agent`
- `component:acp`
- `component:bridge`

## Related Documentation

### Plans
- [Embeddings Implementation Plan](../../plans/embeddings-implementation-plan.md)
- [IssueAgent Architecture](../../plans/issue-agent-architecture.md)

### Specs
- [SearchKit Spec v0.2.2](../spec-v0.2.2.md)
- [ACP Alignment](../acp-alignment.md)

### Reference Implementation
- [Pierrebhat Spec](../../../../pierrebhat/docs/SPEC.md)

### ADRs
- ADR-0002: Agent Client Protocol
- ADR-0006: Foundation Models
- ADR-0009: IssueAgent Architecture (TBD)

## Key Technologies

- **Swift 5.9+** (async/await, actors)
- **MLX Swift** (Apple's ML framework)
- **SQLite FTS5** (full-text search)
- **Foundation Models** (on-device LLM)
- **Accelerate** (vDSP for vector ops)
- **ACP** (Agent Client Protocol)

## Getting Help

- **Questions:** Open a GitHub discussion
- **Bugs:** Open a GitHub issue
- **Docs:** Check `docs/` directory
- **Examples:** See each issue's "Example Usage" section

## Next Steps

1. ✅ **Plans created** - Done
2. ✅ **Issues created** - Done
3. ⏳ **Assign issues** - Assign to developers
4. ⏳ **Set up tracking** - GitHub Projects / Linear
5. ⏳ **Begin Phase 1** - Start #001

## Summary

**Created:**
- 10 detailed GitHub-style issues
- Complete implementation order
- Dependency graph
- Timeline estimates
- Success criteria
- Testing strategy

**Ready for:**
- Developer assignment
- Sprint planning
- Implementation kickoff

**Estimated delivery:**
- Phase 1 (Embeddings): 3 weeks
- Phase 2 (SearchKit): 3 weeks
- Phase 3 (IssueAgent): 3 weeks
- Phase 4 (Integration): 1 week

**Total:** 8-10 weeks with buffer

---

Good luck with implementation! 🚀
