# Nostr Client Library (Swift)

**Phase:** 1 - MVP
**Component:** OpenAgentsCore (Shared)
**Priority:** P0 (Critical Path - Blocks all marketplace features)
**Estimated Effort:** 4-6 weeks

## Summary

Build a comprehensive Nostr client library in Swift for both iOS and macOS, implementing core NIPs (Nostr Implementation Possibilities) required for the compute marketplace. This library will handle event signing, relay communication, subscriptions, and provide the foundation for all marketplace interactions.

## Motivation

The compute marketplace architecture requires Nostr as the decentralized protocol layer for:

- **Job discovery and submission** (NIP-90 Data Vending Machine)
- **Lightning payments** (NIP-57 Zaps)
- **Identity management** (NIP-01 events + NIP-19 BECH32 identifiers)
- **Privacy** (NIP-04 encrypted direct messages)
- **Provider advertising** (NIP-89 capabilities, NIP-11 relay metadata)

Without a robust Nostr client, neither iOS marketplace coordination nor macOS worker functionality can be implemented. This is the critical path dependency for the entire marketplace vision.

## Acceptance Criteria

### Core Event Model (NIP-01)
- [ ] `NostrEvent` struct with required fields:
  - `id`: String (32-byte hex sha256)
  - `pubkey`: String (32-byte hex)
  - `created_at`: Int64 (unix timestamp)
  - `kind`: Int
  - `tags`: [[String]]
  - `content`: String
  - `sig`: String (64-byte hex Schnorr signature)
- [ ] Event ID calculation (SHA-256 of serialized event)
- [ ] Event signing with secp256k1 (Schnorr signatures)
- [ ] Event verification (signature validation)
- [ ] Event serialization to/from JSON
- [ ] Filter struct for subscriptions (authors, kinds, tags, since, until, limit)

### Relay Communication (NIP-01)
- [ ] WebSocket connection management using `URLSessionWebSocketTask` (iOS/macOS)
- [ ] Send MESSAGE types: `EVENT`, `REQ` (subscribe), `CLOSE` (unsubscribe)
- [ ] Receive MESSAGE types: `EVENT`, `NOTICE`, `EOSE` (end of stored events), `OK`
- [ ] Connection state management (disconnected, connecting, connected, error)
- [ ] Automatic reconnection with exponential backoff
- [ ] Ping/pong keep-alive
- [ ] Graceful disconnect and cleanup

### Multi-Relay Manager
- [ ] `RelayManager` class to coordinate multiple relay connections
- [ ] Add/remove relays at runtime
- [ ] Broadcast events to all connected relays (with partial failure handling)
- [ ] Subscribe to filters across multiple relays (deduplicate events by ID)
- [ ] Relay quality tracking (latency, success rate, uptime)
- [ ] Backpressure handling (queue limits, drop oldest on overflow)
- [ ] Per-relay connection pooling (reuse connections)

### Encrypted Direct Messages (NIP-04)
- [ ] ECDH shared secret derivation (secp256k1)
- [ ] AES-256-CBC encryption/decryption with IV
- [ ] Base64 encoding for encrypted content
- [ ] `kind:4` event creation with encrypted content
- [ ] Decrypt incoming `kind:4` events
- [ ] Key derivation from secp256k1 private key

### BECH32 Encoding (NIP-19)
- [ ] Encode public key → `npub1...`
- [ ] Encode private key → `nsec1...`
- [ ] Encode note ID → `note1...`
- [ ] Decode `npub/nsec/note` → raw hex
- [ ] Validation and error handling for malformed identifiers

### Lightning Zaps (NIP-57)
- [ ] Parse `zap` event (`kind:9735`)
- [ ] Extract BOLT11 invoice from `description` tag
- [ ] Verify zap receipt signature
- [ ] Create zap request event (`kind:9734`)
- [ ] Amount encoding/decoding (millisats)

### Data Vending Machine - Client Side (NIP-90)
- [ ] Create job request events (`kind:5000-5999`)
  - Support `i` tags (input: url, event, job, text)
  - Support `bid` tag (amount in msats)
  - Support `output` tag (MIME type)
  - Support `relays` tag (where to post results)
  - Support `param` tags (custom parameters)
