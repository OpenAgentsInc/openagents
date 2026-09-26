// Platform-scoped identity storage. No secret is printed or put in UserDefaults.
import Foundation
import Security

enum DeviceIdentity {
    static func loadOrCreate(synthetic: Bool) throws -> Data {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "com.openagents.coder.reader",
            kSecAttrAccount as String: synthetic ? "synthetic-device-v1" : "device-v1",
            kSecAttrSynchronizable as String: false,
        ]
        var result: CFTypeRef?
        query[kSecReturnData as String] = true
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecSuccess, let secret = result as? Data, secret.count == 32 {
            return secret
        }
        guard status == errSecItemNotFound else {
            throw ReaderError.message("Device identity is unavailable. Unlock the device and reopen Coder.")
        }
        var secret = Data(count: 32)
        let randomStatus = secret.withUnsafeMutableBytes {
            SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!)
        }
        guard randomStatus == errSecSuccess else {
            throw ReaderError.message("Could not create a device identity.")
        }
        query.removeValue(forKey: kSecReturnData as String)
        query[kSecValueData as String] = secret
        query[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        guard SecItemAdd(query as CFDictionary, nil) == errSecSuccess else {
            throw ReaderError.message("Could not save the device identity in Keychain.")
        }
        return secret
    }

    static func cacheDirectory(synthetic: Bool) throws -> URL {
        let support = try FileManager.default.url(for: .applicationSupportDirectory,
                                                   in: .userDomainMask, appropriateFor: nil, create: true)
        var directory = support.appendingPathComponent(synthetic ? "synthetic-reader-v1" : "reader-v1",
                                                       isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true,
                                                 attributes: [.protectionKey: FileProtectionType.complete])
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.complete],
                                               ofItemAtPath: directory.path)
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try directory.setResourceValues(values)
        return directory
    }
}

enum ReaderError: LocalizedError {
    case message(String)
    var errorDescription: String? {
        switch self { case let .message(message): return message }
    }
}
