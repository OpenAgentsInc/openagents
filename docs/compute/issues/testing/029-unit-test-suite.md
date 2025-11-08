# Unit Test Suite (Comprehensive)

**Phase:** Cross-Cutting
**Component:** All (OpenAgentsCore, iOS, macOS)
**Priority:** P0 (Critical - Quality gate)
**Estimated Effort:** 3-4 weeks (ongoing)

## Summary

Implement comprehensive unit test coverage for all marketplace components, targeting 80%+ code coverage for critical paths and 90%+ for crypto/policy modules.

## Motivation

Tests ensure:
- **Correctness**: Code works as specified
- **Regression prevention**: Changes don't break existing functionality
- **Documentation**: Tests show how APIs should be used
- **Confidence**: Safe to refactor

Critical for security-sensitive code (crypto, payments, policy).

## Test Coverage Targets

### OpenAgentsCore (Shared)

**Crypto & Keys** (Target: 95%+)
- [ ] Secp256k1 operations (key gen, signing, verification)
- [ ] Schnorr signatures (test vectors from BIP340)
- [ ] ECDSA signatures (Bitcoin test vectors)
- [ ] ECDH shared secret derivation
- [ ] BECH32 encoding/decoding (NIP-19 test vectors)
- [ ] Secure Enclave key storage (mock on Simulator)
- [ ] HD wallet (BIP32/39/84 test vectors)

**Nostr** (Target: 90%+)
- [ ] Event ID calculation (NIP-01 test vectors)
- [ ] Event signing and verification
- [ ] Event serialization/deserialization
- [ ] Filter matching logic
- [ ] NIP-04 encryption/decryption (test vectors)
- [ ] BECH32 npub/nsec encoding
- [ ] DVM event creation/parsing (NIP-90)
- [ ] Zap event parsing (NIP-57)
- [ ] Relay message parsing

**Lightning** (Target: 90%+)
- [ ] BOLT11 invoice parsing (test vectors)
- [ ] BOLT11 invoice generation
- [ ] Signature verification (secp256k1)
- [ ] Amount conversions (msat ↔ sat ↔ BTC)
- [ ] LNURL decoding/encoding
- [ ] Lightning Address parsing
- [ ] Expiry checking

**Job Schemas** (Target: 85%+)
- [ ] Job schema registry loading
- [ ] Parameter validation (types, ranges, enums)
- [ ] Required param enforcement
- [ ] JobBuilder creates valid events
- [ ] All example requests in registry are valid

**Policy & Safety** (Target: 95%+)
- [ ] AUP classifier detects prohibited content
- [ ] Keyword classifier (healthcare, legal, finance)
- [ ] Foundation Models classifier (integration test)
- [ ] Policy config enforcement levels
- [ ] Audit logging

**Worker** (Target: 80%+)
- [ ] Job queue FIFO ordering
- [ ] Concurrency limits
- [ ] Job executor with Foundation Models
- [ ] Prompt building from job inputs
- [ ] System instructions per job kind
- [ ] Stats calculation

**Model Router** (Target: 80%+)
- [ ] Backend routing logic
- [ ] Fallback chains
- [ ] Availability checks
- [ ] Model mapping per job kind

### iOS App

**Identity Management** (Target: 80%+)
- [ ] Identity creation and storage
- [ ] Import from nsec
- [ ] Duplicate prevention
- [ ] Active identity switching
- [ ] QR code generation

**Relay Management** (Target: 75%+)
- [ ] Relay URL validation
- [ ] Add/remove relays
- [ ] Connection state management

**Marketplace Viewer** (Target: 70%+)
- [ ] Parse NIP-89 capability events
- [ ] Provider stats calculation
- [ ] Activity feed sorting

**Job Creation** (Target: 80%+)
- [ ] Job request validation
- [ ] Param validation against schema
- [ ] Event signing
- [ ] Encrypted params (NIP-04)

### macOS App

