# OpenAgents Compute Marketplace - GitHub Issues

This directory contains draft GitHub issues for implementing the OpenAgents compute marketplace based on the architecture defined in `docs/compute/apple-terms-research.md`.

## Organization

Issues are organized into phases aligned with the phased rollout strategy:

- **Phase 1 (MVP)**: Foundation + Basic Marketplace
- **Phase 2 (Payments)**: Wallet + Lightning Integration
- **Phase 3 (Backends)**: Model Diversity (MLX, Ollama)
- **Phase 4 (SearchKit)**: Advanced Agent Capabilities

## Why This Order? The Marketplace Build Strategy

We're building a decentralized compute marketplace where anyone can buy or sell AI inference. The phased approach is designed to validate the core concept early, then layer on economic incentives and advanced capabilities.

### Phase 1: Prove the Marketplace Works (6-8 weeks)

**What we build**: iOS app for coordination (marketplace viewer, identity, wallet) + macOS worker app (using Foundation Models backend). No payments yet—just prove the technical foundation works.

**Why start here**: Foundation Models are Apple's on-device LLMs (free, fast, private). By starting with the Foundation Models backend, we avoid the complexity of payment flows and Bitcoin wallets while validating the harder parts: Nostr protocol integration, job schemas, policy enforcement, and multi-platform coordination. iOS handles **no compute**—it's coordination only. macOS runs the actual worker. If the marketplace UX is broken or Nostr relay performance is poor, we learn that *before* investing 10+ weeks in Lightning integration.

**What we learn**: Do users want to buy remote compute? Do macOS users want to sell their idle GPU time? Is Nostr fast enough for job routing? Can we enforce safety policies? Phase 1 answers these questions with minimal investment.

**Key decision**: We're using the **OpenAgents fork of nostr-sdk-ios** instead of building Nostr from scratch. The official SDK hasn't had a release since February 2025 (9 months stale), and we need to add marketplace-specific NIPs (NIP-57 Zaps, NIP-89 Application Handlers, NIP-90 Data Vending Machines). Forking lets us move fast while still leveraging 25+ NIPs already implemented (event signing, relay management, encryption, bech32 encoding).

### Phase 2: Add the Economic Loop (4-5 weeks)

**What we build**: Bitcoin/Lightning wallet using Breez Spark SDK. Now buyers can pay for jobs, and sellers earn sats.

**Why now**: Once Phase 1 proves the marketplace works, payments unlock the economic flywheel. Sellers are incentivized to run workers 24/7. Buyers get access to more providers. Marketplace liquidity increases.

**Key decision**: We're using **Breez Spark SDK** instead of manually implementing Lightning. Spark is a Layer 2 Bitcoin protocol (statechain-based, not Lightning itself) that provides BOLT11 compatibility with better UX (offline receive, instant sends, no channel management). This reduces Phase 2 effort by ~35-40% (10.5-15 weeks vs 15-20 weeks for manual Lightning). The SDK is production-ready and maintained by Breez, so we avoid reinventing wallet infrastructure.

**Why defer payments to Phase 2**: Payments are complex (Bitcoin key management, Secure Enclave, seed backup, BOLT11 invoice parsing, payment coordination). By deferring to Phase 2, we validate marketplace demand first. If Phase 1 shows low engagement, we pivot *before* sinking 15 weeks into wallet code.

### Phase 3: Expand Provider Capabilities (4-5 weeks)

**What we build**: MLX (Apple Silicon-optimized LLMs), Ollama (user-friendly model management), and llama.cpp (low-level control) backends. Multi-backend routing lets providers offer different models at different price points.

**Why now**: With payments working (Phase 2), providers want to differentiate. Some offer fast jobs using the Foundation Models backend, others offer larger custom models via MLX or Ollama backends. The model router picks the right backend for each job based on requirements and bidding.

**Why not Phase 1**: Starting with the Foundation Models backend alone proves the concept. Adding MLX/Ollama in Phase 1 triples complexity without validating marketplace demand. Phase 3 is about scale and differentiation once the marketplace is proven.

### Phase 4: Advanced Capabilities (6-8 weeks, optional)

**What we build**: SearchKit for hybrid search (codebase indexing + RAG). This powers advanced agents that can "search your entire codebase" or "find all usages of this API."

**Why last**: SearchKit is powerful but niche. Most early marketplace jobs are simple (summarization, code generation, Q&A). SearchKit targets power users with large codebases. We build it only if Phase 1-3 show demand for advanced agent capabilities.

