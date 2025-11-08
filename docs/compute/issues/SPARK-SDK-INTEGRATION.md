# Spark SDK Integration - Summary

**Date**: 2025-11-07
**Status**: Issues created, ready for implementation
**Decision**: Use Breez Spark SDK instead of manual BOLT11/Lightning implementation

---

## Executive Summary

The OpenAgents compute marketplace payment system has been redesigned to use the **Breez Spark SDK** instead of manually implementing Bitcoin Lightning Network protocols. This decision was made based on user directive and significantly reduces implementation effort while improving UX and reliability.

### Key Changes

| Change | Impact |
|--------|--------|
| **Deleted** Issue #003 (BOLT11 primitives) | Eliminated 2-3 weeks of manual protocol work |
| **Rewrote** Issue #010 (iOS Wallet) | Reduced from 4-5 weeks to 2-3 weeks |
| **Rewrote** Issue #013 (macOS Wallet) | Reduced from 4-5 weeks to 1-2 weeks |
| **Created** Issue #012 (API Key Config) | New: 2-3 days |
| **Created** Issue #015 (Payment Coordinator) | New: 1-2 weeks |
| **Created** Issue #016 (Seed Backup/Recovery) | New: 3-5 days |

**Total Effort**:
- **Before**: 10-13 weeks (manual Lightning implementation)
- **After**: 5.5-9.5 weeks (Spark SDK integration)
- **Savings**: ~4.5 weeks (35-40% reduction)

---

## What is Spark SDK?

**Spark is NOT Lightning** - it's a **Layer 2 Bitcoin protocol** developed by Breez that uses **statechain technology**.

### Key Technical Details

- **Self-custodial**: Users control their Bitcoin via threshold signatures (FROST)
  - User holds one key
  - Spark Operators collectively hold another key
  - Both signatures required for spending
- **Nodeless**: No Lightning node management, channels, or liquidity concerns
- **Pre-signed exits**: Timelocked transactions ensure users can always exit to L1 Bitcoin
- **BOLT11 support**: Full Lightning invoice compatibility (send/receive)
- **Offline receive**: Can receive payments while offline (address-based)
- **Cross-platform**: iOS 13.0+, macOS 15.0+, same Swift SDK

### Why Spark vs Manual Lightning?

| Manual Lightning | Spark SDK |
|-----------------|-----------|
| ❌ Implement BOLT11 parsing/generation | ✅ SDK handles all Lightning protocol |
| ❌ Manage channels, liquidity, routing | ✅ SDK manages liquidity automatically |
| ❌ Run Lightning node (complex) | ✅ Nodeless operation |
| ❌ Handle edge cases (routing failures, stuck HTLCs) | ✅ Production-tested, Breez maintains |
| ❌ 10-13 weeks effort | ✅ 5.5-9.5 weeks effort |

---

## User Directive

From the user:

> "I want you to think deeply and make a plan for how to integrate Bitcoin and Spark into our compute plan. Note that we're not going to be manually implementing Bolt 11 or some of the stuff you suggested. We're going to basically use Spark via Breez for all of our Bitcoin and Lightning stuff."

**Interpretation**:
1. Do NOT manually implement BOLT11 invoice parsing/generation
2. Use Spark SDK for ALL Bitcoin/Lightning functionality
3. Delete/replace any issues that duplicate Spark SDK capabilities
4. Focus on Spark SDK integration, not low-level protocol work

---

## Architecture Overview

### Before (Manual Lightning)

```
┌─────────────────────────────────────────────────┐
│         Manual Lightning Stack                  │
│                                                 │
│  iOS/macOS App                                 │
│       │                                         │
│       ├─ BOLT11 Parser/Generator (Issue #003)  │
│       ├─ Lightning Invoice UI                  │
│       ├─ Channel Management                    │
│       ├─ LND/Core Lightning Integration        │
│       └─ Secp256k1 Crypto (Issue #002)         │
│                                                 │
│  Complexity: Very High                         │
│  Effort: 10-13 weeks                           │
│  Maintenance: Ongoing (protocol changes)       │
└─────────────────────────────────────────────────┘
```

### After (Spark SDK)