**Worker Service** (Target: 85%+)
- [ ] Job subscription and filtering
- [ ] Policy enforcement integration
- [ ] Feedback event publishing
- [ ] Result event publishing
- [ ] Encrypted result handling

**Capability Advertising** (Target: 80%+)
- [ ] NIP-89 event creation
- [ ] Capability encoding (JSON)
- [ ] Republishing logic
- [ ] Deletion events (NIP-09)

**Bidding Engine** (Target: 80%+)
- [ ] Minimum bid calculation
- [ ] Surge pricing logic
- [ ] Priority scoring
- [ ] Resource-aware pricing

## Test Organization

```
ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/
├── Crypto/
│   ├── Secp256k1Tests.swift
│   ├── SchnorrTests.swift
│   ├── ECDSATests.swift
│   ├── ECDHTests.swift
│   └── Bech32Tests.swift
├── Nostr/
│   ├── NostrEventTests.swift
│   ├── NostrFilterTests.swift
│   ├── NostrEncryptionTests.swift
│   ├── NostrDVMTests.swift
│   └── NostrZapsTests.swift
├── Lightning/
│   ├── BOLT11Tests.swift
│   ├── LNURLTests.swift
│   └── LightningAddressTests.swift
├── JobSchemas/
│   ├── JobSchemaRegistryTests.swift
│   ├── JobBuilderTests.swift
│   └── ValidationTests.swift
├── Policy/
│   ├── PolicyEnforcerTests.swift
│   ├── ClassifierTests.swift
│   └── AUPComplianceTests.swift
├── Worker/
│   ├── JobQueueTests.swift
│   ├── JobExecutorTests.swift
│   └── WorkerStatsTests.swift
└── ModelRouter/
    ├── RoutingTests.swift
    └── FallbackTests.swift

ios/OpenAgentsTests/ (iOS app tests)
└── ... (UI layer tests)

ios/OpenAgentsMacTests/ (macOS app tests)
└── ... (Worker, capability advertising tests)
```

## Test Utilities

```swift
// TestHelpers.swift

struct TestVectors {
    static let nostrPrivateKey = "..."  // Known test key
    static let nostrPublicKey = "..."
    static let bolt11Invoice = "lnbc..."  // Valid test invoice
    // ... more test data
}

class MockNostrRelay: NostrRelayDelegate {
    var receivedEvents: [NostrEvent] = []
    // ... mock implementation
}

class MockFoundationModels: ModelBackend {
    var responses: [String] = []
    // ... mock implementation
}
```

## Testing Tools

- **XCTest**: Apple's testing framework
- **XCTestExpectation**: Async testing
- **Test Doubles**: Mocks, stubs for external dependencies
- **Code Coverage**: Xcode coverage reports

## CI Integration

```yaml
# .github/workflows/test.yml

name: Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run Tests
        run: |
          cd ios
          xcodebuild test \
            -workspace OpenAgents.xcworkspace \
            -scheme OpenAgents \
            -sdk iphonesimulator \
            -destination 'platform=iOS Simulator,name=iPhone 15' \
            -enableCodeCoverage YES
      - name: Upload Coverage
        uses: codecov/codecov-action@v3
```

## Success Metrics

- [ ] 80%+ overall code coverage
- [ ] 90%+ crypto/policy coverage
- [ ] All critical paths tested
- [ ] CI runs tests on every PR
- [ ] No flaky tests
- [ ] Test execution <5 minutes

## Dependencies

All completed issues (tests written alongside implementation)

## Notes

- **Test-Driven Development**: Write tests before/during implementation
- **Test Vectors**: Use official test vectors (NIP-01, BIP340, BOLT11)
- **Mocks**: Mock external dependencies (Foundation Models, Nostr relays)
- **Security**: Test crypto operations thoroughly (never skip)

## Future Enhancements

- Property-based testing (QuickCheck-style)
- Mutation testing (verify tests catch bugs)
- Performance regression tests
- Snapshot testing (UI)
