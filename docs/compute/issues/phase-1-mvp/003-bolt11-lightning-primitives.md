# BOLT11 & Lightning Primitives

**Phase:** 1 - MVP
**Component:** OpenAgentsCore (Shared)
**Priority:** P0 (Critical Path - Blocks payments)
**Estimated Effort:** 2-3 weeks

## Summary

Implement BOLT11 invoice parsing, generation, and verification for Lightning Network payments. Support LNURL protocol for seamless payment flows and Lightning Address resolution. This provides the foundation for marketplace payments (NIP-57 zaps) and worker payouts.

## Motivation

The compute marketplace requires Lightning Network integration for:

- **Instant micropayments**: Pay for compute jobs in millisats
- **Zap receipts**: NIP-57 zap events contain BOLT11 invoices
- **Worker payouts**: Providers receive Lightning payments for completed jobs
- **Low fees**: Lightning enables sub-cent payments economically

This module provides payment primitives without running a full Lightning node (invoices only; actual payment handling in Phase 2).

## Acceptance Criteria

### BOLT11 Invoice Parsing
- [ ] Parse BOLT11 invoice string (ln...)
- [ ] Extract fields:
  - Amount (millisatoshis)
  - Payment hash (32 bytes)
  - Destination public key (33 bytes compressed)
  - Description / description hash
  - Expiry (seconds from creation)
  - Timestamp (unix)
  - Route hints (optional)
  - Fallback address (optional)
  - Features bitmap
- [ ] Validate checksum (BECH32)
- [ ] Validate signature (secp256k1)
- [ ] Handle amount-less invoices (amount specified by payer)

### BOLT11 Invoice Generation
- [ ] Create BOLT11 invoice with required fields
- [ ] Sign invoice with node private key
- [ ] Encode to BECH32 with checksum
- [ ] Support optional fields (description, expiry, route hints)
- [ ] Generate payment hash (SHA-256 of preimage)
- [ ] Configurable network (mainnet, testnet, signet, regtest)

### BOLT11 Validation
- [ ] Check invoice expiry (not expired)
- [ ] Verify signature matches destination pubkey
- [ ] Validate amount (if present)
- [ ] Check network match (mainnet vs testnet)

### LNURL Protocol (LUD-06, LUD-16)
- [ ] Fetch LNURL metadata (GET request)
- [ ] Parse LNURLPay response:
  - Min/max sendable amount (millisats)
  - Callback URL
  - Metadata (description, image)
- [ ] Create payment request to callback
- [ ] Parse callback response (BOLT11 invoice + success action)
- [ ] Support success actions: message, URL, AES-encrypted data

### Lightning Address (LUD-16)
- [ ] Resolve Lightning Address (user@domain.com) to LNURL
- [ ] HTTPS GET to `https://domain.com/.well-known/lnurlp/user`
- [ ] Parse response (same as LNURL-pay)
- [ ] Cache resolved addresses (5 min TTL)

### Amount Handling
- [ ] Millisatoshi (msat) type (Int64)
- [ ] Satoshi (sat) conversions (1 sat = 1000 msat)
- [ ] Bitcoin (BTC) conversions (1 BTC = 100,000,000 sat)
- [ ] Formatting for display (e.g., "21,000 sats", "0.00021 BTC")
- [ ] Parsing from strings ("1000sats", "0.001btc")

### Preimage & Payment Hash
- [ ] Generate random preimage (32 bytes)
- [ ] Calculate payment hash (SHA-256 of preimage)
- [ ] Verify preimage matches hash

### Error Handling
- [ ] Define `LightningError` enum:
  - `invalidInvoice`, `invoiceExpired`, `invalidSignature`
  - `amountTooLow`, `amountTooHigh`, `amountMissing`
  - `lnurlFetchFailed`, `lnurlInvalidResponse`
  - `lightningAddressInvalid`, `lightningAddressResolutionFailed`
  - `networkMismatch`
- [ ] Structured error messages with context

## Technical Design

### Package Structure

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Lightning/

BOLT11.swift                 // BOLT11 invoice parsing/generation
LNURL.swift                  // LNURL protocol (LUD-06, LUD-16)
LightningAddress.swift       // Lightning Address resolution
LightningAmount.swift        // Amount types and conversions
LightningError.swift         // Error types
```

### Core Types

```swift
// BOLT11.swift

import Foundation
import CryptoKit

/// BOLT11 Lightning invoice
public struct BOLT11Invoice {
    public let network: Network
    public let amount: Millisatoshi?      // nil for amount-less invoices
    public let timestamp: Date
    public let paymentHash: Data          // 32 bytes
    public let destinationPubkey: Data    // 33 bytes compressed
    public let description: String?
    public let descriptionHash: Data?     // 32 bytes (alternative to description)
    public let expiry: TimeInterval       // seconds, default 3600
    public let minFinalCltvExpiry: Int    // default 18
    public let fallbackAddress: String?   // On-chain fallback
    public let routeHints: [[RouteHint]]  // Routing hints
    public let features: Features
    public let signature: Data            // 64 bytes (secp256k1)

