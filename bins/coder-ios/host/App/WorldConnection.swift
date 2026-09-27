// Native storage holds only the relay choice that Rust accepted. World
// validation and connection state remain in the Rust application.
import Foundation
import Security

enum WorldConnection {
    private static func query(synthetic: Bool) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "com.openagents.coder.verse.connection",
         kSecAttrAccount as String: synthetic ? "synthetic-relay-v1" : "relay-v1",
         kSecAttrSynchronizable as String: false]
    }

    static func load(synthetic: Bool) throws -> String? {
        var request = query(synthetic: synthetic)
        request[kSecReturnData as String] = true
        var result: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data, data.count <= 2048,
              let relay = String(data: data, encoding: .utf8) else {
            throw ReaderError.message("Saved world relay unavailable. Unlock the device and retry.")
        }
        return relay
    }

    static func save(_ relay: String?, synthetic: Bool) throws {
        let request = query(synthetic: synthetic)
        // An empty value records an explicit Leave. No item means that Rust
        // should choose its default on a first launch.
        let data = Data((relay ?? "").utf8)
        guard data.count <= 2048 else { throw ReaderError.message("The relay URL is too long.") }
        let attributes: [String: Any] = [kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        var status = SecItemUpdate(request as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(request.merging(attributes) { _, new in new } as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw ReaderError.message("The world connection choice could not be saved.")
        }
    }

    static func resetSyntheticPreference() throws {
        let status = SecItemDelete(query(synthetic: true) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw ReaderError.message("Could not reset the preview world connection.")
        }
    }
}
