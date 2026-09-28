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

    /// The Wallet tab's Spark seed: BIP39 entropy, 16 bytes (12 words) when
    /// created here and 16 or 32 when restored. It is separate from the
    /// device and world keys, never synced or backed up, and handed only to
    /// Rust.
    static func loadOrCreateSpark() throws -> Data {
        var query = sparkQuery
        var result: CFTypeRef?
        query[kSecReturnData as String] = true
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecSuccess, let seed = result as? Data, seed.count == 16 || seed.count == 32 {
            return seed
        }
        guard status == errSecItemNotFound else {
            throw Failure.message("The wallet key is unavailable. Unlock the device and reopen OpenAgents.")
        }
        var seed = Data(count: 16)
        let random = seed.withUnsafeMutableBytes {
            SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!)
        }
        guard random == errSecSuccess else { throw Failure.message("Could not create a wallet key.") }
        try replaceSpark(seed)
        return seed
    }

    /// Put a restored seed in place of the current one.
    static func replaceSpark(_ seed: Data) throws {
        guard seed.count == 16 || seed.count == 32 else {
            throw Failure.message("That wallet key can't be saved.")
        }
        SecItemDelete(sparkQuery as CFDictionary)
        var item = sparkQuery
        item[kSecValueData as String] = seed
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        guard SecItemAdd(item as CFDictionary, nil) == errSecSuccess else {
            throw Failure.message("Could not save the wallet key in Keychain.")
        }
    }

    /// Delete the Mutinynet test wallet's key, which the Spark wallet
    /// replaced. It held only signet test coins.
    static func deleteMutinynetWallet() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.openagents.app.wallet",
            kSecAttrSynchronizable as String: false,
        ]
        SecItemDelete(query as CFDictionary)
    }

    private static let sparkQuery: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.openagents.app.spark",
        kSecAttrAccount as String: "seed-v1",
        kSecAttrSynchronizable as String: false,
    ]

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