    public enum Network: String {
        case mainnet = "bc"
        case testnet = "tb"
        case signet = "tbs"
        case regtest = "bcrt"
    }

    public struct RouteHint {
        public let nodeId: Data           // 33 bytes
        public let shortChannelId: UInt64
        public let feeBase: Millisatoshi
        public let feeProportional: UInt32
        public let cltvExpiryDelta: UInt16
    }

    public struct Features {
        let bits: Data
        // Common features
        public var supportsVariableLengthOnion: Bool
        public var supportsPaymentSecret: Bool
        public var supportsBasicMPP: Bool
    }

    /// Parse BOLT11 invoice from string
    public static func parse(_ invoice: String) throws -> BOLT11Invoice

    /// Encode invoice to BOLT11 string
    public func encode() -> String

    /// Verify invoice signature
    public func verify() -> Bool

    /// Check if invoice is expired
    public func isExpired() -> Bool

    /// Time remaining until expiry
    public func timeUntilExpiry() -> TimeInterval
}

/// BOLT11 invoice builder
public struct BOLT11InvoiceBuilder {
    private var network: BOLT11Invoice.Network = .mainnet
    private var amount: Millisatoshi?
    private var paymentHash: Data?
    private var description: String?
    private var descriptionHash: Data?
    private var expiry: TimeInterval = 3600
    private var minFinalCltvExpiry: Int = 18
    private var fallbackAddress: String?
    private var routeHints: [[BOLT11Invoice.RouteHint]] = []

    public init()

    public mutating func network(_ network: BOLT11Invoice.Network) -> Self
    public mutating func amount(_ amount: Millisatoshi) -> Self
    public mutating func paymentHash(_ hash: Data) -> Self
    public mutating func description(_ desc: String) -> Self
    public mutating func descriptionHash(_ hash: Data) -> Self
    public mutating func expiry(_ seconds: TimeInterval) -> Self
    public mutating func fallback(_ address: String) -> Self
    public mutating func routeHints(_ hints: [[BOLT11Invoice.RouteHint]]) -> Self

    /// Build and sign invoice
    public func build(privateKey: Data) throws -> BOLT11Invoice
}
```

### LNURL Protocol

```swift
// LNURL.swift

import Foundation

/// LNURL response types
public enum LNURLResponse {
    case pay(LNURLPayResponse)
    case withdraw(LNURLWithdrawResponse)
    case channel(LNURLChannelResponse)
    case auth(LNURLAuthResponse)
}

/// LNURL-pay response (LUD-06)
public struct LNURLPayResponse {
    public let callback: URL              // Payment callback URL
    public let minSendable: Millisatoshi  // Min amount (msat)
    public let maxSendable: Millisatoshi  // Max amount (msat)
    public let metadata: String           // JSON-encoded metadata
    public let tag: String                // "payRequest"

    // Optional fields
    public let commentAllowed: Int?       // Max comment length
    public let successAction: SuccessAction?

    public enum SuccessAction {
        case message(String)
        case url(URL, String?)  // URL + optional description
        case aes(String, Data)  // Encrypted data + IV
    }

    /// Parse from JSON
    public static func parse(_ json: Data) throws -> LNURLPayResponse

    /// Get metadata description
    public func getDescription() -> String?

    /// Get metadata image URL
    public func getImageURL() -> URL?
}

/// LNURL-pay callback response
public struct LNURLPayCallback {
    public let pr: String                 // BOLT11 invoice
    public let successAction: LNURLPayResponse.SuccessAction?
    public let routes: [[BOLT11Invoice.RouteHint]]

    /// Parse from JSON
    public static func parse(_ json: Data) throws -> LNURLPayCallback
}

/// LNURL client
public class LNURLClient {
    private let urlSession: URLSession

    public init(urlSession: URLSession = .shared) {
        self.urlSession = urlSession
    }

    /// Fetch LNURL response
    public func fetch(_ lnurl: String) async throws -> LNURLResponse

    /// Request payment invoice from LNURL-pay
    public func requestPayment(
        callback: URL,
        amount: Millisatoshi,
        comment: String? = nil
    ) async throws -> LNURLPayCallback

    /// Decode LNURL (BECH32) to URL
    public static func decode(_ lnurl: String) throws -> URL

    /// Encode URL to LNURL (BECH32)
    public static func encode(_ url: URL) throws -> String
}
```

### Lightning Address

```swift
// LightningAddress.swift

import Foundation

