# Secp256k1 & Cryptography Integration

**Phase:** 1 - MVP
**Component:** OpenAgentsCore (Shared)
**Priority:** P0 (Critical Path - Blocks Nostr, wallet, identity)
**Estimated Effort:** 2-3 weeks

## Summary

Integrate secp256k1 elliptic curve cryptography for Nostr identity (key generation, signing, verification, ECDH) and Bitcoin wallet operations. Implement Secure Enclave integration for iOS/macOS to protect private keys with hardware-backed security.

## Motivation

The compute marketplace requires cryptographic primitives for:

- **Nostr identity**: secp256k1 Schnorr signatures for event signing (NIP-01)
- **Encrypted messages**: ECDH shared secret derivation for NIP-04 encryption
- **Bitcoin wallet**: secp256k1 ECDSA for transaction signing
- **Security**: Secure Enclave storage for private keys (iOS/macOS)

This module provides the cryptographic foundation for all identity, security, and payment operations in OpenAgents.

## Acceptance Criteria

### Secp256k1 Operations
- [ ] Generate secp256k1 private key (32 bytes)
- [ ] Derive public key from private key (compressed 33-byte format)
- [ ] Sign data with private key (Schnorr signatures for Nostr, ECDSA for Bitcoin)
- [ ] Verify signature with public key
- [ ] ECDH shared secret derivation (for NIP-04 encryption)
- [ ] Key validation (private key in valid range, public key on curve)

### Schnorr Signatures (Nostr - NIP-01)
- [ ] Sign 32-byte message hash (SHA-256)
- [ ] Produce 64-byte Schnorr signature
- [ ] Verify Schnorr signature with public key
- [ ] Hex encoding/decoding for signatures

### ECDSA Signatures (Bitcoin)
- [ ] Sign transaction hash with private key
- [ ] DER-encoded signature format
- [ ] Signature verification
- [ ] Low-S normalization (Bitcoin consensus rules)

### ECDH (NIP-04 Encryption)
- [ ] Derive shared secret from private key + public key
- [ ] Output 32-byte shared secret (SHA-256 of ECDH point)
- [ ] Validate public key before ECDH operation

### Secure Enclave Integration (iOS/macOS)
- [ ] Generate key in Secure Enclave (kSecAttrTokenIDSecureEnclave)
- [ ] Store existing key in Secure Enclave
- [ ] Sign data using Secure Enclave key (no key export)
- [ ] Retrieve public key from Secure Enclave key
- [ ] Biometric authentication requirement (Face ID / Touch ID)
- [ ] Fallback to Keychain for devices without Secure Enclave
- [ ] Key deletion from Secure Enclave

### Keychain Storage (Fallback)
- [ ] Store private key in Keychain with accessibility settings
- [ ] Retrieve private key from Keychain
- [ ] Delete private key from Keychain
- [ ] Support for multiple keys (tagged by identifier)
- [ ] Keychain synchronization control (disable sync for sensitive keys)

### BECH32 Support (NIP-19)
- [ ] Encode binary data to BECH32 (with checksum)
- [ ] Decode BECH32 to binary (validate checksum)
- [ ] Support prefixes: `npub`, `nsec`, `note`

