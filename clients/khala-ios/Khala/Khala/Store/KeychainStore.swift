import Foundation
import Security

/// Minimal Keychain wrapper for the Khala API bearer key.
///
/// The key (an `oa_agent_…` token) is the only secret the app holds. It must
/// live in the Keychain — never `UserDefaults` or a plist.
enum KeychainStore {
    /// Service + account identifiers for the single stored key.
    private static let service = "com.openagents.khala"
    private static let account = "khala_api_key"

    static func saveAPIKey(_ key: String) {
        let data = Data(key.utf8)
        // Delete any existing item first so save is idempotent.
        deleteAPIKey()
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
        ]
        SecItemAdd(query as CFDictionary, nil)
    }

    static func loadAPIKey() -> String? {
        // Demo/test hook (env-gated; a no-op in normal use): allow injecting the
        // bearer key via the KHALA_API_KEY launch environment so the API
        // round-trip is testable on a simulator without driving the UI. Falls
        // back to the Keychain when the env var is absent.
        if let envKey = ProcessInfo.processInfo.environment["KHALA_API_KEY"],
           !envKey.isEmpty {
            return envKey
        }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess,
              let data = result as? Data,
              let key = String(data: data, encoding: .utf8),
              !key.isEmpty
        else {
            return nil
        }
        return key
    }

    @discardableResult
    static func deleteAPIKey() -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        let status = SecItemDelete(query as CFDictionary)
        return status == errSecSuccess || status == errSecItemNotFound
    }

    static var hasAPIKey: Bool {
        loadAPIKey() != nil
    }
}