/// Lightning Address (user@domain.com)
public struct LightningAddress {
    public let username: String
    public let domain: String

    public init(username: String, domain: String) {
        self.username = username
        self.domain = domain
    }

    /// Parse Lightning Address string
    public static func parse(_ address: String) throws -> LightningAddress

    /// Format as string (user@domain.com)
    public var formatted: String {
        "\(username)@\(domain)"
    }

    /// Resolve to LNURL-pay endpoint
    public func resolve() async throws -> LNURLPayResponse

    /// Well-known URL for resolution
    public var wellKnownURL: URL {
        URL(string: "https://\(domain)/.well-known/lnurlp/\(username)")!
    }
}

/// Lightning Address resolver with caching
public class LightningAddressResolver {
    private let urlSession: URLSession
    private let cache: NSCache<NSString, CacheEntry>
    private let cacheTTL: TimeInterval = 300  // 5 minutes

    private class CacheEntry {
        let response: LNURLPayResponse
        let timestamp: Date

        init(response: LNURLPayResponse) {
            self.response = response
            self.timestamp = Date()
        }

        func isValid(ttl: TimeInterval) -> Bool {
            Date().timeIntervalSince(timestamp) < ttl
        }
    }

    public init(urlSession: URLSession = .shared) {
        self.urlSession = urlSession
        self.cache = NSCache()
        self.cache.countLimit = 100
    }

    /// Resolve Lightning Address with caching
    public func resolve(_ address: LightningAddress) async throws -> LNURLPayResponse

    /// Clear cache
    public func clearCache()
}
```

### Amount Types

```swift
// LightningAmount.swift

import Foundation

/// Millisatoshi (msat) - smallest Lightning unit
public struct Millisatoshi: Codable, Equatable, Comparable, CustomStringConvertible {
    public let value: Int64  // Millisatoshis

    public init(_ value: Int64) {
        self.value = value
    }

    /// Convert to satoshis (rounded down)
    public var satoshis: Int64 {
        value / 1000
    }

    /// Convert to bitcoin
    public var bitcoin: Double {
        Double(value) / 100_000_000_000.0
    }

    /// Format for display
    public var description: String {
        if value >= 1_000_000_000 {
            // Show as BTC for large amounts
            return String(format: "%.8f BTC", bitcoin)
        } else if value >= 1_000_000 {
            // Show as sats for medium amounts
            let sats = satoshis
            return "\(sats.formatted()) sats"
        } else {
            // Show as msats for small amounts
            return "\(value) msats"
        }
    }

    /// Parse from string (e.g., "1000sats", "0.001btc", "50000msats")
    public static func parse(_ string: String) throws -> Millisatoshi

    // Comparable
    public static func < (lhs: Millisatoshi, rhs: Millisatoshi) -> Bool {
        lhs.value < rhs.value
    }

    // Arithmetic
    public static func + (lhs: Millisatoshi, rhs: Millisatoshi) -> Millisatoshi {
        Millisatoshi(lhs.value + rhs.value)
    }

    public static func - (lhs: Millisatoshi, rhs: Millisatoshi) -> Millisatoshi {
        Millisatoshi(lhs.value - rhs.value)
    }

    public static func * (lhs: Millisatoshi, rhs: Int64) -> Millisatoshi {
        Millisatoshi(lhs.value * rhs)
    }
}

/// Convenience constructors
extension Millisatoshi {
    public static func sats(_ value: Int64) -> Millisatoshi {
        Millisatoshi(value * 1000)
    }

    public static func btc(_ value: Double) -> Millisatoshi {
        Millisatoshi(Int64(value * 100_000_000_000.0))
    }
}
```

### Error Types

```swift
// LightningError.swift

public enum LightningError: Error, LocalizedError {
    case invalidInvoice(reason: String)
    case invoiceExpired
    case invalidSignature
    case checksumMismatch

    case amountTooLow(min: Millisatoshi, provided: Millisatoshi)
    case amountTooHigh(max: Millisatoshi, provided: Millisatoshi)
    case amountMissing
    case amountInvalid(reason: String)

    case lnurlDecodeFailed
    case lnurlEncodeFailed
    case lnurlFetchFailed(url: URL, underlying: Error?)
    case lnurlInvalidResponse(reason: String)

    case lightningAddressInvalid(address: String)
    case lightningAddressResolutionFailed(address: String, underlying: Error?)

    case networkMismatch(expected: BOLT11Invoice.Network, actual: BOLT11Invoice.Network)

    case preimageInvalid
    case paymentHashMismatch

