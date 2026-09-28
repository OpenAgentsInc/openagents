// This device's Nostr key, kept in Keychain on this device only. No secret is
// printed or put in UserDefaults.
import Foundation
import Security

enum DeviceKey {
    enum Failure: LocalizedError {
        case message(String)
        var errorDescription: String? {
            switch self { case let .message(message): return message }
        }
    }

    static func loadOrCreate() throws -> Data {
        try loadOrCreate(service: "com.openagents.app.device",
                         accessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly)
    }

    /// The Verse tab's world identity: a separate key that signs only world
    /// presence, so players in the world never see the device key that holds
    /// host grants.
    static func loadOrCreateVerse() throws -> Data {
        try loadOrCreate(service: "com.openagents.app.verse",
                         accessible: kSecAttrAccessibleWhenUnlockedThisDeviceOnly)
    }

    private static func loadOrCreate(service: String, accessible: CFString) throws -> Data {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: "device-v1",
            kSecAttrSynchronizable as String: false,
        ]
        var result: CFTypeRef?
        query[kSecReturnData as String] = true
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecSuccess, let secret = result as? Data, secret.count == 32 {
            return secret
        }
        guard status == errSecItemNotFound else {
            throw Failure.message("The device key is unavailable. Unlock the device and reopen OpenAgents.")
        }
        var secret = Data(count: 32)
        let random = secret.withUnsafeMutableBytes {
            SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!)
        }
        guard random == errSecSuccess else { throw Failure.message("Could not create a device key.") }
        query.removeValue(forKey: kSecReturnData as String)
        query[kSecValueData as String] = secret
        query[kSecAttrAccessible as String] = accessible
        guard SecItemAdd(query as CFDictionary, nil) == errSecSuccess else {
            throw Failure.message("Could not save the device key in Keychain.")
        }
        return secret
    }

    /// The app's private state directory, excluded from backup.
    static func stateDirectory() throws -> URL {
        let support = try FileManager.default.url(for: .applicationSupportDirectory,
                                                  in: .userDomainMask, appropriateFor: nil, create: true)
        var directory = support.appendingPathComponent("openagents-v1", isDirectory: true)
        try FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true,
            attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication])
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try directory.setResourceValues(values)
        return directory
    }
}