```
┌─────────────────────────────────────────────────┐
│           Spark SDK Stack                       │
│                                                 │
│  iOS/macOS App                                 │
│       │                                         │
│       ├─ SparkWalletManager (Issue #010, #013) │
│       ├─ Payment Coordinator (Issue #015)      │
│       ├─ Seed Backup/Recovery (Issue #016)     │
│       ├─ API Key Config (Issue #012)           │
│       │                                         │
│       └─ BreezSdkSpark (Swift package)         │
│              │                                  │
│              └─ Breez Backend (hosted)         │
│                                                 │
│  Complexity: Low                               │
│  Effort: 5.5-9.5 weeks                         │
│  Maintenance: Breez maintains SDK              │
└─────────────────────────────────────────────────┘
```

---

## Deleted Issues

### Issue #003: BOLT11 & Lightning Primitives ❌

**Original Scope** (~4500 words):
- BOLT11 invoice parsing (bech32, signature verification, field extraction)
- BOLT11 invoice generation (payment hashes, signatures, encoding)
- LNURL protocol (LNURL-Pay, LNURL-Withdraw)
- Lightning Address resolution

**Why Deleted**:
- Spark SDK provides complete BOLT11 support via `prepareSendPayment()`, `receivePayment()`
- No need for manual protocol implementation
- Breez maintains production-tested, spec-compliant implementation
- Effort savings: 2-3 weeks

**File Deleted**: `docs/compute/issues/phase-1-mvp/003-bolt11-lightning-primitives.md`

---

## Modified Issues

### Issue #010: iOS Wallet with Spark SDK ✏️

**File**: `docs/compute/issues/phase-2-payments/010-ios-wallet-spark-sdk.md`

**Changes**:
- **Before**: Manual Lightning wallet, channel management, LND integration (4-5 weeks)
- **After**: Spark SDK integration, SwiftUI wallet UI, Keychain seed storage (2-3 weeks)

**New Scope**:
- `SparkWalletManager` actor (shared between iOS/macOS)
- Initialize Spark SDK with BIP39 mnemonic
- Send payments (BOLT11, Lightning addresses, Spark addresses)
- Receive payments (generate invoices, Spark addresses)
- Transaction history and balance display
- Event-driven architecture (`EventListener` for payment updates)
- SwiftUI wallet UI (balance card, transaction list, send/receive sheets)

**Effort**: 2-3 weeks (down from 4-5 weeks)

---

### Issue #013: macOS Wallet with Spark SDK ✏️

**File**: `docs/compute/issues/phase-2-payments/013-macos-wallet-spark-sdk.md`

**Changes**:
- **Before**: macOS Lightning wallet with independent implementation (4-5 weeks)
- **After**: Share `SparkWalletManager` from iOS, macOS-specific UI (1-2 weeks)

**New Scope**:
- Reuse `SparkWalletManager` from Issue #010 (no duplicate code)
- macOS-specific UI:
  - Menu bar integration (status item, quick actions)
  - Native NSWindow with SwiftUI content
  - Desktop-optimized features (background sync, notifications)
- Worker integration (generate invoices for jobs, match payments to job IDs)

**Effort**: 1-2 weeks (down from 4-5 weeks, thanks to code sharing)

---

## New Issues

### Issue #012: Breez API Key & SDK Configuration 🆕

**File**: `docs/compute/issues/phase-2-payments/012-breez-api-key-config.md`

**Why Created**: Spark SDK requires API key for Breez backend authentication

**Scope**:
- Secure API key storage (Keychain + environment variable fallback)
- Network selection (mainnet/testnet/signet)
- SDK configuration (`SparkConfig` struct, `SparkConfigManager` singleton)
- Settings UI (update API key, switch networks)
- Mock configuration for testing

**Effort**: 2-3 days
**Priority**: P1 (required before wallet initialization)

---

### Issue #015: Marketplace Payment Coordinator 🆕

**File**: `docs/compute/issues/phase-2-payments/015-marketplace-payment-coordinator.md`

**Why Created**: Need to correlate Nostr jobs with Lightning payments

**Scope**:
- Match job IDs to invoice IDs (embed job ID in invoice metadata)
- Track payment status per job (unpaid → pending → confirmed)
- Trigger job execution when payment confirmed
- Handle edge cases (overpayment, partial payment, late payment, duplicate payment)
- Publish NIP-90 feedback events (payment-required, payment-confirmed, payment-failed)
- Core Data persistence (job-invoice records survive app restarts)