- [ ] Parse job result events (`kind:6000-6999`)
  - Extract `request` (echo of original job)
  - Extract `i` tags (inputs processed)
  - Extract `amount` with optional BOLT11
  - Decrypt encrypted results (NIP-04)
- [ ] Parse feedback events (`kind:7000`)
  - `status` tag values: `payment-required`, `processing`, `error`, `success`, `partial`
  - `amount` tag for payment requests
- [ ] Job state tracking (request → feedback → result correlation)

### Subscription Management
- [ ] `Subscription` class with unique ID, filters, and event callback
- [ ] Subscribe/unsubscribe to filters
- [ ] Auto-resubscribe on relay reconnect
- [ ] Subscription deduplication (same filter = same subscription)
- [ ] Event routing to multiple subscribers (one-to-many)
- [ ] Memory management (weak references to avoid leaks)

### Caching
- [ ] In-memory event cache (LRU, configurable size)
- [ ] Cache lookups by event ID
- [ ] Cache invalidation on relay disconnect
- [ ] Optional persistent cache (SQLite) for offline access

### Error Handling
- [ ] Define `NostrError` enum:
  - `invalidEvent`, `invalidSignature`, `invalidFilter`
  - `relayConnectionFailed`, `relayTimeout`, `relayRejectedEvent`
  - `encryptionFailed`, `decryptionFailed`
  - `bech32EncodingFailed`, `bech32DecodingFailed`
- [ ] Structured error context (relay URL, event ID, etc.)
- [ ] Logging (os_log for development, configurable in production)

## Technical Design

### Package Structure

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Nostr/

NostrCore.swift              // Event model, signing, verification
NostrRelay.swift             // Single relay WebSocket connection
NostrRelayManager.swift      // Multi-relay coordinator
NostrSubscription.swift      // Subscription management
NostrEncryption.swift        // NIP-04 encryption
NostrBech32.swift            // NIP-19 encoding/decoding
NostrZaps.swift              // NIP-57 zap helpers
NostrDVM.swift               // NIP-90 Data Vending Machine (client)
NostrFilter.swift            // Filter construction
NostrError.swift             // Error types
NostrCache.swift             // Event caching
```

### Core Types

```swift
// NostrCore.swift

/// Nostr event (NIP-01)
public struct NostrEvent: Codable, Identifiable, Equatable {
    public let id: String           // 32-byte hex sha256
    public let pubkey: String       // 32-byte hex
    public let created_at: Int64    // unix timestamp
    public let kind: Int
    public let tags: [[String]]
    public let content: String
    public let sig: String          // 64-byte hex Schnorr signature

    /// Calculate event ID (SHA-256 of serialized event)
    public static func calculateId(
        pubkey: String,
        created_at: Int64,
        kind: Int,
        tags: [[String]],
        content: String
    ) -> String

    /// Sign event with private key
    public static func sign(
        privateKey: String,  // 32-byte hex
        created_at: Int64,
        kind: Int,
        tags: [[String]],
        content: String
    ) throws -> NostrEvent

    /// Verify event signature
    public func verify() -> Bool

    /// Find first tag with given name
    public func tag(_ name: String) -> [String]?

    /// Find all tags with given name
    public func tags(_ name: String) -> [[String]]
}

/// Subscription filter (NIP-01)
public struct NostrFilter: Codable, Equatable, Hashable {
    public var ids: [String]?          // Event IDs
    public var authors: [String]?      // Pubkeys
    public var kinds: [Int]?           // Event kinds
    public var e: [String]?            // Referenced event IDs
    public var p: [String]?            // Referenced pubkeys
    public var since: Int64?           // Unix timestamp
    public var until: Int64?           // Unix timestamp
    public var limit: Int?             // Max events

    public init(
        ids: [String]? = nil,
        authors: [String]? = nil,
        kinds: [Int]? = nil,
        e: [String]? = nil,
        p: [String]? = nil,
        since: Int64? = nil,
        until: Int64? = nil,
        limit: Int? = nil
    )
}

/// Relay message types (NIP-01)
public enum NostrRelayMessage {
    case event(subscriptionId: String, event: NostrEvent)
    case notice(message: String)
    case eose(subscriptionId: String)  // End of stored events
    case ok(eventId: String, accepted: Bool, message: String)
}

public enum NostrClientMessage {
    case event(NostrEvent)
    case req(subscriptionId: String, filters: [NostrFilter])
    case close(subscriptionId: String)
}
```

### Relay Connection

```swift
// NostrRelay.swift

