import Foundation
import Security

/// The wallet seed's Keychain item, exactly as the earlier OpenAgents apps
/// wrote it: a generic password, service `com.openagents.app.spark`, account
/// `seed-v1`, not synchronizable, holding 16 or 32 bytes of BIP-39 entropy.
/// No access group is named, so the item is in the app's default group (team
/// plus bundle ID), the same one those apps used.
///
/// Read only: this type has no add, update or delete, so it can never
/// replace or remove a stored seed, and it never makes a new one.
enum WalletKeychain {
    enum Read {
        case found(Data)
        case notFound
        /// A value is stored but isn't 16 or 32 bytes. It is left alone.
        case unreadable
        /// The Keychain couldn't answer now, such as on a locked device.
        case unavailable(OSStatus)
    }

    static func readSeed() -> Read {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.openagents.app.spark",
            kSecAttrAccount as String: "seed-v1",
            kSecAttrSynchronizable as String: false,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess:
            guard let bytes = result as? Data else { return .unavailable(status) }
            return bytes.count == 16 || bytes.count == 32 ? .found(bytes) : .unreadable
        case errSecItemNotFound:
            return .notFound
        default:
            // A locked device answers errSecInteractionNotAllowed, not "not found".
            return .unavailable(status)
        }
    }
}
