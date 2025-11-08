# OpenAgents Compute Marketplace - GitHub Issues

This directory contains draft GitHub issues for implementing the OpenAgents compute marketplace based on the architecture defined in `docs/compute/apple-terms-research.md`.

## Organization

Issues are organized into phases aligned with the phased rollout strategy:

- **Phase 1 (MVP)**: Foundation + Basic Marketplace
- **Phase 2 (Payments)**: Wallet + Lightning Integration
- **Phase 3 (Backends)**: Model Diversity (MLX, Ollama)
- **Phase 4 (SearchKit)**: Advanced Agent Capabilities

## Phase Overview

### Phase 1: MVP (Foundation) - 9 Issues
**Goal**: Prove marketplace concept with iOS coordination + macOS Foundation Models worker

| # | Issue | Priority | Effort | Status |
|---|-------|----------|--------|--------|
| 001 | Nostr Client Library | P0 | 4-6w | Draft |
| 002 | Secp256k1 & Cryptography | P0 | 2-3w | Draft |
| 003 | BOLT11 & Lightning Primitives | P0 | 2-3w | Draft |
| 004 | Job Schema Registry | P1 | 1-2w | Draft |
| 005 | iOS: Nostr Identity & Key Management | P0 | 2-3w | Draft |
| 006 | iOS: Marketplace Viewer (Read-Only) | P1 | 1-2w | Draft |
| 007 | macOS: Foundation Models Worker | P0 | 3-4w | Draft |
| 008 | macOS: Capability Advertising (NIP-89) | P1 | 1w | Draft |
| 009 | Policy & Safety Module (AUP) | P0 | 2-3w | Draft |

**Total Estimated Effort**: ~20-29 weeks (sequential) | ~6-8 weeks (with 2-3 engineers)

### Phase 2: Payments (Economic Loop) - 7 Issues
**Goal**: Enable full marketplace with wallet + payments

| # | Issue | Priority | Effort |
|---|-------|----------|--------|
| 010 | iOS: Bitcoin/Lightning Wallet | P0 | 4-5w |
| 011 | iOS: Job Creation & Submission | P0 | 2-3w |
| 012 | iOS: Payment Flows | P0 | 2-3w |
| 013 | macOS: Lightning Integration | P0 | 4-5w |
| 014 | macOS: Bidding Engine | P1 | 2w |
| 015 | iOS: Active Job Management | P1 | 2w |
| 016 | iOS: Provider Dashboard | P1 | 2w |

**Total Estimated Effort**: ~18-23 weeks | ~5-6 weeks (parallelized)

### Phase 3: Backends (Compute Diversity) - 7 Issues
**Goal**: Multi-backend model routing

| # | Issue | Priority | Effort |
|---|-------|----------|--------|
| 017 | macOS: MLX/Swift-MLX Integration | P1 | 3-4w |
| 018 | macOS: Ollama Integration | P1 | 2-3w |
| 019 | macOS: llama.cpp Integration | P2 | 2-3w |
| 020 | macOS: Model Router | P1 | 2-3w |
| 021 | macOS: Resource Management | P0 | 2w |
| 022 | Reputation System | P2 | 2-3w |
| 023 | macOS: Observability Dashboard | P2 | 3w |

**Total Estimated Effort**: ~16-21 weeks | ~4-5 weeks (parallelized)

### Phase 4: SearchKit (Advanced Capabilities) - 1 Issue
**Goal**: Hybrid search for better agents

| # | Issue | Priority | Effort |
|---|-------|----------|--------|
| 024 | SearchKit MVP (Epic) | P2 | 6-8w |

**Note**: Can be deferred until marketplace validates demand for advanced search

### Cross-Phase: Documentation - 4 Issues

| # | Issue | Priority | Effort |
|---|-------|----------|--------|
| 025 | ADRs: Marketplace Architecture | P1 | 1-2w |
| 026 | Integration Guides | P2 | 1-2w |
| 027 | Protocol Specifications | P1 | 1w |
| 028 | User Documentation | P2 | 1-2w |

### Cross-Phase: Testing - 3 Issues

| # | Issue | Priority | Effort |
|---|-------|----------|--------|
| 029 | Unit Test Suite | P0 | 3-4w |
| 030 | Integration & E2E Tests | P0 | 3-4w |
| 031 | Security & Compliance Testing | P0 | 2-3w |

## Dependency Graph

