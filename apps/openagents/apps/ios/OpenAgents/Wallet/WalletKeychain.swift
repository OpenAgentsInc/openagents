import Foundation
import Security

/// The wallet seed's Keychain item, exactly as the earlier OpenAgents app
/// wrote it: a generic password, service `com.openagents.app.spark`, account
/// `seed-v1`, not synchronizable, readable while unlocked on this device
/// only, holding 16 or 32 bytes of BIP-39 entropy. No access group is named,
/// so the item stays in the app's default group (team plus bundle ID), the
/// same one the earlier app used.
///
/// Rust (`wallet::seed`) decides when to read and when a new seed may be
/// added. This type only reads and adds: it has no delete and no update, so
/// a stored seed can't be removed or replaced from here.
final class WalletKeychain: KeyStore, @unchecked Sendable {
    private static let query: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.openagents.app.spark",
        kSecAttrAccount as String: "seed-v1",
        kSecAttrSynchronizable as String: false,
    ]

    func readSeed() -> KeyRead {
        var query = Self.query
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess:
            guard let bytes = result as? Data else { return .failed(status: status) }
            return .found(bytes: bytes)
        case errSecItemNotFound:
            return .notFound
        default:
            // A locked device answers errSecInteractionNotAllowed: not "not found".
            return .failed(status: status)
        }
    }

    func addSeed(entropy: Data) -> KeyAdd {
        var item = Self.query
        item[kSecValueData as String] = entropy
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        switch status {
        case errSecSuccess: return .added
        case errSecDuplicateItem: return .alreadyExists
        default: return .failed(status: status)
        }
    }
}
