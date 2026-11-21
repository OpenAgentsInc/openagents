import Foundation
import Security

/// The wallet seed's Keychain item (`com.openagents.app.spark`, account
/// `seed-v1`, this device only, not synchronizable), holding 16 or 32 bytes
/// of BIP-39 entropy.
///
/// It reads, and adds a seed only where none is stored: this type intentionally
/// has no update or delete, so it can never replace or remove an existing seed.
public enum OAWalletKeychain {
    public enum Read: Equatable, Sendable {
        case found(Data)
        case notFound
        /// Stored bytes are not 16 or 32 bytes; left untouched.
        case unreadable
        /// Keychain could not answer right now (e.g. locked device).
        case unavailable(OSStatus)
    }

    public static let service = "com.openagents.app.spark"
    public static let account = "seed-v1"

    /// Stores 16 or 32 bytes of BIP-39 entropy only if no seed exists yet.
    /// Returns false if a seed already exists (`errSecDuplicateItem`) or Keychain refused.
    public static func addSeedIfMissing(_ entropy: Data) -> Bool {
        guard entropy.count == 16 || entropy.count == 32 else { return false }
        let item: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: false,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
            kSecValueData as String: entropy,
        ]
        return SecItemAdd(item as CFDictionary, nil) == errSecSuccess
    }

    public static func readSeed() -> Read {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: false,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess:
            guard let bytes = result as? Data else { return .unavailable(status) }
            return (bytes.count == 16 || bytes.count == 32) ? .found(bytes) : .unreadable
        case errSecItemNotFound:
            return .notFound
        default:
            return .unavailable(status)
        }
    }
}
