import Foundation
import Security

enum KeychainStore {
    static func save(_ value: String, service: String, account: String) throws {
        let data = Data(value.utf8)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecValueData as String: data
        ]

        SecItemDelete(query as CFDictionary)
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.writeFailed(status)
        }
    }

    static func load(service: String, account: String) throws -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]

        var item: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw KeychainError.readFailed(status)
        }

        guard
            let data = item as? Data,
            let value = String(data: data, encoding: .utf8)
        else {
            throw KeychainError.invalidData
        }

        return value
    }

    static func delete(service: String, account: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        SecItemDelete(query as CFDictionary)
    }
}

enum KeychainError: LocalizedError {
    case writeFailed(OSStatus)
    case readFailed(OSStatus)
    case invalidData

    var errorDescription: String? {
        switch self {
        case .writeFailed(let status):
            return "Keychain write failed (\(status))."
        case .readFailed(let status):
            return "Keychain read failed (\(status))."
        case .invalidData:
            return "Keychain item contained invalid data."
        }
    }
}