public protocol NostrRelayDelegate: AnyObject {
    func relay(_ relay: NostrRelay, didReceive event: NostrEvent, subscriptionId: String)
    func relay(_ relay: NostrRelay, didReceive message: NostrRelayMessage)
    func relay(_ relay: NostrRelay, didChangeState state: NostrRelay.State)
    func relay(_ relay: NostrRelay, didFailWithError error: NostrError)
}

public class NostrRelay {
    public enum State {
        case disconnected
        case connecting
        case connected
        case error(NostrError)
    }

    public let url: URL
    public private(set) var state: State = .disconnected
    public weak var delegate: NostrRelayDelegate?

    private var webSocketTask: URLSessionWebSocketTask?
    private let session: URLSession
    private var pingTimer: Timer?
    private var reconnectAttempts: Int = 0

    public init(url: URL) {
        self.url = url
        self.session = URLSession(configuration: .default)
    }

    /// Connect to relay
    public func connect()

    /// Disconnect from relay
    public func disconnect()

    /// Send event to relay
    public func send(_ event: NostrEvent) async throws

    /// Subscribe to filter
    public func subscribe(id: String, filters: [NostrFilter]) async throws

    /// Unsubscribe
    public func unsubscribe(id: String) async throws

    // Private methods
    private func receiveMessage()
    private func sendPing()
    private func handleReconnect()
    private func parseMessage(_ text: String) -> NostrRelayMessage?
}
```

### Relay Manager

```swift
// NostrRelayManager.swift

public class NostrRelayManager {
    public typealias EventHandler = (NostrEvent, String) -> Void  // event, relay URL

    private var relays: [String: NostrRelay] = [:]
    private var subscriptions: [String: Subscription] = [:]
    private let eventCache: NostrCache

    public struct RelayStats {
        public let url: String
        public let connected: Bool
        public let latency: TimeInterval?
        public let successRate: Double
        public let eventCount: Int
    }

    private class Subscription {
        let id: String
        let filters: [NostrFilter]
        let handler: EventHandler
        let relayURLs: Set<String>  // Which relays to subscribe on

        init(id: String, filters: [NostrFilter], relayURLs: Set<String>, handler: @escaping EventHandler) {
            self.id = id
            self.filters = filters
            self.relayURLs = relayURLs
            self.handler = handler
        }
    }

    public init(cacheSize: Int = 1000) {
        self.eventCache = NostrCache(capacity: cacheSize)
    }

    /// Add relay
    public func addRelay(url: String) async throws

    /// Remove relay
    public func removeRelay(url: String)

    /// Connect to all relays
    public func connectAll()

    /// Disconnect from all relays
    public func disconnectAll()

    /// Broadcast event to all connected relays
    public func broadcast(_ event: NostrEvent) async -> [String: Result<Void, NostrError>]

    /// Subscribe to filters across specified relays (or all if nil)
    public func subscribe(
        id: String,
        filters: [NostrFilter],
        relays: Set<String>? = nil,
        handler: @escaping EventHandler
    ) async throws

    /// Unsubscribe
    public func unsubscribe(id: String)

    /// Get stats for all relays
    public func getStats() -> [RelayStats]

    // Event deduplication (by ID)
    private func handleEvent(_ event: NostrEvent, from relayURL: String)
}
```

### Encryption (NIP-04)

```swift
// NostrEncryption.swift

public struct NostrEncryption {
    /// Encrypt content for recipient
    public static func encrypt(
        content: String,
        privateKey: String,   // Sender's private key (hex)
        recipientPubkey: String  // Recipient's public key (hex)
    ) throws -> String  // Base64 encoded: IV + encrypted content

    /// Decrypt content from sender
    public static func decrypt(
        encryptedContent: String,  // Base64 encoded
        privateKey: String,         // Receiver's private key (hex)
        senderPubkey: String       // Sender's public key (hex)
    ) throws -> String

    /// Derive shared secret using ECDH
    private static func deriveSharedSecret(
        privateKey: String,
        publicKey: String
    ) throws -> Data
}
```

### BECH32 (NIP-19)

```swift
// NostrBech32.swift

public enum Bech32Prefix: String {
    case npub = "npub"   // Public key
    case nsec = "nsec"   // Private key
    case note = "note"   // Note ID (event ID)
}