**Why optional**: If marketplace adoption focuses on simple jobs, we defer SearchKit indefinitely and focus on scaling what works.

## The Big Picture

This phased approach follows the lean startup principle: **build → measure → learn**. Each phase adds a major capability (marketplace infrastructure, payments, model diversity, advanced search) but *only after* the previous phase validates demand. We're not building a full-featured marketplace upfront—we're iterating toward product-market fit.

## Phase Overview

### Phase 1: MVP (Foundation) - 8 Issues
**Goal**: Prove marketplace concept with iOS coordination + macOS worker (Foundation Models backend)

| # | Issue | Priority | Effort | Status |
|---|-------|----------|--------|--------|
| 001 | Nostr Client Library (Fork Integration) | P0 | 2-3w | ✅ Draft |
| 002 | Secp256k1 & Cryptography | P0 | 2-3w | ✅ Draft |
| ~~003~~ | ~~BOLT11 & Lightning Primitives~~ | ~~P0~~ | ~~2-3w~~ | ❌ **DELETED** (replaced by Spark SDK) |
| 004 | Job Schema Registry | P1 | 1-2w | ✅ Draft |
| 005 | iOS: Nostr Identity & Key Management | P0 | 2-3w | ✅ Draft |
| 006 | iOS: Marketplace Viewer (Read-Only) | P1 | 1-2w | ✅ Draft |
| 007 | macOS: Foundation Models Worker | P0 | 3-4w | ✅ Draft |
| 008 | macOS: Capability Advertising (NIP-89) | P1 | 1w | ✅ Draft |
| 009 | Policy & Safety Module (AUP) | P0 | 2-3w | ✅ Draft |

**Total Estimated Effort**: ~16-24 weeks (sequential) | ~6-8 weeks (with 2-3 engineers)

**Notes**:
- Issue #001 effort reduced from 4-6w to 2-3w by using **OpenAgents fork of nostr-sdk-ios** (25+ NIPs already implemented, just add marketplace NIPs)
- Issue #003 deleted - Breez Spark SDK replaces manual BOLT11 implementation (see `SPARK-SDK-INTEGRATION.md`)

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
2. **001** Nostr Client Library (2-3w) [depends on 002] - **Reduced from 4-6w via nostr-sdk-ios fork**
3. ~~**003** BOLT11 & Lightning Primitives~~ [**DELETED** - Spark SDK replaces this]
4. **004** Job Schema Registry (1-2w) [depends on 001]
5. **009** Policy & Safety Module (2-3w) [independent, can parallelize]
6. **005** iOS Nostr Identity (2-3w) [depends on 001, 002]
7. **007** macOS Worker (3-4w) [depends on 001, 004, 009]
8. **006** iOS Viewer (1-2w) [depends on 001, 004, 005]
9. **008** Capability Advertising (1w) [depends on 001, 004]

**Sequential**: 16-24 weeks (reduced from 20-29 weeks via Spark SDK + nostr-sdk-ios fork)
**Parallelized** (3 engineers): ~6-8 weeks

## Timeline Estimates

### With 3 Engineers (Recommended)

**Phase 1**: 6-8 weeks
- Engineer 1: Crypto → **Nostr (fork integration)** → iOS Nostr/Viewer
- Engineer 2: Job Schemas → Capability Advertising
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

**Total**: ~14-18 weeks for Phases 1-3 (reduced from 17-21 weeks via nostr-sdk-ios fork + Spark SDK)

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
- iOS does **no worker compute** (only coordination, marketplace viewer, wallet, identity)
- macOS runs the worker with multiple backends (Foundation Models, MLX, Ollama, llama.cpp)
- Payments for in-app consumption → web/desktop redirect (avoid IAP issues)
- Policy module enforces AUP for all marketplace jobs (especially Foundation Models AUP)
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
- [x] **Issue #001 updated** to use nostr-sdk-ios fork (effort reduced 4-6w → 2-3w)
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
- [x] Added "Why This Order?" strategy explanation to README
- [ ] Phase 2-4 remaining issues (can create when needed)
- [ ] User review and approval
- [ ] Publish to GitHub

**Key Achievements**:
- 19 comprehensive issues completed (~75,000 words)
- **nostr-sdk-ios fork** integration reduces Phase 1 effort by ~40% (issue #001: 4-6w → 2-3w)
- **Spark SDK** integration reduces Phase 2 effort by ~35-40% (10.5-15w vs 15-20w manual)
- **Total effort savings**: Reduced from 17-21 weeks to 14-18 weeks for Phases 1-3
- All critical path issues specified with clear rationale

Last updated: 2025-11-07