### HD Wallet (BIP32/39/84) - Future Bitcoin Wallet
- [ ] Mnemonic generation (BIP39 - 12/24 words)
- [ ] Mnemonic → seed derivation (PBKDF2)
- [ ] HD key derivation (BIP32 - master key → child keys)
- [ ] BIP84 derivation path (m/84'/0'/0'/0/index for native SegWit)
- [ ] Extended public key (xpub) derivation

### Error Handling
- [ ] Define `CryptoError` enum:
  - `invalidPrivateKey`, `invalidPublicKey`
  - `invalidSignature`
  - `ecdhFailed`
  - `secureEnclaveNotAvailable`, `secureEnclaveOperationFailed`
  - `keychainStoreFailed`, `keychainRetrieveFailed`
  - `bech32EncodingFailed`, `bech32DecodingFailed`
- [ ] Structured error messages with context

## Technical Design

### Package Structure

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Crypto/

Secp256k1.swift              // Core secp256k1 operations
SecureKeys.swift             // Secure Enclave + Keychain integration
Schnorr.swift                // Schnorr signature operations (Nostr)
ECDSA.swift                  // ECDSA operations (Bitcoin)
ECDH.swift                   // ECDH shared secret derivation
Bech32.swift                 // BECH32 encoding/decoding
HDWallet.swift               // BIP32/39/84 HD wallet
CryptoError.swift            // Error types
```

### Core Types

```swift
// Secp256k1.swift

import secp256k1

/// Secp256k1 cryptography wrapper
public struct Secp256k1 {
    /// Generate random private key (32 bytes)
    public static func generatePrivateKey() throws -> Data

    /// Derive public key from private key (33-byte compressed)
    public static func derivePublicKey(from privateKey: Data) throws -> Data

    /// Validate private key (in valid range)
    public static func isValidPrivateKey(_ privateKey: Data) -> Bool

    /// Validate public key (on curve)
    public static func isValidPublicKey(_ publicKey: Data) -> Bool

    /// Convert private key to hex string
    public static func privateKeyToHex(_ privateKey: Data) -> String

    /// Convert hex string to private key
    public static func privateKeyFromHex(_ hex: String) throws -> Data

    /// Convert public key to hex string (compressed)
    public static func publicKeyToHex(_ publicKey: Data) -> String

    /// Convert hex string to public key
    public static func publicKeyFromHex(_ hex: String) throws -> Data
}
```

### Schnorr Signatures (Nostr)

```swift
// Schnorr.swift

public struct Schnorr {
    /// Sign 32-byte message hash with private key (Schnorr)
    /// Returns 64-byte signature
    public static func sign(
        messageHash: Data,      // 32 bytes (SHA-256)
        privateKey: Data        // 32 bytes
    ) throws -> Data            // 64 bytes

    /// Verify Schnorr signature
    public static func verify(
        signature: Data,        // 64 bytes
        messageHash: Data,      // 32 bytes
        publicKey: Data         // 33 bytes (compressed)
    ) -> Bool

    /// Sign message hash and return hex-encoded signature
    public static func signHex(
        messageHash: Data,
        privateKey: Data
    ) throws -> String          // 128-char hex

    /// Verify hex-encoded signature
    public static func verifyHex(
        signatureHex: String,   // 128-char hex
        messageHash: Data,      // 32 bytes
        publicKeyHex: String    // 66-char hex (compressed)
    ) -> Bool
}
```

### ECDSA Signatures (Bitcoin)

```swift
// ECDSA.swift

public struct ECDSA {
    /// Sign 32-byte message hash with private key (ECDSA)
    /// Returns DER-encoded signature
    public static func sign(
        messageHash: Data,      // 32 bytes (double SHA-256 for Bitcoin)
        privateKey: Data        // 32 bytes
    ) throws -> Data            // DER-encoded signature (70-72 bytes typical)

    /// Verify ECDSA signature (DER-encoded)
    public static func verify(
        signature: Data,        // DER-encoded
        messageHash: Data,      // 32 bytes
        publicKey: Data         // 33 bytes (compressed)
    ) -> Bool

    /// Normalize signature to low-S (Bitcoin consensus rule)
    public static func normalizeLowS(_ signature: Data) throws -> Data

    /// Convert DER signature to compact format (r,s as 64 bytes)
    public static func derToCompact(_ derSignature: Data) throws -> Data

    /// Convert compact signature to DER format
    public static func compactToDer(_ compactSignature: Data) throws -> Data
}
```

### ECDH (NIP-04 Encryption)

```swift
// ECDH.swift

public struct ECDH {
    /// Derive shared secret using ECDH
    /// Returns SHA-256 hash of ECDH point (32 bytes)
    public static func deriveSharedSecret(
        privateKey: Data,       // My private key (32 bytes)
        publicKey: Data         // Their public key (33 bytes compressed)
    ) throws -> Data            // Shared secret (32 bytes)

    /// Convenience: Derive shared secret and return hex
    public static func deriveSharedSecretHex(
        privateKeyHex: String,
        publicKeyHex: String
    ) throws -> String
}
```

### Secure Enclave Integration

```swift
// SecureKeys.swift

import Security
import LocalAuthentication

/// Secure key storage (Secure Enclave on supported devices, Keychain fallback)
public class SecureKeys {
    public enum KeyType {
        case nostr          // Nostr identity key
        case bitcoin        // Bitcoin wallet key
        case encryption     // Encryption key
    }

    public struct KeyIdentifier {
        let type: KeyType
        let label: String   // User-facing label (e.g., "Default Nostr Identity")
    }

    /// Check if Secure Enclave is available
    public static var isSecureEnclaveAvailable: Bool

    /// Generate new key in Secure Enclave (or Keychain fallback)
    /// Returns public key (private key never leaves Secure Enclave)
    public static func generateKey(
        identifier: KeyIdentifier,
        requireBiometric: Bool = true
    ) throws -> Data  // Public key (33 bytes compressed)

    /// Import existing private key into Secure Enclave (or Keychain)
    /// WARNING: Key should be deleted from source after import
    public static func importKey(
        privateKey: Data,
        identifier: KeyIdentifier,
        requireBiometric: Bool = true
    ) throws

    /// Sign data using Secure Enclave key
    /// Triggers biometric authentication if required
    public static func sign(
        data: Data,
        identifier: KeyIdentifier,
        algorithm: SigningAlgorithm,
        context: LAContext? = nil
    ) throws -> Data

    public enum SigningAlgorithm {
        case schnorr        // Nostr
        case ecdsaSha256    // Bitcoin
    }

    /// Get public key for stored key
    public static func getPublicKey(
        identifier: KeyIdentifier
    ) throws -> Data

    /// Delete key from Secure Enclave/Keychain
    public static func deleteKey(
        identifier: KeyIdentifier
    ) throws

    /// List all stored keys
    public static func listKeys() throws -> [KeyIdentifier]

    /// Check if key exists
    public static func keyExists(
        identifier: KeyIdentifier
    ) -> Bool

    // Private implementation
    private static func createKeyAttributes(
        identifier: KeyIdentifier,
        requireBiometric: Bool
    ) -> [String: Any]

    private static func queryAttributes(
        identifier: KeyIdentifier
    ) -> [String: Any]
}
```

### BECH32 Encoding

```swift
// Bech32.swift

public struct Bech32 {
    /// Encode binary data to BECH32 with given HRP (human-readable part)
    public static func encode(
        hrp: String,            // e.g., "npub", "nsec", "note"
        data: Data              // Binary data to encode
    ) throws -> String          // BECH32 string

    /// Decode BECH32 string to binary data
    public static func decode(
        _ bech32: String
    ) throws -> (hrp: String, data: Data)

    /// Validate BECH32 checksum
    public static func isValid(_ bech32: String) -> Bool

    // Internal checksum calculation
    private static func createChecksum(hrp: String, data: [UInt8]) -> [UInt8]
    private static func verifyChecksum(hrp: String, data: [UInt8]) -> Bool
}
```

### HD Wallet (BIP32/39/84)

```swift
// HDWallet.swift

public struct HDWallet {
    /// Generate BIP39 mnemonic (12 or 24 words)
    public static func generateMnemonic(
        wordCount: MnemonicWordCount = .twelve
    ) throws -> String

    public enum MnemonicWordCount: Int {
        case twelve = 12
        case twentyFour = 24
    }

    /// Validate BIP39 mnemonic
    public static func validateMnemonic(_ mnemonic: String) -> Bool

    /// Derive seed from mnemonic (BIP39)
    public static func deriveSeed(
        from mnemonic: String,
        passphrase: String = ""
    ) throws -> Data  // 64 bytes

    /// Derive master key from seed (BIP32)
    public static func deriveMasterKey(from seed: Data) throws -> HDKey

    /// HD key (BIP32)
    public struct HDKey {
        public let privateKey: Data         // 32 bytes
        public let publicKey: Data          // 33 bytes compressed
        public let chainCode: Data          // 32 bytes
        public let depth: UInt8
        public let fingerprint: UInt32
        public let childIndex: UInt32

        /// Derive child key at index (BIP32)
        public func deriveChild(at index: UInt32, hardened: Bool = false) throws -> HDKey

        /// Derive key at BIP44/84 path (e.g., "m/84'/0'/0'/0/0")
        public func derivePath(_ path: String) throws -> HDKey

        /// Extended public key (xpub) serialization
        public func toXpub() -> String

        /// Extended private key (xprv) serialization
        public func toXprv() -> String
    }

    /// Convenience: Derive Bitcoin native SegWit address at index (BIP84)
    public static func deriveBitcoinAddress(
        from mnemonic: String,
        account: UInt32 = 0,
        change: UInt32 = 0,
        index: UInt32 = 0
    ) throws -> String  // bc1... address
}
```

### Error Types

```swift
// CryptoError.swift

public enum CryptoError: Error, LocalizedError {
    case invalidPrivateKey(reason: String)
    case invalidPublicKey(reason: String)
    case invalidSignature
    case signatureFailed(underlying: Error?)
    case verificationFailed

    case ecdhFailed(reason: String)

    case secureEnclaveNotAvailable
    case secureEnclaveOperationFailed(underlying: Error?)
    case biometricAuthenticationFailed

    case keychainStoreFailed(status: OSStatus)
    case keychainRetrieveFailed(status: OSStatus)
    case keychainDeleteFailed(status: OSStatus)
    case keyNotFound(identifier: String)

    case bech32EncodingFailed(reason: String)
    case bech32DecodingFailed(reason: String)
    case bech32ChecksumInvalid

    case hdWalletMnemonicInvalid
    case hdWalletDerivationFailed(path: String)

    public var errorDescription: String? {
        switch self {
        case .invalidPrivateKey(let reason):
            return "Invalid private key: \(reason)"
        case .secureEnclaveNotAvailable:
            return "Secure Enclave not available on this device"
        // ... other cases
        }
    }
}
```

## Dependencies

### Swift Packages
- **secp256k1.swift**: For secp256k1 operations (Schnorr, ECDSA, ECDH)
  - URL: `https://github.com/GigaBitcoin/secp256k1.swift`
  - Version: ~2.0.0
  - Add to `Package.swift` dependencies

### System Frameworks
- **Security**: Secure Enclave, Keychain
- **LocalAuthentication**: Biometric authentication (Face ID, Touch ID)
- **CryptoKit**: SHA-256 hashing
- **Foundation**: Data, String utilities

### OpenAgents Dependencies
- None (foundational)

## Testing Requirements

### Unit Tests
- [ ] Key generation (validate 32-byte private key, 33-byte public key)
- [ ] Public key derivation (deterministic from private key)
- [ ] Schnorr signing and verification (test vectors)
- [ ] ECDSA signing and verification (Bitcoin test vectors)
- [ ] ECDH shared secret derivation (test vectors)
- [ ] BECH32 encoding/decoding (NIP-19 test vectors)
- [ ] HD wallet mnemonic generation (valid BIP39)
- [ ] HD wallet key derivation (BIP32 test vectors)
- [ ] Error handling for invalid keys/signatures

### Integration Tests
- [ ] Secure Enclave key generation (on real device)
- [ ] Secure Enclave signing with biometric auth
- [ ] Keychain storage/retrieval
- [ ] Fallback to Keychain on Simulator (no Secure Enclave)
- [ ] Key deletion from Secure Enclave/Keychain

### Security Tests
- [ ] Private key never logged or exposed
- [ ] Secure Enclave key cannot be extracted
- [ ] Keychain items have correct accessibility attributes
- [ ] Biometric authentication required when configured
- [ ] Memory wiped after use (private keys zeroed)

### Performance Tests
- [ ] Signing throughput (>500 signatures/sec)
- [ ] Verification throughput (>1000 verifications/sec)
- [ ] ECDH derivation (<10ms p95)

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 3.1.5(i) (Cryptocurrency Wallets)**
- ✅ **Compliant**: Cryptocurrency wallets are allowed for **Organization** developers
- ⚠️  **Action Required**: Ensure Apple Developer account is **Organization** type (not Individual)
- ✅ No on-device mining (only key management and signing)

**ASRG 5.1.1 (Privacy - Biometric Data)**
- ✅ LocalAuthentication framework does **not** access biometric data (only auth result)
- ✅ Privacy policy should mention biometric auth for key access (if enabled)

**ASRG 2.5.6 (Security)**
- ✅ Secure Enclave provides hardware-backed security
- ✅ Private keys never leave device
- ✅ Keychain with appropriate accessibility settings

### DPLA Compliance

**No specific DPLA concerns** for cryptography module (general-purpose crypto, not Foundation Models-specific).

### Best Practices

1. **Key Storage**:
   - Private keys in Secure Enclave (when available)
   - Fallback to Keychain with `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`
   - Never store keys in UserDefaults or plain files

2. **Biometric Auth**:
   - Optional but recommended for Nostr signing
   - Required for Bitcoin wallet operations (high-value)
   - Grace period: 30 seconds (configurable)

3. **Memory Safety**:
   - Zero out private key data after use
   - Use `SecureBytes` wrapper for sensitive data
   - Avoid string interpolation with private keys (no logging)

4. **Export/Backup**:
   - Nostr keys: Allow export as BECH32 `nsec` for backup
   - Bitcoin keys: Mnemonic backup (BIP39 - 12/24 words)
   - Warn users about backup security

## Reference Links

### Specifications
- **secp256k1**: https://en.bitcoin.it/wiki/Secp256k1
- **BIP32 (HD Wallets)**: https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki
- **BIP39 (Mnemonic)**: https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki
- **BIP84 (Native SegWit)**: https://github.com/bitcoin/bips/blob/master/bip-0084.mediawiki
- **NIP-19 (BECH32)**: https://github.com/nostr-protocol/nips/blob/master/19.md
- **Schnorr Signatures (BIP340)**: https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki

### Apple Documentation
- **Secure Enclave**: https://developer.apple.com/documentation/security/certificate_key_and_trust_services/keys/storing_keys_in_the_secure_enclave
- **Keychain Services**: https://developer.apple.com/documentation/security/keychain_services
- **LocalAuthentication**: https://developer.apple.com/documentation/localauthentication
- **CryptoKit**: https://developer.apple.com/documentation/cryptokit

### External Libraries
- **secp256k1.swift**: https://github.com/GigaBitcoin/secp256k1.swift
- **LibWally (C library reference)**: https://github.com/ElementsProject/libwally-core

### OpenAgents
- **Issue #001**: Nostr Client Library (depends on this)
- **Issue #010**: Bitcoin/Lightning Wallet (depends on this)

## Success Metrics

- [ ] All unit tests pass (95%+ code coverage)
- [ ] Schnorr/ECDSA signatures match test vectors
- [ ] Secure Enclave operations work on real iOS/macOS hardware
- [ ] Keychain fallback works on Simulator
- [ ] Performance targets met (>500 signs/sec, >1000 verifies/sec)
- [ ] No private key leaks in logs (security audit)
- [ ] API documentation (DocC) complete
- [ ] Published as part of OpenAgentsCore package

## Notes

- **secp256k1.swift vs C library**: Use `secp256k1.swift` for Swift-native API and safety
- **Secure Enclave Limitations**: secp256k1 may not be directly supported; use P-256 for SE keys, secp256k1 for Keychain keys if needed
  - **Alternative**: Store secp256k1 key in Keychain only (still secure on iOS/macOS)
  - Investigate SE support for secp256k1 in testing phase
- **HD Wallet**: Can defer BIP32/39/84 to Phase 2 (Payments) if needed for MVP
- **Memory Safety**: Consider `SecureBytes` wrapper that zeros memory on deinit

## Future Enhancements (Post-MVP)

- Support for other curves (P-256, ed25519)
- Hardware wallet integration (Ledger, Trezor via USB-C)
- Multi-sig support (Schnorr MuSig, Bitcoin multi-sig)
- Key derivation schemes (HKDF)
- Zero-knowledge proofs (future privacy features)