public struct NostrBech32 {
    /// Encode hex to bech32
    public static func encode(hex: String, prefix: Bech32Prefix) throws -> String

    /// Decode bech32 to hex
    public static func decode(_ bech32: String) throws -> (hex: String, prefix: Bech32Prefix)

    // Convenience methods
    public static func npub(from pubkeyHex: String) throws -> String
    public static func nsec(from privkeyHex: String) throws -> String
    public static func note(from eventIdHex: String) throws -> String
}
```

### Data Vending Machine (NIP-90)

```swift
// NostrDVM.swift

public struct DVMJobRequest {
    public let kind: Int             // 5000-5999
    public let inputs: [DVMInput]
    public let bid: Int64?           // msats
    public let output: String?       // MIME type
    public let relays: [String]?     // Where to post results
    public let params: [String: String]?  // Custom params
    public let encrypted: Bool       // Encrypt params?

    public enum DVMInput {
        case url(String)
        case event(String)  // Event ID
        case job(String)    // Job ID
        case text(String)
    }

    /// Create Nostr event for this job request
    public func toEvent(privateKey: String, recipientPubkey: String?) throws -> NostrEvent
}

public struct DVMJobResult {
    public let jobRequestId: String
    public let inputs: [String]
    public let content: String       // Decrypted if necessary
    public let amount: Int64?        // msats
    public let bolt11: String?       // Payment invoice
    public let providerPubkey: String

    /// Parse from Nostr event (kind:6000-6999)
    public static func from(event: NostrEvent, privateKey: String?) throws -> DVMJobResult
}

public struct DVMFeedback {
    public enum Status: String {
        case paymentRequired = "payment-required"
        case processing = "processing"
        case error = "error"
        case success = "success"
        case partial = "partial"
    }

    public let jobRequestId: String
    public let status: Status
    public let message: String?
    public let amount: Int64?
    public let bolt11: String?

    /// Parse from Nostr event (kind:7000)
    public static func from(event: NostrEvent) throws -> DVMFeedback
}
```

### Error Types

```swift
// NostrError.swift

public enum NostrError: Error, LocalizedError {
    case invalidEvent(reason: String)
    case invalidSignature
    case invalidFilter(reason: String)
    case invalidPublicKey
    case invalidPrivateKey

    case relayConnectionFailed(url: String, underlying: Error?)
    case relayTimeout(url: String)
    case relayRejectedEvent(eventId: String, message: String)
    case relayNotConnected(url: String)

    case encryptionFailed(reason: String)
    case decryptionFailed(reason: String)

    case bech32EncodingFailed(reason: String)
    case bech32DecodingFailed(reason: String)

    case invalidDVMEvent(reason: String)

    public var errorDescription: String? {
        switch self {
        case .invalidEvent(let reason):
            return "Invalid Nostr event: \(reason)"
        case .invalidSignature:
            return "Invalid event signature"
        // ... other cases
        }
    }
}
```

### Caching

```swift
// NostrCache.swift

public class NostrCache {
    private let capacity: Int
    private var cache: [String: CacheEntry] = [:]
    private var accessOrder: [String] = []  // LRU tracking

    private struct CacheEntry {
        let event: NostrEvent
        let timestamp: Date
    }

    public init(capacity: Int) {
        self.capacity = capacity
    }

    public func store(_ event: NostrEvent)
    public func get(_ id: String) -> NostrEvent?
    public func contains(_ id: String) -> Bool
    public func clear()

    private func evictIfNeeded()
}
```

## Dependencies

### Swift Packages
- **secp256k1.swift**: For Schnorr signatures and ECDH
  - URL: `https://github.com/GigaBitcoin/secp256k1.swift`
  - Version: ~2.0.0
- **CryptoSwift** (optional): For AES-256-CBC if not using CommonCrypto
  - URL: `https://github.com/krzyzanowskim/CryptoSwift`
  - Version: ~1.8.0

### System Frameworks
- **Foundation**: Core types, networking
- **CryptoKit**: Hashing (SHA-256)
- **CommonCrypto** (macOS/iOS): AES encryption (NIP-04)

### OpenAgents Dependencies
- None (this is the foundation)

## Testing Requirements

