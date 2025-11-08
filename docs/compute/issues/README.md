# OpenAgents Compute Marketplace - GitHub Issues

This directory contains draft GitHub issues for implementing the OpenAgents compute marketplace based on the architecture defined in `docs/compute/apple-terms-research.md`.

## Organization

Issues are organized into phases aligned with the phased rollout strategy:

- **Phase 1 (MVP)**: Foundation + Basic Marketplace
- **Phase 2 (Payments)**: Wallet + Lightning Integration
- **Phase 3 (Backends)**: Model Diversity (MLX, Ollama)
- **Phase 4 (SearchKit)**: Advanced Agent Capabilities

## Phase Overview

### Phase 1: MVP (Foundation) - 8 Issues
**Goal**: Prove marketplace concept with iOS coordination + macOS Foundation Models worker

| # | Issue | Priority | Effort | Status |
|---|-------|----------|--------|--------|
| 001 | Nostr Client Library | P0 | 4-6w | ✅ Draft |
| 002 | Secp256k1 & Cryptography | P0 | 2-3w | ✅ Draft |
| ~~003~~ | ~~BOLT11 & Lightning Primitives~~ | ~~P0~~ | ~~2-3w~~ | ❌ **DELETED** (replaced by Spark SDK) |
| 004 | Job Schema Registry | P1 | 1-2w | ✅ Draft |
| 005 | iOS: Nostr Identity & Key Management | P0 | 2-3w | ✅ Draft |
| 006 | iOS: Marketplace Viewer (Read-Only) | P1 | 1-2w | ✅ Draft |
| 007 | macOS: Foundation Models Worker | P0 | 3-4w | ✅ Draft |
| 008 | macOS: Capability Advertising (NIP-89) | P1 | 1w | ✅ Draft |
| 009 | Policy & Safety Module (AUP) | P0 | 2-3w | ✅ Draft |

**Total Estimated Effort**: ~18-26 weeks (sequential) | ~6-8 weeks (with 2-3 engineers)
**Note**: Issue #003 deleted - Breez Spark SDK replaces manual BOLT11 implementation (see `SPARK-SDK-INTEGRATION.md`)

### Phase 2: Payments (Economic Loop) - 6 Issues
**Goal**: Enable full marketplace with Spark SDK wallet + payments

| # | Issue | Priority | Effort | Status |
|---|-------|----------|--------|--------|
| 010 | iOS: Wallet with Spark SDK | P0 | 2-3w | ✅ Draft (rewrote) |
| 011 | iOS: Job Creation & Submission | P0 | 2-3w | ✅ Draft |
| 012 | Breez API Key & SDK Configuration | P1 | 2-3d | ✅ Draft (new) |
| 013 | macOS: Wallet with Spark SDK | P0 | 1-2w | ✅ Draft (rewrote) |
| 014 | macOS: Bidding Engine | P1 | 2w | ✅ Draft |
| 015 | Marketplace Payment Coordinator | P0 | 1-2w | ✅ Draft (new) |
| 016 | Seed Backup & Recovery UI | P1 | 3-5d | ✅ Draft (new) |

**Total Estimated Effort**: ~10.5-15 weeks (sequential) | ~4-5 weeks (parallelized)
**Effort Savings**: ~35-40% reduction vs manual Lightning implementation (see `SPARK-SDK-INTEGRATION.md`)

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
├── ❌ 003 (BOLT11) - DELETED (Spark SDK)
├── 004 (Schemas) ← 001 (Nostr)
├── 005 (iOS Nostr) ← 001, 002
├── 006 (iOS Viewer) ← 001, 004, 005
├── 007 (macOS Worker) ← 001, 004, 009
├── 008 (Capabilities) ← 001, 004
└── 009 (Policy/AUP) ← (independent)

Phase 2 Payments (Spark SDK):
├── 012 (API Key Config) ← (foundational)
├── 010 (iOS Wallet) ← 002, 012
├── 013 (macOS Wallet) ← 002, 012, 010 (shares core)
├── 016 (Seed Backup) ← 010, 013
├── 011 (Job Creation) ← 001, 004, 005, 010
├── 014 (Bidding) ← 007, 013
└── 015 (Payment Coordinator) ← 010, 013, 011, 014

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
3. ~~**003** BOLT11 & Lightning Primitives~~ [**DELETED** - Spark SDK replaces this]
4. **004** Job Schema Registry (1-2w) [depends on 001]
5. **009** Policy & Safety Module (2-3w) [independent, can parallelize]
6. **005** iOS Nostr Identity (2-3w) [depends on 001, 002]
7. **007** macOS Worker (3-4w) [depends on 001, 004, 009]
8. **006** iOS Viewer (1-2w) [depends on 001, 004, 005]
9. **008** Capability Advertising (1w) [depends on 001, 004]

**Sequential**: 18-26 weeks (reduced from 20-29 weeks)
**Parallelized** (3 engineers): ~6-8 weeks

## Timeline Estimates

### With 3 Engineers (Recommended)

**Phase 1**: 6-8 weeks
- Engineer 1: Crypto → Nostr → iOS Nostr/Viewer
- Engineer 2: ~~BOLT11~~ → Job Schemas → Capability Advertising
- Engineer 3: Policy/AUP → macOS Worker

**Phase 2**: 4-5 weeks (reduced from 5-6 weeks via Spark SDK)
- Engineer 1: API Key Config → iOS Wallet (Spark SDK) → Seed Backup
- Engineer 2: macOS Wallet (Spark SDK) → Bidding Engine
- Engineer 3: iOS Job Creation → Payment Coordinator

**Phase 3**: 4-5 weeks
- Engineer 1: MLX Integration → Model Router
- Engineer 2: Ollama Integration → Resource Management
- Engineer 3: llama.cpp → Reputation → Observability

**Phase 4** (Optional): 6-8 weeks
- Engineer 1 or 2: SearchKit (can overlap with Phase 3)

**Total**: ~14-18 weeks for Phases 1-3 (reduced from 15-19 weeks)

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
- [x] Phase 1 issues 001-009 created (draft)
- [x] **Issue #003 deleted** (Spark SDK replaces BOLT11)
- [x] Phase 2 issues rewritten with Spark SDK integration
  - [x] #010 iOS Wallet (rewrote)
  - [x] #011 iOS Job Creation (complete)
  - [x] #012 API Key Config (new)
  - [x] #013 macOS Wallet (rewrote)
  - [x] #014 macOS Bidding Engine (complete)
  - [x] #015 Payment Coordinator (new)
  - [x] #016 Seed Backup/Recovery (new)
- [x] Phase 3 key issues 017, 020 (MLX, Router)
- [x] Phase 4 issue 024 (SearchKit)
- [x] Documentation issue 025 (ADRs)
- [x] Testing issue 029 (Unit Tests)
- [x] Spark SDK integration summary created
- [ ] Phase 2-4 remaining issues (can create when needed)
- [ ] User review and approval
- [ ] Publish to GitHub

**Key Achievements**:
- 16 comprehensive issues completed (~55,000 words)
- Spark SDK integration reduces effort by ~35-40%
- All critical path issues specified

Last updated: 2025-11-07
