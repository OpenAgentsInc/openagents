# OpenAgents Compute Marketplace Issues - Status

**Last Updated**: 2025-11-07
**Total Issues**: 31 planned
**Completed (Draft)**: 6 of 31

## Summary

This directory contains draft GitHub issues for implementing the OpenAgents compute marketplace. The issues are based on the comprehensive architecture analysis in `/Users/christopherdavid/code/openagents/docs/compute/apple-terms-research.md`.

## Progress

### ✅ Completed Draft Issues

#### Foundation Layer (Critical Path)
- **✅ 001**: Nostr Client Library (Swift) - **COMPLETE**
  - Full NIP-01, NIP-04, NIP-19, NIP-57, NIP-90 implementation
  - Multi-relay manager, event signing, encryption
  - ~6000 words, comprehensive technical spec

- **✅ 002**: Secp256k1 & Cryptography Integration - **COMPLETE**
  - Schnorr/ECDSA signatures, ECDH, Secure Enclave
  - BECH32 encoding, HD wallet (BIP32/39/84)
  - ~5500 words, full API design

- **✅ 003**: BOLT11 & Lightning Primitives - **COMPLETE**
  - Invoice parsing/generation, LNURL protocol
  - Lightning Address resolution
  - ~4500 words, complete spec

- **✅ 004**: Job Schema Registry (NIP-90 Extension) - **COMPLETE**
  - 12 job kinds defined with schemas
  - JobBuilder, validation, capability advertising
  - ~4000 words, registry format + Swift types

#### iOS Application
- **✅ 005**: iOS Nostr Identity & Key Management - **COMPLETE**
  - Key generation/import, Secure Enclave storage
  - Relay management UI, identity display
  - ~4000 words, full SwiftUI implementation

#### Policy & Safety
- **✅ 009**: Policy & Safety Module (Foundation Models AUP) - **COMPLETE**
  - AUP enforcement, content classification
  - Foundation Models-based classifier + keyword fallback
  - ~4500 words, compliance-first design

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

| Issue | Words | LOC Estimated | Effort |
|-------|-------|---------------|--------|
| 001 | ~6000 | ~600 | 4-6w |
| 002 | ~5500 | ~450 | 2-3w |
| 003 | ~4500 | ~400 | 2-3w |
| 004 | ~4000 | ~350 | 1-2w |
| 005 | ~4000 | ~300 | 2-3w |
| 009 | ~4500 | ~400 | 2-3w |
| **Total** | **~28,500** | **~2,500** | **14-21w** |

## Next Steps

### Immediate (Complete Phase 1)
1. Create issues **006-008** (iOS Viewer, macOS Worker, Capabilities)
2. Review all Phase 1 issues for completeness
3. Get user approval on Phase 1

### Short-term (Phase 2 Foundation)
4. Create **critical** Phase 2 issues (010, 011, 013 - wallet + payments)
5. Create placeholders for remaining Phase 2-4 issues
6. Prioritize based on user feedback

### Publishing
7. Convert approved issues to GitHub via `gh issue create`
8. Create milestones: Phase 1, Phase 2, Phase 3, Phase 4
9. Add labels: priority (p0/p1/p2), component (ios/macos/shared), phase

## Key Insights from Research

### Architecture (from apple-terms-research.md)
- **iOS**: Coordination + wallet + agent management (NO worker compute)
- **macOS**: Worker runtime (Foundation Models + MLX + Ollama)
- **Nostr**: Open marketplace (NIP-90 + NIP-57)
- **Compliance**: Strict Apple rules enforced (no background workers, AUP filters)

### Critical Dependencies
```
Crypto (002) → Nostr (001) → Everything Else
BOLT11 (003) → Payments (010, 012, 013)
Schemas (004) → Worker (007), Marketplace (006)
Policy (009) → Worker (007) - AUP enforcement
```

### Timeline Estimates
- **Phase 1** (MVP): 6-8 weeks with 3 engineers
- **Phase 2** (Payments): 5-6 weeks
- **Phase 3** (Backends): 4-5 weeks
- **Total**: ~15-19 weeks for full marketplace (Phases 1-3)

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
├── README.md                    # Master index with all 31 issues listed
├── STATUS.md                    # This file (progress tracking)
├── phase-1-mvp/
│   ├── 001-nostr-client-library.md          ✅ COMPLETE
│   ├── 002-secp256k1-crypto.md              ✅ COMPLETE
│   ├── 003-bolt11-lightning-primitives.md   ✅ COMPLETE
│   ├── 004-job-schema-registry.md           ✅ COMPLETE
│   ├── 005-ios-nostr-identity.md            ✅ COMPLETE
│   ├── 006-ios-marketplace-viewer.md        ⏳ TODO
│   ├── 007-macos-foundation-models-worker.md ⏳ TODO
│   ├── 008-macos-capability-advertising.md  ⏳ TODO
│   └── 009-policy-safety-module.md          ✅ COMPLETE
├── phase-2-payments/             ⏳ 7 issues TODO
├── phase-3-backends/             ⏳ 7 issues TODO
├── phase-4-searchkit/            ⏳ 1 issue TODO
├── documentation/                ⏳ 4 issues TODO
└── testing/                      ⏳ 3 issues TODO
```

---

**Ready for review**: Issues 001, 002, 003, 004, 005, 009
**Next**: Complete Phase 1 (issues 006, 007, 008), then proceed to Phase 2