```
Phase 1 Foundation:
├── 001 (Nostr) ← 002 (Crypto)
├── 003 (BOLT11) ← 002 (Crypto)
├── 004 (Schemas) ← 001 (Nostr)
├── 005 (iOS Nostr) ← 001, 002
├── 006 (iOS Viewer) ← 001, 004, 005
├── 007 (macOS Worker) ← 001, 004, 009
├── 008 (Capabilities) ← 001, 004
└── 009 (Policy/AUP) ← (independent)

Phase 2 Payments:
├── 010 (Wallet) ← 002, 003
├── 011 (Job Creation) ← 001, 004, 005, 010
├── 012 (Payments) ← 010, 003
├── 013 (macOS Lightning) ← 003, 007
├── 014 (Bidding) ← 007, 013
├── 015 (Job Mgmt) ← 011, 012
└── 016 (Provider Dashboard) ← 013, 014

Phase 3 Backends:
├── 017 (MLX) ← 007
├── 018 (Ollama) ← 007
├── 019 (llama.cpp) ← 007
├── 020 (Router) ← 017, 018, 019
├── 021 (Resources) ← 007, 020
├── 022 (Reputation) ← 001, 014
└── 023 (Observability) ← 007, 020

Phase 4 SearchKit:
└── 024 (SearchKit) ← 007 (can run parallel to Phases 2-3)
```

## Critical Path

The critical path for marketplace MVP (Phase 1 complete):

1. **002** Secp256k1 & Cryptography (2-3w)
2. **001** Nostr Client Library (4-6w) [depends on 002]
3. **003** BOLT11 & Lightning Primitives (2-3w) [depends on 002]
4. **004** Job Schema Registry (1-2w) [depends on 001]
5. **009** Policy & Safety Module (2-3w) [independent, can parallelize]
6. **005** iOS Nostr Identity (2-3w) [depends on 001, 002]
7. **007** macOS Worker (3-4w) [depends on 001, 004, 009]
8. **006** iOS Viewer (1-2w) [depends on 001, 004, 005]
9. **008** Capability Advertising (1w) [depends on 001, 004]

**Sequential**: 20-29 weeks
**Parallelized** (3 engineers): ~6-8 weeks

## Timeline Estimates

### With 3 Engineers (Recommended)

**Phase 1**: 6-8 weeks
- Engineer 1: Crypto → Nostr → iOS Nostr/Viewer
- Engineer 2: BOLT11 → Job Schemas → Capability Advertising
- Engineer 3: Policy/AUP → macOS Worker

**Phase 2**: 5-6 weeks
- Engineer 1: iOS Wallet → Payment Flows → Job Management
- Engineer 2: macOS Lightning → Bidding Engine
- Engineer 3: iOS Job Creation → Provider Dashboard

**Phase 3**: 4-5 weeks
- Engineer 1: MLX Integration → Model Router
- Engineer 2: Ollama Integration → Resource Management
- Engineer 3: llama.cpp → Reputation → Observability

**Phase 4** (Optional): 6-8 weeks
- Engineer 1 or 2: SearchKit (can overlap with Phase 3)

**Total**: ~15-19 weeks for Phases 1-3 (marketplace through backend diversity)

## Apple Compliance Notes

All issues include an "Apple Compliance Considerations" section addressing:

- **ASRG 2.4.2**: No background mining/compute (iOS = coordination only)
- **ASRG 2.5.2**: No downloaded code execution
- **ASRG 2.5.4**: Background execution limits
- **ASRG 3.1.1/3.1.3**: Payment flows (IAP vs non-IAP)
- **ASRG 3.1.5(i)**: Cryptocurrency wallet requirements (Organization developer)
- **DPLA §3.3.8(I)**: Foundation Models Acceptable Use Policy
- **AUP**: Prohibited job types (regulated health/legal/finance, guardrail circumvention)

Key compliance strategies:
- iOS does **no worker compute** (only coordination)
- macOS runs all providers (Foundation Models, MLX, Ollama)
- Payments for in-app consumption → web/desktop redirect (avoid IAP issues)
- Policy module enforces AUP for all marketplace jobs
- Bitcoin wallet allowed (Organization developer only)

## Publishing to GitHub

Once these draft issues are reviewed and approved:

```bash
# Navigate to each phase directory and create issues
cd docs/compute/issues/phase-1-mvp

# Example: Create issue #001
gh issue create \
  --title "Nostr Client Library (Swift)" \
  --body-file 001-nostr-client-library.md \
  --label "phase-1,foundation,p0" \
  --milestone "Phase 1: MVP"

# Repeat for all issues
```

## Contributing

These issues are the foundation for the OpenAgents compute marketplace. Contributions welcome:

1. Review draft issues for completeness
2. Suggest additional job schemas (issue #004)
3. Identify missing dependencies
4. Clarify technical designs
5. Add test cases

## Questions?

- **Architecture**: See `docs/compute/apple-terms-research.md`
- **ADRs**: See `docs/adr/` for existing decisions
- **Foundation Models**: See `docs/foundation-models/` for API docs
- **Engine Spec**: See `docs/engine/spec-v0.2.2.md` for SearchKit

## Status

- [x] Directory structure created
- [x] Phase 1 issues 001-004 created (draft)
- [ ] Phase 1 issues 005-009 (in progress)
- [ ] Phase 2 issues 010-016
- [ ] Phase 3 issues 017-023
- [ ] Phase 4 issue 024
- [ ] Documentation issues 025-028
- [ ] Testing issues 029-031
- [ ] User review and approval
- [ ] Publish to GitHub

Last updated: 2025-11-07
