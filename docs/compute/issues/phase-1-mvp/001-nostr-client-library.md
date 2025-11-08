# Nostr Client Library (Swift)

**Phase:** 1 - MVP
**Component:** OpenAgentsCore (Shared)
**Priority:** P0 (Critical Path - Blocks all marketplace features)
**Estimated Effort:** 2-3 weeks

## Summary

Integrate and extend the **OpenAgents fork of nostr-sdk-ios** ([https://github.com/OpenAgentsInc/nostr-sdk-ios](https://github.com/OpenAgentsInc/nostr-sdk-ios)) for both iOS and macOS, adding marketplace-specific NIPs required for the compute marketplace. The SDK already implements 25+ NIPs including core protocol, encryption, and relay management. This issue focuses on adding the missing marketplace NIPs: **NIP-57 (Lightning Zaps)**, **NIP-89 (Application Handlers)**, and **NIP-90 (Data Vending Machines)**.

## Motivation

The compute marketplace architecture requires Nostr as the decentralized protocol layer for:

- **Job discovery and submission** (NIP-90 Data Vending Machine)
- **Lightning payments** (NIP-57 Zaps)
- **Identity management** (NIP-01 events + NIP-19 BECH32 identifiers)
- **Privacy** (NIP-04 encrypted direct messages, NIP-44 modern encryption)
- **Provider advertising** (NIP-89 capabilities, NIP-11 relay metadata)

Without a robust Nostr client, neither iOS marketplace coordination nor macOS worker functionality can be implemented. This is the critical path dependency for the entire marketplace vision.

## Why Fork nostr-sdk-ios?

We are using a **fork** of the official nostr-sdk-ios ([original repo](https://github.com/nostr-sdk/nostr-sdk-ios)) for two reasons:

1. **Stale Upstream**: The official SDK hasn't had a release since **February 2025** (9 months ago as of November 2025)
2. **Rapid Iteration**: We need to quickly add marketplace-specific NIPs (NIP-57, NIP-89, NIP-90) without waiting for upstream merges

Our fork is at: [https://github.com/OpenAgentsInc/nostr-sdk-ios](https://github.com/OpenAgentsInc/nostr-sdk-ios)

## What the SDK Already Provides

The nostr-sdk-ios fork already implements **25+ NIPs**, including all the core infrastructure we need:

### ✅ Core Protocol (Implemented)
- **NIP-01**: Basic protocol (events, signing, relay communication)
- **NIP-02**: Follow lists
- **NIP-05**: DNS-based verification
- **NIP-09**: Event deletion
- **NIP-10**: Text notes and threads
- **NIP-11**: Relay information
- **NIP-19**: bech32-encoded entities (npub, nsec, note)
- **NIP-44**: Encrypted payloads (modern, replaces NIP-04)
- **NIP-59**: Gift wrap (private messaging)
- **NIP-65**: Relay list metadata

### ✅ Infrastructure (Implemented)
- **RelayPool**: Multi-relay management with Combine publishers
- **NostrEvent**: Base event class with signing, verification, serialization
- **EventKind**: Enum with `.unknown(rawValue)` fallback for custom kinds
- **Filter**: Event subscriptions (authors, kinds, tags, since, until, limit)
- **WebSocket**: URLSessionWebSocketTask integration
- **Cryptography**: secp256k1 (Schnorr signatures), ECDH, AES-256-CBC

### ❌ Marketplace NIPs (Missing - This Issue)
- **NIP-57**: Lightning Zaps (kinds 9734, 9735)
- **NIP-89**: Recommended Application Handlers (kinds 31989, 31990)
- **NIP-90**: Data Vending Machines (kinds 5000-5999, 6000-6999, 7000)

## Acceptance Criteria

### SDK Integration
- [ ] Add nostr-sdk-ios fork as SwiftPM dependency in OpenAgentsCore
  - URL: `https://github.com/OpenAgentsInc/nostr-sdk-ios.git`
  - Version: `0.3.0` or higher (or branch: `main` for latest)
- [ ] Verify existing SDK features work in OpenAgentsCore:
  - [ ] RelayPool multi-relay management
  - [ ] NostrEvent signing/verification
  - [ ] Filter-based subscriptions
  - [ ] NIP-19 bech32 encoding/decoding
  - [ ] NIP-44 encryption/decryption

### NIP-57: Lightning Zaps
- [ ] Add `EventKind.zapRequest` (kind 9734)
- [ ] Add `EventKind.zapReceipt` (kind 9735)
- [ ] Create `ZapRequestEvent` class (extends `NostrEvent`)
  - [ ] Convenience init with `relays`, `amount`, `lnurl`, `p` tags
  - [ ] Computed property: `amountMillisats: Int64?`
  - [ ] Computed property: `recipientPubkey: String?`
  - [ ] Computed property: `lnurl: String?`
- [ ] Create `ZapReceiptEvent` class (extends `NostrEvent`)
  - [ ] Parse `bolt11` tag (Lightning invoice)
  - [ ] Parse `description` tag (original zap request JSON)
  - [ ] Computed property: `bolt11Invoice: String?`
  - [ ] Computed property: `zapRequestJSON: String?`
  - [ ] Verify signature matches BOLT11 description hash (NIP-57 requirement)

### NIP-89: Recommended Application Handlers
- [ ] Add `EventKind.applicationHandlerRecommendation` (kind 31989)
- [ ] Add `EventKind.applicationHandlerInfo` (kind 31990)
- [ ] Create `ApplicationHandlerRecommendationEvent` class
  - [ ] Parse `d` tag (application identifier)
  - [ ] Parse `a` tags (handler references)
  - [ ] Computed property: `recommendedHandlers: [String]`
- [ ] Create `ApplicationHandlerInfoEvent` class
  - [ ] Parse `d` tag (handler identifier)
  - [ ] Parse `k` tag (event kinds supported)
  - [ ] Computed property: `supportedKinds: [Int]`
  - [ ] Computed property: `handlerInfo: [String: String]` (name, about, picture, etc.)

### NIP-90: Data Vending Machines (Client Side)
- [ ] Add DVM event kinds to `EventKind`:
  - [ ] Parameterized range for `jobRequest` (5000-5999)
  - [ ] Parameterized range for `jobResult` (6000-6999)
  - [ ] `jobFeedback` (kind 7000)
- [ ] Create `DVMJobRequestEvent` class
  - [ ] Convenience init with `jobKind`, `inputs`, `bid`, `output`, `relays`, `params`
  - [ ] Parse/create `i` tags (input: url, event, job, text)
  - [ ] Parse/create `bid` tag (amount in msats)
  - [ ] Parse/create `output` tag (MIME type)
  - [ ] Parse/create `relays` tag (where to post results)
  - [ ] Parse/create `param` tags (key-value parameters)
  - [ ] Computed property: `inputs: [DVMInput]`
  - [ ] Computed property: `bidMillisats: Int64?`
  - [ ] Computed property: `expectedOutputMIME: String?`
- [ ] Create `DVMJobResultEvent` class
  - [ ] Parse `request` tag (echo of original job event ID)
  - [ ] Parse `i` tags (inputs processed)
  - [ ] Parse `amount` tag with optional BOLT11
  - [ ] Computed property: `originalRequestId: String?`
  - [ ] Computed property: `bolt11Invoice: String?`
  - [ ] Computed property: `amountMillisats: Int64?`
  - [ ] Support encrypted results (decrypt via NIP-44 if `encrypted` tag present)
- [ ] Create `DVMJobFeedbackEvent` class
  - [ ] Parse `status` tag (payment-required, processing, error, success, partial)
  - [ ] Parse `amount` tag (for payment-required status)
  - [ ] Parse `bolt11` tag (payment invoice)
  - [ ] Computed property: `status: DVMJobStatus`
  - [ ] Computed property: `message: String?`
  - [ ] Computed property: `paymentInfo: (amount: Int64, invoice: String)?`

### EventKind Enum Updates
- [ ] Update `EventKind.allCases` to include NIP-57, NIP-89, NIP-90 kinds
- [ ] Update `EventKind.classForKind` switch to route new kinds to custom classes
- [ ] Ensure `.unknown(rawValue)` fallback still works for unrecognized kinds

### Testing
- [ ] Unit tests for NIP-57 zap event creation/parsing
- [ ] Unit tests for NIP-89 handler recommendation parsing
- [ ] Unit tests for NIP-90 job request/result/feedback creation/parsing
- [ ] Integration test: Create job request → verify serialization
- [ ] Integration test: Parse zap receipt from public relay

## Technical Design

### Integration Approach

**Use the existing SDK** for all core functionality (events, relays, subscriptions, encryption). **Extend the SDK** by forking and adding marketplace-specific event types.

### Package Structure

```swift
// OpenAgentsCore Package.swift dependency:
.package(url: "https://github.com/OpenAgentsInc/nostr-sdk-ios.git", from: "0.3.0")

// Files to add in our fork (https://github.com/OpenAgentsInc/nostr-sdk-ios):
Sources/NostrSDK/
├── EventKind.swift                    // UPDATE: Add NIP-57, NIP-89, NIP-90 cases
├── Events/
│   ├── ZapRequestEvent.swift          // NEW: NIP-57 zap requests
│   ├── ZapReceiptEvent.swift          // NEW: NIP-57 zap receipts
│   ├── ApplicationHandlerRecommendationEvent.swift  // NEW: NIP-89
│   ├── ApplicationHandlerInfoEvent.swift            // NEW: NIP-89
│   ├── DVMJobRequestEvent.swift       // NEW: NIP-90 job requests
│   ├── DVMJobResultEvent.swift        // NEW: NIP-90 job results
│   └── DVMJobFeedbackEvent.swift      // NEW: NIP-90 feedback
```

### Existing SDK Classes (Already Implemented)

The SDK already provides these types - we'll use them directly:

```swift
// RelayPool.swift (already exists)
public final class RelayPool: ObservableObject {
    @Published public private(set) var relays: Set<Relay>
    @Published public private(set) var events: PassthroughSubject<RelayEvent, Never>

    public func add(relay: Relay)
    public func remove(relay: Relay)
    public func connect()
    public func disconnect()
    public func publishEvent(_ event: NostrEvent)
    public func subscribe(with filter: Filter, subscriptionId: String = UUID().uuidString) -> String
    public func closeSubscription(with subscriptionId: String)
}

// NostrEvent.swift (already exists)
public class NostrEvent: Codable, Equatable, Hashable {
    public let id: String           // 32-byte hex sha256
    public let pubkey: String       // 32-byte hex public key
    public let createdAt: Int64     // Unix timestamp
    public let kind: EventKind
    public let tags: [Tag]
    public let content: String
    public let signature: String?

    // Create unsigned rumor
    required init(kind: EventKind, content: String, tags: [Tag] = [],
                  createdAt: Int64 = Int64(Date.now.timeIntervalSince1970),
                  pubkey: String)

    // Create signed event
    required init(kind: EventKind, content: String, tags: [Tag] = [],
                  createdAt: Int64 = Int64(Date.now.timeIntervalSince1970),
                  signedBy keypair: Keypair) throws

    // Helper methods
    public func firstValueForTagName(_ tag: TagName) -> String?
    public func allTags(withTagName tagName: TagName) -> [Tag]
}

// Filter.swift (already exists)
public struct Filter: Codable, Equatable, Hashable {
    public var ids: [String]?
    public var authors: [String]?
    public var kinds: [EventKind]?
    public var e: [String]?      // Referenced event IDs
    public var p: [String]?      // Referenced pubkeys
    public var since: Int64?
    public var until: Int64?
    public var limit: Int?
}
```

### New Marketplace Event Types (To Add in Fork)

#### 1. EventKind Enum Updates

```swift
// EventKind.swift (UPDATE existing file)

public enum EventKind: RawRepresentable, CaseIterable, Codable, Equatable, Hashable {
    // ... existing cases ...

    // NIP-57: Lightning Zaps
    case zapRequest        // 9734
    case zapReceipt        // 9735

    // NIP-89: Application Handlers
    case applicationHandlerRecommendation  // 31989
    case applicationHandlerInfo            // 31990

    // NIP-90: Data Vending Machines
    case jobRequest(Int)   // 5000-5999 (parameterized range)
    case jobResult(Int)    // 6000-6999 (parameterized range)
    case jobFeedback       // 7000

    case unknown(RawValue)

    public var rawValue: RawValue {
        switch self {
        // ... existing cases ...
        case .zapRequest:                    return 9734
        case .zapReceipt:                    return 9735
        case .applicationHandlerRecommendation: return 31989
        case .applicationHandlerInfo:        return 31990
        case .jobRequest(let kind):          return kind  // 5000-5999
        case .jobResult(let kind):           return kind  // 6000-6999
        case .jobFeedback:                   return 7000
        case let .unknown(value):            return value
        }
    }

    public init(rawValue: Int) {
        switch rawValue {
        case 9734: self = .zapRequest
        case 9735: self = .zapReceipt
        case 31989: self = .applicationHandlerRecommendation
        case 31990: self = .applicationHandlerInfo
        case 5000...5999: self = .jobRequest(rawValue)
        case 6000...6999: self = .jobResult(rawValue)
        case 7000: self = .jobFeedback
        default:
            if let match = Self.allCases.first(where: { $0.rawValue == rawValue }) {
                self = match
            } else {
                self = .unknown(rawValue)
            }
        }
    }

    public var classForKind: NostrEvent.Type {
        switch self {
        // ... existing cases ...
        case .zapRequest:                    return ZapRequestEvent.self
        case .zapReceipt:                    return ZapReceiptEvent.self
        case .applicationHandlerRecommendation: return ApplicationHandlerRecommendationEvent.self
        case .applicationHandlerInfo:        return ApplicationHandlerInfoEvent.self
        case .jobRequest:                    return DVMJobRequestEvent.self
        case .jobResult:                     return DVMJobResultEvent.self
        case .jobFeedback:                   return DVMJobFeedbackEvent.self
        case .unknown:                       return NostrEvent.self
        }
    }
}
```

#### 2. NIP-57: Zap Events

```swift
// ZapRequestEvent.swift (NEW file)

/// NIP-57 Zap Request (kind 9734)
public final class ZapRequestEvent: NostrEvent {
    /// Amount in millisats (from `amount` tag)
    public var amountMillisats: Int64? {
        guard let amountStr = firstValueForTagName(.custom(name: "amount")) else { return nil }
        return Int64(amountStr)
    }

    /// Recipient pubkey (from `p` tag)
    public var recipientPubkey: String? {
        firstValueForTagName(.pubkey)
    }

    /// LNURL (from `lnurl` tag)
    public var lnurl: String? {
        firstValueForTagName(.custom(name: "lnurl"))
    }

    /// Relay URLs (from `relays` tag)
    public var relayURLs: [String] {
        allValues(forTagName: .custom(name: "relays"))
    }

    /// Convenience initializer for creating zap requests
    public init(
        recipientPubkey: String,
        amountMillisats: Int64,
        lnurl: String,
        relayURLs: [String],
        content: String = "",
        signedBy keypair: Keypair
    ) throws {
        var tags: [Tag] = [
            Tag(name: "p", value: recipientPubkey),
            Tag(name: "amount", value: String(amountMillisats)),
            Tag(name: "lnurl", value: lnurl)
        ]
        if !relayURLs.isEmpty {
            tags.append(Tag(name: "relays", otherParameters: relayURLs))
        }

        try super.init(
            kind: .zapRequest,
            content: content,
            tags: tags,
            signedBy: keypair
        )
    }
}

/// NIP-57 Zap Receipt (kind 9735)
public final class ZapReceiptEvent: NostrEvent {
    /// BOLT11 Lightning invoice (from `bolt11` tag)
    public var bolt11Invoice: String? {
        firstValueForTagName(.custom(name: "bolt11"))
    }

    /// Zap request JSON (from `description` tag)
    public var zapRequestJSON: String? {
        firstValueForTagName(.custom(name: "description"))
    }

    /// Parse zap request from description tag
    public var zapRequest: ZapRequestEvent? {
        guard let json = zapRequestJSON,
              let data = json.data(using: .utf8),
              let event = try? JSONDecoder().decode(ZapRequestEvent.self, from: data) else {
            return nil
        }
        return event
    }
}
```

#### 3. NIP-89: Application Handler Events

```swift
// ApplicationHandlerRecommendationEvent.swift (NEW file)

/// NIP-89 Application Handler Recommendation (kind 31989)
public final class ApplicationHandlerRecommendationEvent: NostrEvent {
    /// Application identifier (from `d` tag)
    public var applicationIdentifier: String? {
        firstValueForTagName(.custom(name: "d"))
    }

    /// Recommended handler references (from `a` tags)
    public var recommendedHandlers: [String] {
        allValues(forTagName: .custom(name: "a"))
    }
}

// ApplicationHandlerInfoEvent.swift (NEW file)

/// NIP-89 Application Handler Info (kind 31990)
public final class ApplicationHandlerInfoEvent: NostrEvent {
    /// Handler identifier (from `d` tag)
    public var handlerIdentifier: String? {
        firstValueForTagName(.custom(name: "d"))
    }

    /// Supported event kinds (from `k` tag)
    public var supportedKinds: [Int] {
        firstValueForTagName(.custom(name: "k"))?
            .split(separator: ",")
            .compactMap { Int($0.trimmingCharacters(in: .whitespaces)) } ?? []
    }

    /// Handler metadata (parsed from content JSON)
    public var handlerInfo: [String: String]? {
        guard let data = content.data(using: .utf8),
              let dict = try? JSONSerialization.jsonObject(with: data) as? [String: String] else {
            return nil
        }
        return dict
    }
}
```

#### 4. NIP-90: DVM Events

```swift
// DVMJobRequestEvent.swift (NEW file)

public enum DVMInput: Codable, Equatable {
    case url(String)
    case event(String)
    case job(String)
    case text(String)
}

/// NIP-90 Job Request (kinds 5000-5999)
public final class DVMJobRequestEvent: NostrEvent {
    /// Inputs (from `i` tags)
    public var inputs: [DVMInput] {
        allTags(withTagName: .custom(name: "i")).compactMap { tag in
            guard tag.otherParameters.count >= 2 else { return nil }
            let value = tag.value
            let type = tag.otherParameters[0]

            switch type {
            case "url": return .url(value)
            case "event": return .event(value)
            case "job": return .job(value)
            case "text": return .text(value)
            default: return nil
            }
        }
    }

    /// Bid amount in millisats (from `bid` tag)
    public var bidMillisats: Int64? {
        guard let bidStr = firstValueForTagName(.custom(name: "bid")) else { return nil }
        return Int64(bidStr)
    }

    /// Expected output MIME type (from `output` tag)
    public var expectedOutputMIME: String? {
        firstValueForTagName(.custom(name: "output"))
    }

    /// Relay URLs for results (from `relays` tag)
    public var relayURLs: [String] {
        allValues(forTagName: .custom(name: "relays"))
    }

    /// Custom parameters (from `param` tags)
    public var parameters: [String: String] {
        var params: [String: String] = [:]
        for tag in allTags(withTagName: .custom(name: "param")) {
            guard tag.otherParameters.count >= 2 else { continue }
            params[tag.value] = tag.otherParameters[0]
        }
        return params
    }
}

// DVMJobResultEvent.swift (NEW file)

/// NIP-90 Job Result (kinds 6000-6999)
public final class DVMJobResultEvent: NostrEvent {
    /// Original job request ID (from `request` tag)
    public var originalRequestId: String? {
        firstValueForTagName(.custom(name: "request"))
    }

    /// BOLT11 invoice (from `amount` tag)
    public var bolt11Invoice: String? {
        let amountTag = allTags(withTagName: .custom(name: "amount")).first
        return amountTag?.otherParameters.first
    }

    /// Amount in millisats (from `amount` tag value)
    public var amountMillisats: Int64? {
        guard let amountStr = firstValueForTagName(.custom(name: "amount")) else { return nil }
        return Int64(amountStr)
    }
}

// DVMJobFeedbackEvent.swift (NEW file)

public enum DVMJobStatus: String, Codable {
    case paymentRequired = "payment-required"
    case processing = "processing"
    case error = "error"
    case success = "success"
    case partial = "partial"
}

/// NIP-90 Job Feedback (kind 7000)
public final class DVMJobFeedbackEvent: NostrEvent {
    /// Job status (from `status` tag)
    public var status: DVMJobStatus {
        guard let statusStr = firstValueForTagName(.custom(name: "status")),
              let status = DVMJobStatus(rawValue: statusStr) else {
            return .error
        }
        return status
    }

    /// Status message (event content)
    public var message: String? {
        content.isEmpty ? nil : content
    }

    /// Payment info (amount + invoice) for payment-required status
    public var paymentInfo: (amount: Int64, invoice: String)? {
        guard status == .paymentRequired,
              let amountStr = firstValueForTagName(.custom(name: "amount")),
              let amount = Int64(amountStr) else {
            return nil
        }

        let amountTag = allTags(withTagName: .custom(name: "amount")).first
        guard let invoice = amountTag?.otherParameters.first else {
            return nil
        }

        return (amount, invoice)
    }
}
```

## Dependencies

### Swift Packages
- **nostr-sdk-ios (OpenAgents fork)**: Comprehensive Nostr SDK
  - URL: `https://github.com/OpenAgentsInc/nostr-sdk-ios.git`
  - Version: `from: "0.3.0"` or `branch: "main"`
  - Includes all dependencies: secp256k1.swift, CryptoSwift, Bech32

The SDK already includes:
- **secp256k1.swift** ~2.0.0: Schnorr signatures, ECDH
- **CryptoSwift** ~1.8.0: AES-256-CBC encryption
- **Bech32**: bech32 encoding/decoding

### System Frameworks (via SDK)
- **Foundation**: Core types, networking
- **CryptoKit**: Hashing (SHA-256)
- **Combine**: Publishers for relay events
- **os.log**: Logging

### OpenAgents Dependencies
- None (this is the foundation layer)

## Testing Requirements

### Unit Tests (SDK Core - Already Exist)
The SDK already has comprehensive unit tests for:
- ✅ Event ID calculation
- ✅ Event signing and verification
- ✅ Event serialization/deserialization
- ✅ Filter matching
- ✅ NIP-04/NIP-44 encryption/decryption
- ✅ BECH32 encoding/decoding

### Unit Tests (New - For Marketplace NIPs)
- [ ] NIP-57 zap event creation/parsing
  - [ ] ZapRequestEvent: parse amount, pubkey, lnurl tags
  - [ ] ZapReceiptEvent: parse bolt11, description tags
- [ ] NIP-89 handler event parsing
  - [ ] Parse `d`, `a`, `k` tags correctly
  - [ ] Handler info JSON deserialization
- [ ] NIP-90 DVM event creation/parsing
  - [ ] DVMJobRequestEvent: parse inputs, bid, output, params
  - [ ] DVMJobResultEvent: parse request, amount, bolt11
  - [ ] DVMJobFeedbackEvent: parse status, payment info
- [ ] EventKind enum updates
  - [ ] Parameterized ranges work (5000-5999, 6000-6999)
  - [ ] `classForKind` routes to correct event classes

### Integration Tests
- [ ] Verify SDK integration in OpenAgentsCore:
  - [ ] Import NostrSDK package
  - [ ] Create RelayPool with public relays
  - [ ] Subscribe to marketplace events (kinds 5000-5999, 9734, 9735)
- [ ] End-to-end job flow:
  - [ ] Create DVMJobRequestEvent → publish to relay
  - [ ] Subscribe to feedback (kind 7000)
  - [ ] Subscribe to results (kinds 6000-6999)
  - [ ] Parse result event and extract BOLT11 invoice

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 2.5.2 (No Executing Downloaded Code)**
- ✅ **Compliant**: Nostr events are **data**, not executable code
- ✅ This library only handles JSON events; no script execution
- ⚠️  **Caution**: Ensure NIP-90 jobs on marketplace send **prompts/data**, not code to execute

**ASRG 5.1.1 (Privacy)**
- ✅ Nostr is decentralized; no single data controller
- ✅ Encryption (NIP-04) for sensitive content
- ⚠️  **Action Required**: Add privacy policy explaining Nostr relay data is public (except encrypted DMs)

**ASRG 5.1.2 (Data Use and Sharing)**
- ⚠️  **Disclosure Needed**: Nostr events are broadcast to multiple relays (user should consent)
- ✅ Events are **opt-in** (user creates/sends explicitly)

### DPLA Compliance

**No specific DPLA concerns** for Nostr library itself (it's a protocol implementation).

**Foundation Models AUP** (if jobs use Foundation Models):
- ⚠️  Jobs created via NIP-90 **must not** request prohibited content (regulated health/legal/finance services)
- ✅ **Mitigation**: Policy module (issue #009) will filter jobs before accepting

### Privacy Best Practices

1. **Local-Only Keys**: Private keys stored in Secure Enclave (not sent to relays)
2. **Encrypted Params**: Use NIP-04 for sensitive job parameters
3. **Relay Selection**: Allow user to configure which relays to use
4. **Event History**: Don't cache events longer than necessary (or make it opt-in)

## Reference Links

### nostr-sdk-ios
- **OpenAgents Fork**: https://github.com/OpenAgentsInc/nostr-sdk-ios
- **Original Repo**: https://github.com/nostr-sdk/nostr-sdk-ios
- **Documentation**: https://nostr-sdk.github.io/nostr-sdk-ios/documentation/nostrsdk/
- **SDK README**: Full NIP compliance status in fork's README.md

### Marketplace NIPs (To Implement)
- **NIP-57**: Lightning Zaps - https://github.com/nostr-protocol/nips/blob/master/57.md
- **NIP-89**: Recommended Application Handlers - https://github.com/nostr-protocol/nips/blob/master/89.md
- **NIP-90**: Data Vending Machines - https://github.com/nostr-protocol/nips/blob/master/90.md

### Core NIPs (Already in SDK)
- **NIP-01**: Basic protocol flow - https://github.com/nostr-protocol/nips/blob/master/01.md
- **NIP-04**: Encrypted direct messages (deprecated) - https://github.com/nostr-protocol/nips/blob/master/04.md
- **NIP-19**: bech32-encoded entities - https://github.com/nostr-protocol/nips/blob/master/19.md
- **NIP-44**: Encrypted payloads (modern) - https://github.com/nostr-protocol/nips/blob/master/44.md
- **NIP-59**: Gift wrap - https://github.com/nostr-protocol/nips/blob/master/59.md

### OpenAgents Documentation
- **ADR-0002**: Agent Client Protocol (complementary protocol)
- **ADR-0008**: Breez Spark SDK for Marketplace Payments (works with NIP-57)
- **Spark SDK Integration**: `docs/compute/issues/SPARK-SDK-INTEGRATION.md`

## Success Metrics

- [ ] nostr-sdk-ios fork integrated as SPM dependency in OpenAgentsCore
- [ ] All new marketplace NIP tests pass (NIP-57, NIP-89, NIP-90)
- [ ] Can create and parse all marketplace event types
- [ ] Integration test: Full DVM job flow works end-to-end
- [ ] EventKind routing works for all new marketplace kinds
- [ ] No breaking changes to existing SDK functionality
- [ ] Fork documented with clear "what we changed" section in README

## Notes

### SDK Integration
- **Minimum Requirements**: SDK requires Swift 5.7, iOS 15, macOS 12 (compatible with our iOS 16/macOS 13 targets)
- **Fork Maintenance**: Pull upstream updates periodically for non-marketplace NIPs
- **Shallow Fork Strategy**: Minimize divergence (~850 LOC additions) to ease future merges

### Implementation Approach
- **Extend, Don't Replace**: Use SDK's existing RelayPool, NostrEvent, Filter classes
- **Type-Safe Event Kinds**: Add explicit cases (not just `.unknown(rawValue)`) for marketplace NIPs
- **Leverage SDK Features**: Use existing encryption (NIP-44), bech32 (NIP-19), relay management

### Relay Configuration
- **Default Relays**: `wss://relay.damus.io`, `wss://nos.lol`, `wss://relay.snort.social`
- **Marketplace Relays**: May need dedicated DVM relays (TBD based on NIP-90 ecosystem)

## Future Enhancements (Post-MVP)

Already in SDK (available for future use):
- ✅ NIP-05 DNS-based verification
- ✅ NIP-09 Event deletion
- ✅ NIP-25 Reactions
- ✅ NIP-65 Relay list metadata

Not in SDK (may add later):
- NIP-42 Authentication to relays
- NIP-47 Nostr Wallet Connect (alternative to Spark SDK)
- NIP-51 Lists (mute lists, bookmarks)
- SQLite persistent event cache (SDK has in-memory only)
