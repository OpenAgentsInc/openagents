// Native storage for the host's Gym grant. Rust validates its authority and
// controls when it may cause network activity; Keychain stores only exact bytes.
import Foundation
import Security

enum GymConnection {
    private static func query(synthetic: Bool) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "com.openagents.coder.gym",
         kSecAttrAccount as String: synthetic ? "synthetic-grant-v1" : "grant-v1",
         kSecAttrSynchronizable as String: false]
    }

    static func load(synthetic: Bool) throws -> String? {
        var request = query(synthetic: synthetic)
        request[kSecReturnData as String] = true
        var result: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data, data.count <= 65_536,
              let code = String(data: data, encoding: .utf8) else {
            throw ReaderError.message("The saved Gym connection is unavailable. Unlock the device and try again.")
        }
        return code
    }

    static func save(_ code: String, synthetic: Bool) throws {
        let data = Data(code.utf8)
        guard data.count <= 65_536 else { throw ReaderError.message("The Gym connection exceeds its size limit.") }
        let request = query(synthetic: synthetic)
        let attributes: [String: Any] = [kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        var status = SecItemUpdate(request as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(request.merging(attributes) { _, new in new } as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw ReaderError.message("The Gym connection works for this session but could not be saved in Keychain.")
        }
    }
}