### Unit Tests
- [ ] Event ID calculation (test vectors from NIP-01)
- [ ] Event signing and verification (Schnorr)
- [ ] Event serialization/deserialization
- [ ] Filter matching logic
- [ ] NIP-04 encryption/decryption (test vectors)
- [ ] BECH32 encoding/decoding (test vectors from NIP-19)
- [ ] DVM event creation/parsing (NIP-90 examples)
- [ ] Zap event parsing (NIP-57 examples)
- [ ] Error handling for malformed events

### Integration Tests
- [ ] Connect to public relay (wss://relay.damus.io)
- [ ] Publish event and verify `OK` response
- [ ] Subscribe to filter and receive events
- [ ] Multi-relay subscription with deduplication
- [ ] Relay reconnection on disconnect
- [ ] Backpressure handling (flood with events)

### E2E Tests
- [ ] Full job request flow:
  1. Create job request (NIP-90)
  2. Publish to relay
  3. Subscribe to feedback (kind:7000)
  4. Receive result (kind:6000-6999)
- [ ] Encrypted DM round-trip (NIP-04)
- [ ] Zap send/receive (NIP-57)

### Performance Tests
- [ ] Event verification throughput (>1000 events/sec)
- [ ] Relay message parsing (>500 messages/sec)
- [ ] Cache performance (lookups <1ms)
- [ ] Memory usage under load (no leaks)

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

### Nostr NIPs
- **NIP-01**: Basic protocol flow - https://github.com/nostr-protocol/nips/blob/master/01.md
- **NIP-04**: Encrypted direct messages - https://github.com/nostr-protocol/nips/blob/master/04.md
- **NIP-05**: DNS-based verification - https://github.com/nostr-protocol/nips/blob/master/05.md
- **NIP-11**: Relay information document - https://github.com/nostr-protocol/nips/blob/master/11.md
- **NIP-19**: bech32-encoded entities - https://github.com/nostr-protocol/nips/blob/master/19.md
- **NIP-57**: Lightning Zaps - https://github.com/nostr-protocol/nips/blob/master/57.md
- **NIP-90**: Data Vending Machine - https://github.com/nostr-protocol/nips/blob/master/90.md

### Apple Documentation
- **URLSession WebSocket**: https://developer.apple.com/documentation/foundation/urlsessionwebsockettask
- **CryptoKit**: https://developer.apple.com/documentation/cryptokit
- **Secure Enclave**: https://developer.apple.com/documentation/security/certificate_key_and_trust_services/keys/storing_keys_in_the_secure_enclave

### External Libraries
- **secp256k1.swift**: https://github.com/GigaBitcoin/secp256k1.swift
- **CryptoSwift**: https://github.com/krzyzanowskim/CryptoSwift

### OpenAgents ADRs
- **ADR-0002**: Agent Client Protocol (ACP transport layer reference)
- **ADR-0004**: iOS ↔ Desktop WebSocket Bridge (WebSocket patterns)

## Success Metrics

- [ ] Library compiles on iOS 16.0+ and macOS 13.0+
- [ ] All unit tests pass (90%+ code coverage)
- [ ] Integration tests pass against 3+ public relays
- [ ] Event verification >1000 events/sec on M1/M2 hardware
- [ ] No memory leaks under sustained load (Instruments)
- [ ] Published as internal SwiftPM package in OpenAgentsCore
- [ ] API documentation (DocC) generated and reviewed

## Notes

- **Secp256k1 Dependency**: Use `secp256k1.swift` for Schnorr signatures (NIP-01 requirement)
- **WebSocket Library**: Use native `URLSessionWebSocketTask` (available iOS 13+, macOS 10.15+)
- **Relay Recommendations**: Default to popular relays: `wss://relay.damus.io`, `wss://nos.lol`, `wss://relay.snort.social`
- **Subscription IDs**: Generate unique IDs per subscription (UUID or counter)
- **Event Caching**: Start with in-memory LRU cache; add SQLite persistence in future if needed
- **Error Logging**: Use `os_log` for debugging; make configurable (log level) for production

## Future Enhancements (Post-MVP)

- NIP-05 DNS-based verification
- NIP-09 Event deletion
- NIP-25 Reactions
- NIP-42 Authentication to relays
- NIP-65 Relay list metadata
- SQLite persistent event cache
- Advanced relay selection (latency-based, geographic)
- Event compression for bandwidth optimization
