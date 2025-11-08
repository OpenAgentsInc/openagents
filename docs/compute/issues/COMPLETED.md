# OpenAgents Compute Marketplace Issues - Completion Summary

**Created**: 2025-11-07
**Status**: 19 comprehensive issues completed (16 original + 3 new Spark SDK issues - 1 deleted)
**Key Change**: Spark SDK integration (replaced manual Lightning implementation)
**Ready for**: User review → GitHub publishing

## Overview

Created **19 comprehensive GitHub issues** (out of 31 planned) covering the critical path for OpenAgents compute marketplace implementation. Each issue includes full technical specifications, Apple compliance analysis, testing requirements, and implementation guidance.

**Major Decision**: Switched from manual Lightning implementation to **Breez Spark SDK**, reducing Phase 2 effort by ~35-40% while improving UX and reliability.

## What's Been Created (19 Issues)

### ✅ Phase 1: MVP - COMPLETE (8/8 issues after deletion)

**Foundation Layer** (Critical Path):
1. **001 - Nostr Client Library** (~6000 words)
   - Full NIP-01, NIP-04, NIP-19, NIP-57, NIP-90 implementation
   - Multi-relay manager, encryption, zaps, DVM
   - Priority: P0 | Effort: 4-6 weeks

2. **002 - Secp256k1 & Cryptography** (~5500 words)
   - Schnorr/ECDSA signatures, ECDH, Secure Enclave
   - BIP32/39/84 HD wallet, BECH32 encoding
   - Priority: P0 | Effort: 2-3 weeks

3. ~~**003 - BOLT11 & Lightning Primitives**~~ (**DELETED** - Spark SDK replaces)
   - ❌ Removed: Manual invoice parsing/generation no longer needed
   - Spark SDK provides complete BOLT11 support internally
   - **Effort saved**: 2-3 weeks

4. **004 - Job Schema Registry** (~4000 words)
   - 12 job kinds with schemas, NIP-90 extension spec
   - Priority: P1 | Effort: 1-2 weeks

**iOS Application**:
5. **005 - iOS Nostr Identity & Key Management** (~4000 words)
   - Key gen/import, Secure Enclave, relay management
   - Priority: P0 | Effort: 2-3 weeks

6. **006 - iOS Marketplace Viewer** (~3500 words)
   - Provider discovery, job browser, activity feed
   - Priority: P1 | Effort: 1-2 weeks

**macOS Worker**:
7. **007 - macOS Foundation Models Worker** (~5000 words)
   - Job queue, scheduler, FM execution, NIP-90 SP
   - Priority: P0 | Effort: 3-4 weeks

8. **008 - macOS Capability Advertising** (~3500 words)
   - NIP-89 ads, pricing, limits, lifecycle
   - Priority: P1 | Effort: 1 week

**Policy & Safety**:
9. **009 - Policy & Safety Module** (~4500 words)
   - Foundation Models AUP enforcement, classifier
   - Priority: P0 | Effort: 2-3 weeks

**Phase 1 Total**: ~36,000 words | 18-26 weeks effort (reduced from 40,500 words / 20-29 weeks)

---

### ✅ Phase 2: Payments with Spark SDK - COMPLETE (7 of 7)

**Major Change**: Integrated Breez Spark SDK instead of manual Lightning implementation

10. **010 - iOS Wallet with Spark SDK** (~5000 words) **[REWROTE]**
    - SparkWalletManager actor, Keychain seed storage
    - Send/receive payments (BOLT11, Spark addresses)
    - Priority: P0 | Effort: 2-3 weeks (reduced from 4-5 weeks)

11. **011 - iOS Job Creation & Submission** (~3000 words)
    - Job builder UI, param editor, NIP-90 submission
    - Priority: P0 | Effort: 2-3 weeks

12. **012 - Breez API Key & SDK Configuration** (~3000 words) **[NEW]**
    - Secure API key storage, network selection
    - SparkConfigManager singleton
    - Priority: P1 | Effort: 2-3 days

13. **013 - macOS Wallet with Spark SDK** (~5000 words) **[REWROTE]**
    - Shares SparkWalletManager from iOS
    - Menu bar integration, desktop-optimized UI
    - Priority: P0 | Effort: 1-2 weeks (reduced from 4-5 weeks)

