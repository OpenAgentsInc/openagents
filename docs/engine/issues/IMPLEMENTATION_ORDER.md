# Implementation Order & Dependencies

This document outlines the recommended implementation order for the OpenAgents engine issues, including dependencies and parallel work opportunities.

## Quick Reference

**Total Issues:** 10 (5 embeddings, 2 SearchKit, 3 IssueAgent)
**Estimated Timeline:** 8-10 weeks
**Critical Path:** Embeddings → SearchKit → IssueAgent

## Phase 1: Embeddings Foundation (Week 1-3)

**Goal:** Local embedding generation and vector storage

### Issues (Sequential)

1. **#001: MLX Embedding Provider** → 3-5 days
   - No dependencies
   - **Start immediately**
   - Deliverable: BGE-small embeddings working

2. **#002: Vector Store SQLite** → 3-4 days
   - Depends on: #001
   - **Start after #001 complete**
   - Deliverable: Persistent vector storage

3. **#003: Embedding Service** → 2-3 days
   - Depends on: #001, #002
   - **Start after #002 complete**
   - Deliverable: High-level embed + search APIs

### Parallel Work Opportunities

- While #001-#003 are being implemented, you can work on:
  - SearchKit planning and design
  - IssueAgent type definitions
  - GitHub API client (Issue #016 prep)

---

## Phase 2: SearchKit Primitives (Week 4-6)

**Goal:** Hybrid search (FTS5 + semantic) infrastructure

### Issues (Partial Parallelization)

4. **#006: FTS5 Lexical Search** → 4-5 days
   - No dependencies (can start in parallel with Phase 1)
   - **Can start during Phase 1**
   - Deliverable: BM25-ranked keyword search

5. **#007: Hybrid Search with RRF** → 3-4 days
   - Depends on: #003, #006
   - **Start after #003 and #006 complete**
   - Deliverable: Fused lexical + semantic search

### Parallel Work Opportunities

- #006 can be developed in parallel with #001-#003 (no embedding dependency)
- While #007 is being implemented:
  - IssueAgent enhancement layer (#016) can start
  - GitHub client can be built

---

## Phase 3: IssueAgent Core (Week 7-9)

**Goal:** Issue processing and retrieval orchestration

### Issues (Partial Parallelization)

6. **#016: Issue Enhancer** → 3-4 days
   - No dependencies (uses existing FM)
   - **Can start in parallel with Phase 2**
   - Deliverable: Issue → Enhanced metadata

7. **#017: Retrieval Orchestrator** → 3-4 days
   - Depends on: #007, #016
   - **Start after #007 and #016 complete**
   - Deliverable: Issue → Top-K relevant files

8. **#018: Agent Delegator** → 5-7 days
   - Depends on: #016, #017, Bridge setup
   - **Start after #017 complete**
   - Deliverable: File + issue → Patch

### Parallel Work Opportunities

- #016 can be developed early (Phase 2 timeframe)
- While #018 is being implemented:
  - PR builder can be developed
  - Workspace manager can be implemented

---

## Phase 4: End-to-End Integration (Week 10)

**Goal:** Complete workflow from issue → patches → PR

### Issues

9. **#020: End-to-End Orchestration** → 5-7 days
   - Depends on: #016, #017, #018
   - **Start after all prior issues complete**
   - Deliverable: Full issue processing service

### Testing & Polish

- Golden test corpus creation
- Performance optimization
- Integration testing
- Documentation

---

## Dependency Graph

```
#001 (MLX Provider)
  └─→ #002 (Vector Store)
        └─→ #003 (Embedding Service)
              └─→ #007 (Hybrid Search) ←─┐
                    └─→ #017 (Retrieval) │
                          └─→ #018 (Agent Delegator)
                                └─→ #020 (End-to-End)

#006 (FTS5 Search) ─────────────────────┘

#016 (Issue Enhancer) ──→ #017 (Retrieval)
                           └─→ #018 (Agent Delegator)
```

## Parallel Work Opportunities

### Early Parallel Work (Week 1-2)

While #001 is being implemented:
- ✅ #006 (FTS5) - no dependencies
- ✅ #016 (Issue Enhancer) - no embedding dependencies
- ✅ GitHub API client
- ✅ Type definitions

### Mid Parallel Work (Week 4-5)

While #007 is being implemented:
- ✅ #016 (Issue Enhancer) - if not already done
- ✅ Workspace manager
- ✅ PR builder scaffolding

### Late Parallel Work (Week 8-9)

While #018 is being implemented:
- ✅ PR builder completion
- ✅ GitHub PR creation
- ✅ Golden test corpus

---

## Critical Path

**Critical path (longest dependency chain):**

```
#001 → #002 → #003 → #007 → #017 → #018 → #020
3d     3d     2d     3d     3d     5d     5d  = 24 days
```

**With parallel work:**
- #006 runs parallel to #001-#003 (-7 days)
- #016 runs parallel to #002-#007 (-4 days)

**Adjusted timeline:** ~13-15 working days (≈3 weeks) on critical path

**Total with buffer:** 8-10 weeks (accounting for reviews, testing, polish)

---

## Recommended Team Allocation

### Solo Developer

**Order:**
1. #001 → #002 → #003 (embeddings)
2. #006 (FTS5, parallel to Phase 1)
3. #007 (hybrid search)
4. #016 (issue enhancer, parallel to Phase 2)
5. #017 (retrieval)
6. #018 (agent delegator)
7. #020 (end-to-end)

**Timeline:** 8-10 weeks

### Two Developers

**Developer A (Backend/Storage):**
1. #001 → #002 → #003 (embeddings)
2. #018 (agent delegator)
3. #020 (end-to-end integration)

**Developer B (Search/Orchestration):**
1. #006 (FTS5, parallel to Dev A's #001-#003)
2. #007 (hybrid search)
3. #016 → #017 (enhancement + retrieval)

**Timeline:** 5-6 weeks

### Three Developers

**Developer A (Embeddings):**
- #001 → #002 → #003

**Developer B (Search):**
- #006 → #007

**Developer C (IssueAgent):**
- #016 → #017 → #018 → #020

**Timeline:** 4-5 weeks

---

## Milestone Deliverables

### M1: Embeddings (Week 3)
- ✅ #001, #002, #003 complete
- Can generate and store embeddings
- Can search via cosine similarity
- **Demo:** Embed and search OpenAgents files

### M2: SearchKit (Week 6)
- ✅ #006, #007 complete
- Hybrid search working
- FTS5 + semantic fusion
- **Demo:** Search OpenAgents codebase with hybrid queries

### M3: IssueAgent Core (Week 9)
- ✅ #016, #017, #018 complete
- Can process issues → patches
- Agent delegation working
- **Demo:** Process a real GitHub issue, generate patches

### M4: End-to-End (Week 10)
- ✅ #020 complete
- Full workflow: issue → patches → PR
- Progress streaming via ACP
- **Demo:** End-to-end issue resolution

---

## Testing Strategy by Phase

### Phase 1 Tests
- Unit tests for provider, store, service
- Performance tests (embedding throughput)
- Integration tests (embed + search)

### Phase 2 Tests
- Unit tests for FTS5, RRF fusion
- Golden tests for hybrid search
- Performance tests (search latency)

### Phase 3 Tests
- Unit tests for enhancer, retrieval, delegator
- Mock agent tests
- Golden tests for patch generation

### Phase 4 Tests
- End-to-end integration tests
- Real GitHub issue tests
- Performance tests (full workflow)
- Stress tests (multiple concurrent issues)

---

## Risk Mitigation

### High-Risk Items

1. **MLX Model Download (#001)**
   - Risk: First-time download may fail
   - Mitigation: Retry logic, fallback to cached model

2. **Agent Delegation (#018)**
   - Risk: External agents (Claude Code, Codex) may be unavailable
   - Mitigation: Fallback to Foundation Models, clear error handling

3. **Patch Applicability (#018)**
   - Risk: Before blocks may not exactly match
   - Mitigation: Strict validation, fuzzy matching (future)

### Medium-Risk Items

1. **Performance Targets**
   - Risk: Hybrid search may be too slow for large codebases
   - Mitigation: Optimize queries, add ANN index if needed

2. **GitHub API Rate Limits**
   - Risk: Heavy usage may hit rate limits
   - Mitigation: Caching, rate limit detection, backoff

---

## Success Criteria

### Phase 1 Success
- [ ] Can embed 100 files in <10s
- [ ] Can search 10k vectors in <500ms
- [ ] Embeddings are L2-normalized

### Phase 2 Success
- [ ] Hybrid search achieves >80% recall@10
- [ ] Search latency <700ms p95
- [ ] RRF fusion improves over lexical-only

### Phase 3 Success
- [ ] Issue enhancement extracts useful metadata
- [ ] Retrieval finds correct files (>80% recall)
- [ ] Patches are syntactically valid (>90%)

### Phase 4 Success
- [ ] End-to-end workflow <90s
- [ ] At least 1 successful PR from real issue
- [ ] Progress streaming works

---

## Next Steps

1. **Assign issues** to developers
2. **Set up tracking** (GitHub Projects, Linear, etc.)
3. **Start with #001** (MLX Embedding Provider)
4. **Weekly check-ins** to review progress
5. **Demo milestones** as they complete

---

## Questions & Decisions

### Open Questions

1. **Agent Credentials:** How to securely store Claude Code/Codex API keys?
   - Recommendation: macOS Keychain

2. **Fork Strategy:** Use shared bot account or user's personal fork?
   - Recommendation: User's personal account with OAuth

3. **Test Execution:** Run tests before creating PRs?
   - Recommendation: Async GitHub Actions checks

4. **Golden Corpus:** Where to get real issues for testing?
   - Recommendation: OpenAgents repo historical issues

### Decisions Needed

- [ ] Choose primary agent (Claude Code vs Codex)
- [ ] Decide on auto-PR vs review-required default
- [ ] Set performance SLAs for production
- [ ] Define success metrics for acceptance

---

## Summary

**Total Issues:** 10
**Estimated Timeline:** 8-10 weeks (solo), 4-5 weeks (3 developers)
**Critical Path:** 13-15 working days
**Parallel Opportunities:** Significant (FTS5, Issue Enhancer)

**Recommended Start:**
1. Begin with #001 (MLX Provider)
2. Start #006 (FTS5) in parallel immediately
3. Start #016 (Issue Enhancer) in week 2

**Key Risks:**
- MLX model availability
- Agent delegation reliability
- Patch applicability

**Mitigation:**
- Robust error handling
- Fallback strategies
- Extensive testing

Good luck! 🚀