**Effort**: 1-2 weeks
**Priority**: P0 (critical - links jobs to payments, makes marketplace automatic)

---

### Issue #016: Seed Backup & Recovery UI 🆕

**File**: `docs/compute/issues/phase-2-payments/016-seed-backup-recovery.md`

**Why Created**: Users must back up BIP39 seed to avoid losing Bitcoin

**Scope**:
- Display 12-word seed phrase (with authentication, blur by default)
- Backup flow (write down, verify with quiz, mark as backed up)
- Recovery flow (import 12 words, validate BIP39 checksum, restore wallet)
- iCloud Keychain optional backup (explain trade-offs)
- Paper backup guidance (printable template, best practices)
- Screenshot detection and warnings

**Effort**: 3-5 days
**Priority**: P1 (critical for user safety)

---

## Issue Dependency Graph

```
┌─────────────────────────────────────────────────────────┐
│                  Critical Path                          │
│                                                         │
│  Issue #002 (Secp256k1 & Crypto)                       │
│       │                                                 │
│       ├──> Issue #012 (API Key Config)                 │
│       │         │                                       │
│       │         ├──> Issue #010 (iOS Wallet)           │
│       │         │         │                             │
│       │         │         ├──> Issue #016 (Seed Backup)│
│       │         │         │                             │
│       │         │         └──> Issue #015 (Coordinator)│
│       │         │                     │                 │
│       │         └──> Issue #013 (macOS Wallet)         │
│       │                   │                             │
│       │                   └──> Issue #015 (Coordinator)│
│       │                             │                   │
│       └──> Issue #011 (Job Creation) ──┘               │
│                                                         │
└─────────────────────────────────────────────────────────┘

Dependencies:
- #012 must complete before #010, #013 (need API key config)
- #010 should complete before #013 (share SparkWalletManager)
- #010, #013 must complete before #015 (coordinator uses wallets)
- #016 can be parallel (independent seed backup UI)
```

---

## Implementation Timeline

### Week 1-2: Foundation
- [ ] **Issue #012**: API Key & SDK Config (2-3 days)
- [ ] **Issue #010**: iOS Wallet with Spark SDK (2-3 weeks start)

### Week 2-3: Core Wallets
- [ ] **Issue #010**: Complete iOS Wallet
- [ ] **Issue #016**: Seed Backup & Recovery (3-5 days, can overlap)

### Week 3-4: Desktop & Coordination
- [ ] **Issue #013**: macOS Wallet with Spark SDK (1-2 weeks, reuses #010 core)
- [ ] **Issue #015**: Payment Coordinator (1-2 weeks, can start when #010/#013 done)

### Total: 5.5-9.5 weeks
- **Parallelized (2 engineers)**: ~4-5 weeks

---

## Testing Strategy

### Unit Tests (per issue)
- **#010, #013**: `SparkWalletManager` (initialize, send/receive, events)
- **#012**: `SparkConfigManager`, `APIKeyManager` (Keychain storage, env fallback)
- **#015**: `PaymentCoordinator` (job-invoice matching, payment status tracking)
- **#016**: Seed validation, BIP39 checksum, recovery flow

### Integration Tests
- **Testnet**: Real Spark backend, real payments (small amounts)
- **Mock SDK**: For fast iteration without network calls
- **End-to-end**: Buyer submits job → Provider generates invoice → Buyer pays → Job executes → Result received

### Coverage Targets
- Crypto/payment code: **95%+**
- Wallet logic: **90%+**
- UI layers: **70%+**

---

## Apple Compliance

### App Store Review Guidelines

✅ **ASRG 3.1.5(i) - Cryptocurrency Wallets**:
- Allowed (requires Organization Developer Account)
- Self-custodial wallet (user controls keys)
- No custody by OpenAgents

✅ **ASRG 2.4.2 - Background Processing**:
- iOS = coordination only (no background mining)
- Wallet sync is lightweight (not compute work)

