import Foundation
import Security

/// The phone's Nostr key: 32 random bytes made the first time they're
/// needed and kept in a Keychain generic password, service
/// `com.openagents.app.nostr`, account `nsec-v1`, not synchronizable,
/// readable after the first unlock on this device only, in the app's default
/// access group like the wallet seed (`WalletKeychain`), so it stays when
/// the app is deleted and installed again. It is a software key: libsecp256k1
/// signs with it in the app's memory (no Secure Enclave), and every copy is
/// wiped after use. Nothing here logs or shows it; only the public key leaves.
enum NostrKey {
    enum Failure: Error, LocalizedError {
        /// The Keychain couldn't answer, such as before the first unlock.
        case unavailable(OSStatus)
        /// A value is stored but isn't a valid key. It is left alone.
        case unreadable
        case signing
        var errorDescription: String? {
            switch self {
            case .unavailable: return "The phone's key isn't available right now. Unlock the phone and try again."
            case .unreadable: return "The key stored on this phone can't be read."
            case .signing: return "The phone couldn't sign the request."
            }
        }
    }

    private static let base: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.openagents.app.nostr",
        kSecAttrAccount as String: "nsec-v1",
        kSecAttrSynchronizable as String: false,
    ]

    /// Runs body with the secret key, making and storing one if there is none.
    /// The bytes are wiped when body returns.
    static func withSecret<T>(_ body: (UnsafePointer<UInt8>) throws -> T) throws -> T {
        var key = try load() ?? create()
        defer { key.resetBytes(in: 0..<key.count) }
        return try key.withUnsafeBytes { try body($0.bindMemory(to: UInt8.self).baseAddress!) }
    }

    /// The x-only public key, as 32 bytes.
    static func publicKey() throws -> Data {
        var pub = Data(count: 32)
        let ok = try withSecret { secret in pub.withUnsafeMutableBytes { oa_nostr_pubkey(secret, $0.bindMemory(to: UInt8.self).baseAddress!) } }
        guard ok == 1 else { throw Failure.unreadable }
        return pub
    }

    /// The BIP-340 signature of a 32-byte message (an event id).
    static func sign(_ message: Data) throws -> Data {
        precondition(message.count == 32)
        var aux = Data(count: 32), sig = Data(count: 64)
        guard aux.withUnsafeMutableBytes({ SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }) == errSecSuccess else { throw Failure.signing }
        let ok = try withSecret { secret in
            message.withUnsafeBytes { m in aux.withUnsafeBytes { a in sig.withUnsafeMutableBytes { s in
                oa_nostr_sign(secret, m.bindMemory(to: UInt8.self).baseAddress!, a.bindMemory(to: UInt8.self).baseAddress!, s.bindMemory(to: UInt8.self).baseAddress!)
            } } }
        }
        guard ok == 1 else { throw Failure.signing }
        return sig
    }

    private static func load() throws -> Data? {
        var query = base
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess:
            guard var bytes = result as? Data else { throw Failure.unavailable(status) }
            let valid = bytes.count == 32 && bytes.withUnsafeBytes { oa_nostr_seckey_valid($0.bindMemory(to: UInt8.self).baseAddress!) } == 1
            if !valid { bytes.resetBytes(in: 0..<bytes.count); throw Failure.unreadable }
            return bytes
        case errSecItemNotFound: return nil
        default: throw Failure.unavailable(status)
        }
    }

    /// A new key, added only if none is stored: an existing item is never replaced.
    private static func create() throws -> Data {
        var key = Data(count: 32)
        repeat {
            guard key.withUnsafeMutableBytes({ SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }) == errSecSuccess else { throw Failure.signing }
        } while key.withUnsafeBytes({ oa_nostr_seckey_valid($0.bindMemory(to: UInt8.self).baseAddress!) }) != 1
        var item = base
        item[kSecValueData as String] = key
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let status = SecItemAdd(item as CFDictionary, nil)
        if status == errSecDuplicateItem {   // made meanwhile: use that one
            key.resetBytes(in: 0..<key.count)
            guard let stored = try load() else { throw Failure.unreadable }
            return stored
        }
        guard status == errSecSuccess else { key.resetBytes(in: 0..<key.count); throw Failure.unavailable(status) }
        return key
    }
}