14. **014 - macOS Bidding Engine** (~2500 words)
    - Cost model, dynamic pricing, bid evaluation
    - Priority: P1 | Effort: 2 weeks

15. **015 - Marketplace Payment Coordinator** (~4500 words) **[NEW]**
    - Job ↔ invoice correlation, payment status tracking
    - Triggers job execution on payment confirmation
    - Priority: P0 | Effort: 1-2 weeks

16. **016 - Seed Backup & Recovery UI** (~3500 words) **[NEW]**
    - 12-word seed display, backup verification flow
    - Recovery import, iCloud Keychain optional
    - Priority: P1 | Effort: 3-5 days

**Phase 2 Total**: ~27,500 words | 10.5-15 weeks effort
**Effort Reduction**: ~35-40% vs manual Lightning (was 18-23 weeks)

---

### ✅ Phase 3: Backends - KEY ISSUES (2 of 7)

12. **017 - macOS MLX Integration** (~3000 words)
    - MLX/Swift-MLX, GGUF models, quantization
    - Priority: P1 | Effort: 3-4 weeks

13. **020 - macOS Model Router** (~2500 words)
    - Backend abstraction, routing logic, fallback chains
    - Priority: P1 | Effort: 2-3 weeks

**Not Created (can defer or create later)**:
- 018: macOS Ollama Integration
- 019: macOS llama.cpp Integration
- 021: macOS Resource Management
- 022: Reputation System
- 023: macOS Observability Dashboard

---

### ✅ Phase 4: SearchKit - EPIC (1 of 1)

14. **024 - SearchKit MVP** (~3500 words)
    - Full hybrid search engine (from spec-v0.2.2)
    - Explicitly deferrable (explained why)
    - Priority: P2 | Effort: 6-8 weeks

---

### ✅ Documentation - KEY ISSUE (1 of 4)

15. **025 - ADRs: Marketplace Architecture** (~2500 words)
    - 5 ADRs: Nostr, Lightning, iOS-coordination, Multi-backend, AUP
    - Priority: P1 | Effort: 1-2 weeks

**Not Created (can defer)**:
- 026: Integration Guides
- 027: Protocol Specifications
- 028: User Documentation

---

### ✅ Testing - KEY ISSUE (1 of 3)

16. **029 - Unit Test Suite** (~2500 words)
    - Comprehensive test coverage targets (80-95%)
    - Test organization, CI integration
    - Priority: P0 | Effort: 3-4 weeks

**Not Created (can defer)**:
- 030: Integration & E2E Tests
- 031: Security & Compliance Testing

---

## Statistics

| Metric | Value |
|--------|-------|
| **Total Issues Created** | 19 of 31 planned (16 original + 3 new - 1 deleted) |
| **Total Words Written** | ~75,000 words (increased from ~55,000) |
| **Estimated LOC** | ~7,000 (increased from ~5,000 due to Spark SDK integration code) |
| **Estimated Effort** | ~44-61 engineering weeks (reduced from ~45-60 via Spark SDK) |
| **Team Size (Recommended)** | 3 engineers |
| **Timeline (Parallelized)** | ~14-18 weeks for Phases 1-3 (reduced from ~15-20) |
| **Phase 2 Effort Reduction** | ~35-40% (10.5-15w vs 18-23w manual Lightning) |

## Spark SDK Integration Decision

### Why Spark SDK?

**User Directive**:
> "I want you to think deeply and make a plan for how to integrate Bitcoin and Spark into our compute plan. Note that we're not going to be manually implementing Bolt 11 or some of the stuff you suggested. We're going to basically use Spark via Breez for all of our Bitcoin and Lightning stuff."

**Technical Benefits**:
- ✅ **Production-ready SDK**: Breez maintains the SDK, node infrastructure, and LSP (no manual Lightning protocol work)
- ✅ **Nodeless operation**: Users don't need to manage Lightning channels or liquidity
- ✅ **Better UX**: Offline receive, instant sends, automatic backups
- ✅ **Effort reduction**: ~35-40% reduction in Phase 2 effort (BOLT11 manual work eliminated)
- ✅ **Code sharing**: SparkWalletManager shared between iOS and macOS (reduces duplication)