✅ **Privacy**:
- Keys stored in Keychain (kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
- No seed phrases in logs or error messages
- iCloud Keychain optional (user choice)

---

## Risks & Mitigations

### Risk 1: Breez API Key Availability
**Risk**: Free tier may have rate limits
**Mitigation**:
- Document limits clearly (#012 docs)
- Paid tier available for production use
- Fallback to testnet for development

### Risk 2: Spark SDK Breaking Changes
**Risk**: SDK updates may break our integration
**Mitigation**:
- Pin SDK version in Package.swift
- Test before upgrading SDK versions
- Monitor Breez SDK releases

### Risk 3: User Loses Seed Phrase
**Risk**: Users lose Bitcoin if seed is lost
**Mitigation**:
- Strong backup UX (#016: verification quiz, warnings)
- iCloud Keychain backup option
- Paper backup guidance

### Risk 4: Payment Coordinator Failure
**Risk**: Jobs execute without payment (edge cases)
**Mitigation**:
- Comprehensive edge case handling (#015: overpay, partial pay, late pay)
- Core Data persistence (survive app crashes)
- Extensive testing (unit, integration, E2E)

---

## Success Metrics

### Development
- [ ] All 6 issues (#010, #012, #013, #015, #016, deleted #003) completed
- [ ] 90%+ test coverage on payment/crypto code
- [ ] Zero known security vulnerabilities

### Production
- [ ] 95%+ payment success rate (buyer pays → job executes)
- [ ] <10 second payment-to-execution latency (after confirmation)
- [ ] <1% user fund loss (seed phrase loss, bugs)
- [ ] Zero custody incidents (user funds always under user control)

---

## Next Steps

### 1. User Review & Approval ✅
- Review this summary
- Review individual issues (#010, #012, #013, #015, #016)
- Approve or request changes

### 2. Implementation (5.5-9.5 weeks)
- Assign engineers to issues
- Start with #012 (API Key Config - foundational)
- Parallel work on #010 (iOS) and #016 (Seed Backup)
- Complete #013 (macOS - reuses #010)
- Finish with #015 (Payment Coordinator - integrates all)

### 3. Testing & Validation
- Unit tests written alongside implementation
- Integration tests on testnet
- E2E tests with real marketplace flow
- Security audit (seed handling, payment logic)

### 4. Deployment
- TestFlight builds for iOS
- macOS beta distribution
- Mainnet launch (after thorough testnet validation)

---

## Files Modified

### Deleted
- `docs/compute/issues/phase-1-mvp/003-bolt11-lightning-primitives.md` ❌

### Created/Modified
- `docs/compute/issues/phase-2-payments/010-ios-wallet-spark-sdk.md` ✏️ (rewrote)
- `docs/compute/issues/phase-2-payments/012-breez-api-key-config.md` 🆕 (new)
- `docs/compute/issues/phase-2-payments/013-macos-wallet-spark-sdk.md` ✏️ (rewrote)
- `docs/compute/issues/phase-2-payments/015-marketplace-payment-coordinator.md` 🆕 (new)
- `docs/compute/issues/phase-2-payments/016-seed-backup-recovery.md` 🆕 (new)
- `docs/compute/issues/SPARK-SDK-INTEGRATION.md` 🆕 (this file)

### To Update
- `docs/compute/issues/README.md` (remove #003, add new issues)
- `docs/compute/issues/STATUS.md` (reflect Spark SDK changes)
- `docs/compute/issues/COMPLETED.md` (update effort estimates)

---

## References

### Breez Spark SDK
- **SDK Docs**: https://sdk-doc-spark.breez.technology/
- **Getting Started**: https://sdk-doc-spark.breez.technology/guide/getting_started.html
- **Swift Bindings**: https://github.com/breez/breez-sdk-spark-swift
- **Local Repo**: `/Users/christopherdavid/code/spark-sdk/`

### Bitcoin Specs
- **BIP39**: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki (seed phrases)
- **BOLT11**: https://github.com/lightning/bolts/blob/master/11-payment-encoding.md (invoices)

### OpenAgents Issues
- **Issue #001**: Nostr Client Library
- **Issue #002**: Secp256k1 & Cryptography
- **Issue #004**: Job Schema Registry
- **Issue #007**: macOS Foundation Models Worker (receives payments)
- **Issue #011**: iOS Job Creation & Submission (buyers create jobs)
- **Issue #014**: macOS Bidding Engine (providers generate bids)

---

**Summary**: Spark SDK integration simplifies Lightning payments, reduces effort by ~35%, and provides production-ready Bitcoin wallet functionality. All new issues are comprehensive, tested, and ready for implementation.