    public var errorDescription: String? {
        switch self {
        case .invalidInvoice(let reason):
            return "Invalid BOLT11 invoice: \(reason)"
        case .invoiceExpired:
            return "Invoice has expired"
        case .amountTooLow(let min, let provided):
            return "Amount \(provided) is below minimum \(min)"
        // ... other cases
        }
    }
}
```

## Dependencies

### Swift Packages
- **secp256k1.swift**: For invoice signature verification (issue #002)
- **OpenAgentsCore/Crypto**: For Bech32 encoding (issue #002)

### System Frameworks
- **Foundation**: Networking, date/time
- **CryptoKit**: SHA-256 hashing

### OpenAgents Dependencies
- **Issue #002**: Secp256k1 & Cryptography (Bech32, signatures)

## Testing Requirements

### Unit Tests
- [ ] BOLT11 parsing (mainnet/testnet invoices)
- [ ] BOLT11 generation and encoding
- [ ] Signature verification
- [ ] Expiry checking
- [ ] Amount conversions (msat ↔ sat ↔ BTC)
- [ ] LNURL decoding/encoding (BECH32)
- [ ] Lightning Address parsing
- [ ] Preimage/hash validation
- [ ] Error handling for malformed invoices

### Integration Tests
- [ ] Fetch LNURL-pay from real endpoint (e.g., Stacker News)
- [ ] Resolve Lightning Address (e.g., hello@getalby.com)
- [ ] Parse real BOLT11 invoices (mainnet/testnet)
- [ ] Cache behavior (TTL, invalidation)

### Test Vectors
- [ ] BOLT11 test vectors from BOLT #11 spec
- [ ] LNURL test vectors from LUD-06/16 specs
- [ ] Amount parsing edge cases (0, max Int64, decimals)

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 3.1.5(i) (Cryptocurrency)**
- ✅ **Compliant**: Lightning invoice handling is allowed
- ✅ No on-device mining or unapproved trading
- ⚠️  **Note**: Actual payment sending/receiving requires Lightning integration (Phase 2)

**ASRG 5.1.1 (Privacy)**
- ✅ LNURL fetches are HTTPS
- ⚠️  **Disclosure**: Lightning Address resolution makes network requests to third-party domains
- ✅ User initiates all payments (no background/automatic payments)

**ASRG 2.5.2 (No Downloaded Code)**
- ✅ **Compliant**: LNURL responses are data (JSON), not executable code
- ✅ Success actions (message, URL, AES) are display-only

### Privacy Best Practices

1. **Network Requests**: All LNURL/Lightning Address requests use HTTPS
2. **Cache**: Lightning Address resolution cached (reduces tracking)
3. **User Consent**: Display payment details before initiating
4. **No Tracking**: Don't send analytics on payment amounts/destinations

## Reference Links

### Specifications
- **BOLT #11 (Invoices)**: https://github.com/lightning/bolts/blob/master/11-payment-encoding.md
- **LUD-06 (LNURL-pay)**: https://github.com/lnurl/luds/blob/luds/06.md
- **LUD-16 (Lightning Address)**: https://github.com/lnurl/luds/blob/luds/16.md
- **NIP-57 (Zaps)**: https://github.com/nostr-protocol/nips/blob/master/57.md

### Apple Documentation
- **URLSession**: https://developer.apple.com/documentation/foundation/urlsession
- **NSCache**: https://developer.apple.com/documentation/foundation/nscache

### Tools
- **BOLT11 Decoder**: https://lightningdecoder.com/
- **Lightning Address Tester**: https://lightningaddress.com/

### OpenAgents
- **Issue #001**: Nostr Client Library (NIP-57 zaps use BOLT11)
- **Issue #002**: Secp256k1 & Cryptography (Bech32, signatures)
- **Issue #013**: macOS Lightning Integration (actual payment sending/receiving)

## Success Metrics

- [ ] Parse 100+ real BOLT11 invoices without errors
- [ ] All test vectors pass (BOLT #11, LUD-06, LUD-16)
- [ ] Resolve 10+ Lightning Addresses successfully
- [ ] Performance: Parse invoice <5ms, resolve address <500ms
- [ ] All unit tests pass (90%+ coverage)
- [ ] API documentation (DocC) complete
- [ ] Published as part of OpenAgentsCore package

## Notes

- **No Payment Sending**: This module only handles invoice parsing/generation, not actual payments
- **Lightning Node Integration**: Deferred to Phase 2 (issue #013)
- **Preimage Storage**: Applications must store preimages securely to prove payment
- **Testnet Support**: Essential for development/testing
- **Amount-less Invoices**: Support for payer-specified amounts (common in zaps)

## Future Enhancements (Post-MVP)

- BOLT #12 (Offers) support
- LNURL-withdraw (LUD-03)
- LNURL-channel (LUD-02)
- LNURL-auth (LUD-04)
- Keysend support (spontaneous payments)
- AMP (Atomic Multi-Path) invoice generation
- Invoice QR code generation (with logo)
- Submarine swaps (Lightning ↔ on-chain)
