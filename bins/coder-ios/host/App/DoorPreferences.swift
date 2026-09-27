// Rust validates this opaque document; native storage only bounds and protects it.
import Foundation
import Security

enum DoorPreferences {
    private static func query(synthetic: Bool) -> [String: Any] {
        var account = synthetic ? "synthetic-doors-v1" : "doors-v1"
        // Synthetic UI tests isolate a durable scope and reuse it after relaunch.
        let arguments = ProcessInfo.processInfo.arguments
        if synthetic, let index = arguments.firstIndex(of: "--door-scope"), index + 1 < arguments.count {
            let scope = arguments[index + 1]
            if scope.range(of: "^[A-Za-z0-9-]{1,64}$", options: .regularExpression) != nil { account += ":" + scope }
        }
        return [kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: "com.openagents.coder.verse.doors",
                kSecAttrAccount as String: account,
                kSecAttrSynchronizable as String: false]
    }

    static func load(synthetic: Bool) throws -> String? {
        var request = query(synthetic: synthetic)
        request[kSecReturnData as String] = true
        let status: OSStatus
        var result: CFTypeRef?
        status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data, data.count <= 2048,
              let document = String(data: data, encoding: .utf8) else {
            throw ReaderError.message("Saved door choices unavailable. Unlock the device and retry.")
        }
        return document
    }

    static func save(_ document: String, synthetic: Bool) throws {
        let data = Data(document.utf8)
        guard data.count <= 2048 else { throw ReaderError.message("Door choice not saved.") }
        let request = query(synthetic: synthetic)
        let attributes: [String: Any] = [kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        var status = SecItemUpdate(request as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(request.merging(attributes) { _, new in new } as CFDictionary, nil)
        }
        guard status == errSecSuccess else { throw ReaderError.message("Door choice not saved.") }
    }
}