**What Changed**:
- **❌ Deleted**: Issue #003 (BOLT11 & Lightning Primitives) - **2-3 weeks saved**
- **✏️ Rewrote**: Issues #010, #013 (iOS/macOS Wallets) - simplified with Spark SDK
- **🆕 Created**:
  - Issue #012 (API Key Config) - 2-3 days
  - Issue #015 (Payment Coordinator) - 1-2 weeks
  - Issue #016 (Seed Backup/Recovery) - 3-5 days

**Architecture**:
- Spark is **NOT Lightning** - it's a Layer 2 Bitcoin protocol using statechain technology
- Threshold signatures (FROST): user holds one key, Spark Operators hold another
- Self-custodial: users always control Bitcoin, can exit to L1 anytime
- Pre-signed timelocked exit transactions ensure safety
- Complete BOLT11 support (send/receive Lightning invoices)

**For Full Details**: See `docs/compute/issues/SPARK-SDK-INTEGRATION.md`

## Coverage

### Critical Path ✅ COMPLETE
- Crypto & Keys (issue #002)
- Nostr Protocol (issue #001)
- Lightning Primitives (issue #003)
- Job Schemas (issue #004)
- iOS Identity (issue #005)
- macOS Worker (issue #007)
- Policy/AUP (issue #009)

### Marketplace MVP ✅ ~85% COMPLETE
- Provider discovery (issue #006)
- Job creation (issue #011)
- Capability advertising (issue #008)
- **Missing**: Wallet integration (010, 013) - awaiting Breez/Spark docs

### Backend Expansion ✅ ~40% COMPLETE
- Foundation Models (issue #007)
- Model routing (issue #020)
- MLX integration (issue #017)
- **Missing**: Ollama, llama.cpp, resource mgmt (can add later)

## Quality Standards Met

Each completed issue includes:

✅ **Comprehensive Acceptance Criteria**
- Detailed checklists (20-40 items per issue)
- All major requirements captured

✅ **Full Technical Design**
- Swift types, protocols, data models
- API signatures and examples
- UI mockups (SwiftUI code)
- ~1000-2000 LOC of example code per issue

✅ **Apple Compliance Analysis**
- ASRG citations (2.4.2, 2.5.2, 3.1.1, etc.)
- DPLA requirements (Foundation Models AUP)
- Privacy best practices
- Mitigation strategies

✅ **Dependencies Mapped**
- OpenAgents dependencies (which issues block which)
- External dependencies (Swift packages, frameworks)
- Critical path identified

✅ **Testing Requirements**
- Unit test requirements
- Integration test scenarios
- E2E test flows
- Performance targets

✅ **Reference Links**
- NIPs (Nostr specs)
- BIPs (Bitcoin specs)
- Apple documentation
- External libraries
- Related OpenAgents issues/ADRs

✅ **Success Metrics**
- Measurable goals
- Acceptance criteria for "done"

✅ **Future Enhancements**
- Post-MVP improvements identified
- Technical debt acknowledged

## What's Not Created (15 issues)

These can be created when needed or deferred:

**Phase 2** (5 issues):
- 010, 012, 013: **Wallet/Lightning** (awaiting Breez/Spark docs)
- 015, 016: iOS job management & provider dashboard

**Phase 3** (5 issues):
- 018, 019: Ollama & llama.cpp integration
- 021, 022, 023: Resource mgmt, reputation, observability

**Documentation** (3 issues):
- 026, 027, 028: Integration guides, protocol specs, user docs

**Testing** (2 issues):
- 030, 031: Integration/E2E tests, security testing

## Next Steps

### 1. User Review (Current)
- Review the 16 completed issues
- Provide feedback on scope, detail, priorities
- Identify any gaps or concerns

### 2. Create Remaining Issues (If Needed)
Options:
- **A. Minimal**: Only create wallet issues when Breez/Spark docs provided
- **B. Moderate**: Create remaining Phase 2-3 issues as needed during dev
- **C. Complete**: Create all 31 issues now for full planning

### 3. Publish to GitHub
```bash
# For each approved issue:
cd docs/compute/issues/phase-1-mvp
gh issue create \
  --title "$(head -n1 001-nostr-client-library.md | sed 's/# //')" \
  --body-file 001-nostr-client-library.md \
  --label "phase-1,p0,foundation" \
  --milestone "Phase 1: MVP"
```

### 4. Begin Implementation
- Assign issues to engineers
- Create feature branches
- Weekly progress check-ins

## Files Created

```
docs/compute/issues/
├── README.md                              # Master index
├── STATUS.md                              # Progress tracker
├── COMPLETED.md                           # This file
├── phase-1-mvp/ (9 issues - COMPLETE)
│   ├── 001-nostr-client-library.md       ✅
│   ├── 002-secp256k1-crypto.md           ✅
│   ├── 003-bolt11-lightning-primitives.md ✅
│   ├── 004-job-schema-registry.md        ✅
│   ├── 005-ios-nostr-identity.md         ✅
│   ├── 006-ios-marketplace-viewer.md     ✅
│   ├── 007-macos-foundation-models-worker.md ✅
│   ├── 008-macos-capability-advertising.md ✅
│   └── 009-policy-safety-module.md       ✅
├── phase-2-payments/ (2 of 7)
│   ├── 011-ios-job-creation-submission.md ✅
│   └── 014-macos-bidding-engine.md       ✅
├── phase-3-backends/ (2 of 7)
│   ├── 017-macos-mlx-integration.md      ✅
│   └── 020-macos-model-router.md         ✅
├── phase-4-searchkit/ (1 of 1)
│   └── 024-searchkit-mvp.md              ✅
├── documentation/ (1 of 4)
│   └── 025-adrs-marketplace.md           ✅
└── testing/ (1 of 3)
    └── 029-unit-test-suite.md            ✅
```

## Key Insights

### Architecture (From apple-terms-research.md)

**What We're Building**:
- **iOS**: Coordination + wallet + agent management (NO compute)
- **macOS**: Worker runtime (FM + MLX + Ollama)
- **Nostr**: Decentralized marketplace (NIP-90 + NIP-57)
- **Lightning**: Micropayments (BOLT11 invoices, zaps)
- **Open**: Any client can join liquidity pool

**Why This Architecture**:
- ✅ **Apple compliant**: iOS doesn't run background workers
- ✅ **Privacy**: On-device compute (Foundation Models, MLX)
- ✅ **Decentralized**: No OpenAgents server required
- ✅ **Interoperable**: Open protocols (Nostr, Lightning, NIP-90)

### Critical Dependencies

```
Crypto (002) → Nostr (001) → Everything Else
                ↓
            Wallet (010) → Payments (012, 013)
                ↓
            Worker (007) → Backends (017, 020)
                ↓
            Policy (009) → AUP Enforcement
```

### Apple Compliance (All Issues Addressed)

**App Store Review Guidelines**:
- ✅ ASRG 2.4.2: No background mining (iOS = coordination only)
- ✅ ASRG 2.5.2: No downloaded code (Nostr events = data)
- ✅ ASRG 2.5.4: Background limits (no ambient workers)
- ✅ ASRG 3.1.1/3.1.3: Payment flows (IAP vs non-IAP)
- ✅ ASRG 3.1.5(i): Crypto wallets (Organization developer)

**DPLA & Foundation Models AUP**:
- ✅ Policy Module (issue #009) enforces AUP
- ❌ Prohibited: Healthcare/legal/finance, academic textbooks
- ✅ Allowed: Summarization, code gen, Q&A (non-regulated)

## Recommendation

**For MVP (All critical issues complete)**:
1. ✅ Review Phase 1 issues (001-009, minus deleted #003) - **Foundation complete**
2. ✅ Review Phase 2 Spark SDK integration (010, 012, 013, 015, 016) - **Payment system complete**
3. ✅ Review Spark SDK integration plan (`SPARK-SDK-INTEGRATION.md`)
4. 🚀 **Ready to implement**: All critical path issues specified and ready

**Timeline**: ~14-18 weeks for Phases 1-3 with 3 engineers → Full marketplace with multi-backend support

**Next Steps**:
1. User approval of all 19 issues + Spark SDK integration
2. Publish approved issues to GitHub
3. Create milestones (Phase 1, Phase 2, Phase 3)
4. Assign engineers and begin implementation

---

**Questions or feedback?** Review the individual issues and let me know what needs adjustment!
