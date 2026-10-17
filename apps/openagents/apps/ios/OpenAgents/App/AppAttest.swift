import DeviceCheck
import Foundation
import Security

/// App Attest for a new key's first sign-in (#11994, #11995). The Rust core
/// calls this on a background thread with `SHA-256(challenge)`; it makes or
/// reuses this install's App Attest key (its id kept in the Keychain, this
/// device only) and attests it. It answers nil when App Attest isn't
/// supported, as in the simulator, or the system refuses; the core then
/// sends a proof of work instead.
final class AppAttest: AppAttester, @unchecked Sendable {
    private static let service = "com.openagents.app.appattest"
    private static let account = "key-id"
    private static let wait: DispatchTimeInterval = .seconds(20)

    func attest(clientDataHash: Data) -> AppAttestation? {
        let attest = DCAppAttestService.shared
        guard attest.isSupported else { return nil }
        // The saved key first; a fresh one if that key is gone or refused.
        for fresh in [false, true] {
            let saved = fresh ? nil : Self.savedKeyId()
            guard let keyId = saved ?? Self.newKey(attest) else { return nil }
            if let object = Self.attest(attest, keyId: keyId, hash: clientDataHash) {
                return AppAttestation(keyId: keyId, attestation: object.base64EncodedString())
            }
            if saved == nil { return nil }
        }
        return nil
    }

    private static func newKey(_ attest: DCAppAttestService) -> String? {
        var keyId: String?
        let done = DispatchSemaphore(value: 0)
        attest.generateKey { id, _ in
            keyId = id
            done.signal()
        }
        guard done.wait(timeout: .now() + wait) == .success, let keyId else { return nil }
        save(keyId)
        return keyId
    }

    private static func attest(_ attest: DCAppAttestService, keyId: String, hash: Data) -> Data? {
        var object: Data?
        let done = DispatchSemaphore(value: 0)
        attest.attestKey(keyId, clientDataHash: hash) { data, _ in
            object = data
            done.signal()
        }
        guard done.wait(timeout: .now() + wait) == .success else { return nil }
        return object
    }

    private static var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service,
         kSecAttrAccount as String: account]
    }

    private static func savedKeyId() -> String? {
        var q = query
        q[kSecReturnData as String] = true
        var item: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &item) == errSecSuccess, let data = item as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private static func save(_ keyId: String) {
        SecItemDelete(query as CFDictionary)
        var q = query
        q[kSecValueData as String] = Data(keyId.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        SecItemAdd(q as CFDictionary, nil)
    }
}
