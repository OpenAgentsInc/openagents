import Foundation
import Security

enum KeychainStore {
    private static let service = "com.openagents.khala-macos"
    private static let account = "khala_api_key"

    static func saveAPIKey(_ key: String) {
        let data = Data(key.trimmingCharacters(in: .whitespacesAndNewlines).utf8)
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
