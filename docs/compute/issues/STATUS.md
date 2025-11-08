# OpenAgents Compute Marketplace Issues - Status

**Last Updated**: 2025-11-07
**Total Issues**: 31 planned (1 deleted, 3 new, net +2)
**Completed (Draft)**: 19 of 31 (All Phase 1 [minus deleted #003] + Phase 2 complete + key Phase 3-4)

## Summary

This directory contains draft GitHub issues for implementing the OpenAgents compute marketplace. The issues are based on the comprehensive architecture analysis in `/Users/christopherdavid/code/openagents/docs/compute/apple-terms-research.md`.

## Progress

### ✅ Completed Draft Issues (19 total)

#### Phase 1: MVP - COMPLETE (8/8 after deletion)
- **✅ 001**: Nostr Client Library (Fork Integration) (~6000 words) - P0, **2-3w** (reduced from 4-6w via nostr-sdk-ios fork)
- **✅ 002**: Secp256k1 & Cryptography (~5500 words) - P0, 2-3w
- **❌ 003**: ~~BOLT11 & Lightning Primitives~~ - **DELETED** (Spark SDK replaces)
- **✅ 004**: Job Schema Registry (~4000 words) - P1, 1-2w
- **✅ 005**: iOS Nostr Identity (~4000 words) - P0, 2-3w
- **✅ 006**: iOS Marketplace Viewer (~3500 words) - P1, 1-2w
- **✅ 007**: macOS Foundation Models Worker (~5000 words) - P0, 3-4w
- **✅ 008**: macOS Capability Advertising (~3500 words) - P1, 1w
- **✅ 009**: Policy & Safety Module (~4500 words) - P0, 2-3w

#### Phase 2: Payments - COMPLETE (7/7 with Spark SDK)
- **✅ 010**: iOS Wallet with Spark SDK (~5000 words) - P0, 2-3w (rewrote)
- **✅ 011**: iOS Job Creation & Submission (~3000 words) - P0, 2-3w
- **✅ 012**: Breez API Key & SDK Config (~3000 words) - P1, 2-3d (new)
- **✅ 013**: macOS Wallet with Spark SDK (~5000 words) - P0, 1-2w (rewrote)
- **✅ 014**: macOS Bidding Engine (~2500 words) - P1, 2w
- **✅ 015**: Marketplace Payment Coordinator (~4500 words) - P0, 1-2w (new)
- **✅ 016**: Seed Backup & Recovery (~3500 words) - P1, 3-5d (new)

#### Phase 3: Backends - KEY ISSUES (2/7)
- **✅ 017**: macOS MLX Integration (~3000 words) - P1, 3-4w
- **✅ 020**: macOS Model Router (~2500 words) - P1, 2-3w

#### Phase 4: SearchKit - COMPLETE (1/1)
- **✅ 024**: SearchKit MVP (~3500 words) - P2, 6-8w (Deferrable)

#### Documentation - KEY ISSUE (1/4)
- **✅ 025**: ADRs: Marketplace Architecture (~2500 words) - P1, 1-2w

#### Testing - KEY ISSUE (1/3)
- **✅ 029**: Unit Test Suite (~2500 words) - P0, 3-4w

### 📝 Remaining Issues

#### Phase 1: MVP (3 remaining)
- [ ] **006**: iOS Marketplace Viewer (Read-Only)
- [ ] **007**: macOS Foundation Models Worker
- [ ] **008**: macOS Capability Advertising (NIP-89)

#### Phase 2: Payments (7 issues)
- [ ] **010**: iOS Bitcoin/Lightning Wallet
- [ ] **011**: iOS Job Creation & Submission
- [ ] **012**: iOS Payment Flows
- [ ] **013**: macOS Lightning Integration
- [ ] **014**: macOS Bidding Engine
- [ ] **015**: iOS Active Job Management
- [ ] **016**: iOS Provider Dashboard

#### Phase 3: Backends (7 issues)
- [ ] **017**: macOS MLX/Swift-MLX Integration
- [ ] **018**: macOS Ollama Integration
- [ ] **019**: macOS llama.cpp Integration
- [ ] **020**: macOS Model Router
- [ ] **021**: macOS Resource Management
- [ ] **022**: Reputation System
- [ ] **023**: macOS Observability Dashboard

#### Phase 4: SearchKit (1 issue)
- [ ] **024**: SearchKit MVP (Epic)

#### Documentation (4 issues)
- [ ] **025**: ADRs - Marketplace Architecture
- [ ] **026**: Integration Guides
- [ ] **027**: Protocol Specifications
- [ ] **028**: User Documentation

#### Testing (3 issues)
- [ ] **029**: Unit Test Suite
- [ ] **030**: Integration & E2E Tests
- [ ] **031**: Security & Compliance Testing

## Issue Quality Standards

Each completed issue includes:

✅ **Complete technical spec** with:
- Summary and motivation
- Detailed acceptance criteria (comprehensive checklists)
- Full technical design (Swift types, APIs, data models)
- Dependencies clearly stated
- Testing requirements (unit, integration, E2E)
- Apple compliance considerations (ASRG, DPLA, AUP)
- Reference links (NIPs, Apple docs, related issues)
- Success metrics
- Future enhancements

✅ **Estimated ~3000-6000 words per issue** for thorough specification

✅ **Ready for GitHub publishing** via `gh issue create`

## Completed Issues Stats

| Category | Issues | Words | LOC Est | Effort |
|----------|--------|-------|---------|--------|
| Phase 1 (MVP) | 8 (was 9, deleted #003) | ~36,000 | ~3,000 | **16-24w** (reduced from 18-26w) |
| Phase 2 (Spark SDK) | 7 (complete) | ~27,500 | ~1,500 | 10.5-15w |
| Phase 3 (Key) | 2 | ~5,500 | ~500 | 5-7w |
| Phase 4 (SearchKit) | 1 | ~3,500 | ~2,000 | 6-8w |
| Documentation | 1 | ~2,500 | - | 1-2w |
| Testing | 1 | ~2,500 | - | 3-4w |
| **TOTAL** | **19** | **~75,000** | **~7,000** | **42-59w** (reduced from 44-61w) |

**Parallelized (3 engineers)**: ~14-18 weeks for Phases 1-3 (reduced from 17-21w via nostr-sdk-ios fork + Spark SDK)

## Next Steps

### ✅ Completed
1. ~~Create all Phase 1 issues~~ - **DONE**
2. ~~Delete issue #003~~ - **DONE** (Spark SDK replaces BOLT11)
3. ~~Rewrite Phase 2 wallet issues with Spark SDK~~ - **DONE** (#010, #013)
4. ~~Create new Spark SDK issues~~ - **DONE** (#012, #015, #016)
5. ~~Update issue #001 for nostr-sdk-ios fork~~ - **DONE** (effort reduced 4-6w → 2-3w)

### Immediate (User Review)
1. **Review Spark SDK integration** (see `SPARK-SDK-INTEGRATION.md`)
2. **Review all 19 completed issues** for completeness
3. **Approve or request changes**

### Short-term (Implementation)
4. **Assign issues to engineers** (3-person team recommended)
5. **Create GitHub milestones**: Phase 1, Phase 2, Phase 3, Phase 4
6. **Begin implementation** (start with #012 API Key Config)

### Publishing
7. Convert approved issues to GitHub via `gh issue create`
8. Add labels: priority (p0/p1/p2), component (ios/macos/shared), phase, spark-sdk
9. Link to Breez Spark SDK documentation in issues

## Key Insights from Research

### Architecture (from apple-terms-research.md)
- **iOS**: Coordination + wallet + agent management (NO worker compute)
- **macOS**: Worker runtime (Foundation Models + MLX + Ollama)
- **Nostr**: Open marketplace (NIP-90 + NIP-57)
- **Spark SDK**: Layer 2 Bitcoin protocol (replaces manual Lightning)
- **Compliance**: Strict Apple rules enforced (no background workers, AUP filters)

### nostr-sdk-ios Fork Decision (Phase 1 Optimization)

**Why fork nostr-sdk-ios?**
- ✅ **Stale upstream**: Official SDK hasn't had release since February 2025 (9 months stale)
- ✅ **Effort reduction**: ~40% reduction in issue #001 (4-6w → 2-3w)
- ✅ **25+ NIPs implemented**: Core protocol, relay management, encryption, bech32 already done
- ✅ **Rapid iteration**: Add marketplace NIPs (NIP-57, NIP-89, NIP-90) without waiting for upstream
- ✅ **Shallow fork**: Only ~850 LOC additions, can still pull upstream updates

**What Changed?**
- **✏️ Rewrote**: Issue #001 (Nostr Client Library) - fork integration instead of building from scratch
- **Effort saved**: 2-3 weeks in Phase 1 critical path
- **What to add**: NIP-57 (Zaps), NIP-89 (Handlers), NIP-90 (DVM) - marketplace-specific event types

**Fork**: https://github.com/OpenAgentsInc/nostr-sdk-ios

### Spark SDK Decision (Phase 2 Redesign)

**Why Spark SDK?**
- ✅ **User directive**: "We're going to basically use Spark via Breez for all of our Bitcoin and Lightning stuff"
- ✅ **Effort reduction**: ~35-40% reduction in Phase 2 (10-13w → 5.5-9.5w)
- ✅ **Production-ready**: Breez maintains SDK, node infrastructure, LSP
- ✅ **Better UX**: Nodeless operation, offline receive, instant sends
- ✅ **No manual BOLT11**: SDK handles all Lightning protocol details

**What Changed?**
- **❌ Deleted**: Issue #003 (BOLT11 & Lightning Primitives) - 2-3 weeks saved
- **✏️ Rewrote**: Issues #010, #013 (iOS/macOS Wallets) - simplified with Spark SDK
- **🆕 Created**: Issues #012 (API Key Config), #015 (Payment Coordinator), #016 (Seed Backup)

**See**: `SPARK-SDK-INTEGRATION.md` for full integration plan

### Critical Dependencies
```
Crypto (002) → Nostr (001) → Everything Else
API Key (012) → Spark SDK Wallets (010, 013)
Schemas (004) → Worker (007), Marketplace (006)
Policy (009) → Worker (007) - AUP enforcement
Payment Coordinator (015) → Job Execution (depends on 010, 013, 011, 014)
```

### Timeline Estimates
- **Phase 1** (MVP): 6-8 weeks with 3 engineers
- **Phase 2** (Payments): 4-5 weeks (reduced from 5-6 via Spark SDK)
- **Phase 3** (Backends): 4-5 weeks
- **Total**: ~14-18 weeks for full marketplace (Phases 1-3)
  - **Reduced from 17-21 weeks** via:
    - nostr-sdk-ios fork (Phase 1: 2-3w savings)
    - Spark SDK (Phase 2: 4-5w savings)

## Compliance Summary

All completed issues address Apple compliance:

### App Store Review Guidelines
- ✅ **ASRG 2.4.2**: No background mining (iOS = coordination only)
- ✅ **ASRG 2.5.2**: No downloaded code (Nostr events = data)
- ✅ **ASRG 2.5.4**: Background limits (no ambient workers)
- ✅ **ASRG 3.1.1/3.1.3**: Payment flows (IAP vs non-IAP)
- ✅ **ASRG 3.1.5(i)**: Cryptocurrency wallets (Organization dev required)

### DPLA & Foundation Models AUP
- ✅ **DPLA §3.3.8(I)**: Foundation Models AUP compliance
- ✅ **AUP Enforcement**: Issue #009 Policy & Safety Module
- ❌ **Prohibited**: Regulated healthcare/legal/finance, academic textbooks
- ✅ **Allowed**: Summarization, code gen, Q&A (non-regulated)

## Questions / Feedback

- **Scope**: Are these issues comprehensive enough?
- **Priorities**: Should any issues be promoted/demoted in priority?
- **Deferred**: Should SearchKit (#024) be fully deferred until post-launch?
- **Additional**: Any missing issues or concerns?

## Files

```
docs/compute/issues/
├── README.md                    # Master index (updated for Spark SDK)
├── STATUS.md                    # This file (progress tracking)
├── COMPLETED.md                 # Delivery summary
├── SPARK-SDK-INTEGRATION.md     # 🆕 Spark SDK integration plan
├── phase-1-mvp/
│   ├── 001-nostr-client-library.md          ✅ COMPLETE
│   ├── 002-secp256k1-crypto.md              ✅ COMPLETE
│   ├── ❌ 003-bolt11-lightning-primitives.md   **DELETED**
│   ├── 004-job-schema-registry.md           ✅ COMPLETE
│   ├── 005-ios-nostr-identity.md            ✅ COMPLETE
│   ├── 006-ios-marketplace-viewer.md        ✅ COMPLETE
│   ├── 007-macos-foundation-models-worker.md ✅ COMPLETE
│   ├── 008-macos-capability-advertising.md  ✅ COMPLETE
│   └── 009-policy-safety-module.md          ✅ COMPLETE
├── phase-2-payments/            ✅ COMPLETE (7/7 with Spark SDK)
│   ├── 010-ios-wallet-spark-sdk.md         ✅ COMPLETE (rewrote)
│   ├── 011-ios-job-creation-submission.md  ✅ COMPLETE
│   ├── 012-breez-api-key-config.md         ✅ COMPLETE (new)
│   ├── 013-macos-wallet-spark-sdk.md       ✅ COMPLETE (rewrote)
│   ├── 014-macos-bidding-engine.md         ✅ COMPLETE
│   ├── 015-marketplace-payment-coordinator.md ✅ COMPLETE (new)
│   └── 016-seed-backup-recovery.md         ✅ COMPLETE (new)
├── phase-3-backends/            ⏳ 2 of 7 (key issues)
│   ├── 017-macos-mlx-integration.md        ✅ COMPLETE
│   └── 020-macos-model-router.md           ✅ COMPLETE
├── phase-4-searchkit/           ✅ 1 of 1
│   └── 024-searchkit-mvp.md                ✅ COMPLETE
├── documentation/               ✅ 1 of 4
│   └── 025-adrs-marketplace.md             ✅ COMPLETE
└── testing/                     ✅ 1 of 3
    └── 029-unit-test-suite.md              ✅ COMPLETE
```

---

**Ready for review**: All 19 completed issues
**Key integration**: Spark SDK (see `SPARK-SDK-INTEGRATION.md`)
**Next**: User approval → GitHub publishing → Implementation (14-18 weeks for Phases 1-3)
