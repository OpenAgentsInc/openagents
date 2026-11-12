import CryptoKit
import Foundation
import Security

/// The device key that ties an app's login to this iPhone or Mac (docs/auth/sessions-and-tokens.md
/// "Native: refresh tokens bound to a device key", #13216). One P-256 key per login, made silently
/// the first time it is asked for and never shown:
///
/// - With a Secure Enclave, the key is made inside it and can't be exported. What we keep is the
///   enclave's own wrapped blob (`dataRepresentation`), which only this device's enclave can use.
///   The key asks for no Face ID, Touch ID or passcode: its access control is "this device only,
///   after first unlock" with private-key use, so a background refresh signs without a prompt.
/// - With no Secure Enclave (the iOS Simulator, an Intel Mac without a T2 chip, a virtual
///   machine), the key is an ordinary P-256 key kept in the same store, and the app says so:
///   the server's devices list shows "key in a file on the device", never "Secure Enclave".
///
/// Where the blob is kept is the caller's `DeviceKeyStore`. On iOS that is a Keychain item beside
/// the tokens. On the Mac it is a file only the user can read (`FileDeviceKeyStore`), never the
/// Keychain: a Mac app that an agent rebuilds has a new code signature each build, and a Keychain
/// item would ask for the login password every time. The enclave's blob is useless off this Mac,
/// so a file loses nothing.
///
/// The server's side is coax/web/api/auth-device.c; the proof's bytes are fixed by
/// coax/web/api/auth-device-crypto.coax, and `DeviceKeyTests` holds the same vector.
public protocol DeviceKeyStore: Sendable {
    func load() -> Data?
    func save(_ blob: Data)
    func clear()
}

public struct DeviceKey: Sendable {
    /// Where the key lives: what the app tells the server in `OA-Device-Store`.
    public enum Store: String, Sendable { case enclave, software }

    public let store: Store
    /// SEC 1 uncompressed, 65 bytes (04 || x || y).
    public let publicKey: Data
    private let signer: @Sendable (Data) throws -> Data

    init(store: Store, publicKey: Data, signer: @escaping @Sendable (Data) throws -> Data) {
        self.store = store; self.publicKey = publicKey; self.signer = signer
    }

    /// ES256 over `message` (SHA-256 inside), as r || s, 64 bytes.
    public func sign(_ message: Data) throws -> Data { try signer(message) }

    public var publicKeyHex: String { DeviceKeys.hex(publicKey) }
}

public final class DeviceKeys: @unchecked Sendable {
    public enum Failure: Error { case noKey }

    private let store: DeviceKeyStore
    private let enclave: Bool
    private let lock = NSLock()
    private var cached: DeviceKey?

    /// `enclave: false` forces a software key (tests, and the checks on machines with no enclave).
    public init(store: DeviceKeyStore, enclave: Bool = SecureEnclave.isAvailable) {
        self.store = store; self.enclave = enclave
    }

    /// This login's key: the stored one, or a new one made now and stored. No prompt either way.
    public func current() throws -> DeviceKey {
        lock.lock(); defer { lock.unlock() }
        if let k = cached { return k }
        if let blob = store.load(), let k = DeviceKeys.open(blob) { cached = k; return k }
        let (key, blob) = try DeviceKeys.make(enclave: enclave)
        store.save(blob)
        cached = key
        return key
    }

    /// The stored key, or nil: never makes one (a refresh must sign with the login's own key).
    public func existing() -> DeviceKey? {
        lock.lock(); defer { lock.unlock() }
        if let k = cached { return k }
        guard let blob = store.load(), let k = DeviceKeys.open(blob) else { return nil }
        cached = k
        return k
    }

    /// Forgets the key (logout, or the server ended the login): the next login makes a new one.
    public func forget() {
        lock.lock(); defer { lock.unlock() }
        cached = nil
        store.clear()
    }

    // MARK: the proof (auth-device-crypto.coax)

    /// What a proof signs: the tag, the method, the whole URL, the time in milliseconds and the
    /// refresh token's SHA-256 in hex, one a line.
    public static func message(method: String, url: String, ms: Int64, token: String) -> String {
        "OA-Device-Proof-v1\n\(method)\n\(url)\n\(ms)\n\(hex(Data(SHA256.hash(data: Data(token.utf8)))))"
    }

    /// The `OA-Device-Proof` header's value: "v1 <ms> <signature hex>".
    public static func proof(key: DeviceKey, method: String, url: String, ms: Int64, token: String) throws -> String {
        let signature = try key.sign(Data(message(method: method, url: url, ms: ms, token: token).utf8))
        return "v1 \(ms) \(hex(signature))"
    }

    public static func hex(_ data: Data) -> String {
        let digits = Array("0123456789abcdef".utf8)
        var out = [UInt8](); out.reserveCapacity(data.count * 2)
        for b in data { out.append(digits[Int(b >> 4)]); out.append(digits[Int(b & 15)]) }
        return String(decoding: out, as: UTF8.self)
    }

    // MARK: making and opening

    /// The stored blob: one byte for the kind ("E" enclave, "S" software), then the key's bytes.
    private static func make(enclave: Bool) throws -> (DeviceKey, Data) {
        if enclave, let made = try? makeEnclave() { return made }
        let key = P256.Signing.PrivateKey()
        return (software(key), Data([0x53]) + key.rawRepresentation)
    }

    private static func makeEnclave() throws -> (DeviceKey, Data) {
        var error: Unmanaged<CFError>?
        // No .userPresence or .biometry flag: the key signs silently. This device only.
        guard let access = SecAccessControlCreateWithFlags(nil, kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly, [.privateKeyUsage], &error) else { throw Failure.noKey }
        let key = try SecureEnclave.P256.Signing.PrivateKey(compactRepresentable: false, accessControl: access)
        return (hardware(key), Data([0x45]) + key.dataRepresentation)
    }

    private static func open(_ blob: Data) -> DeviceKey? {
        guard let kind = blob.first else { return nil }
        let body = blob.dropFirst()
        if kind == 0x45, let key = try? SecureEnclave.P256.Signing.PrivateKey(dataRepresentation: Data(body)) { return hardware(key) }
        if kind == 0x53, let key = try? P256.Signing.PrivateKey(rawRepresentation: Data(body)) { return software(key) }
        return nil
    }

    private static func hardware(_ key: SecureEnclave.P256.Signing.PrivateKey) -> DeviceKey {
        DeviceKey(store: .enclave, publicKey: key.publicKey.x963Representation) { message in
            try key.signature(for: message).rawRepresentation
        }
    }

    private static func software(_ key: P256.Signing.PrivateKey) -> DeviceKey {
        DeviceKey(store: .software, publicKey: key.publicKey.x963Representation) { message in
            try key.signature(for: message).rawRepresentation
        }
    }
}

#if os(macOS)
/// A file only the user can read (0600, its folder 0700), for the Mac: `~/.openagents/<app>/device-key`.
/// Never the Keychain (see `DeviceKeys`).
public struct FileDeviceKeyStore: DeviceKeyStore {
    public let url: URL

    public init(url: URL) { self.url = url }

    /// `~/.openagents/<app>/device-key`.
    public init(app: String) {
        self.url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent(app, isDirectory: true)
            .appendingPathComponent("device-key")
    }

    public func load() -> Data? { try? Data(contentsOf: url) }

    public func save(_ blob: Data) {
        let fm = FileManager.default
        try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try? fm.removeItem(at: url)
        fm.createFile(atPath: url.path, contents: blob, attributes: [.posixPermissions: 0o600])
    }

    public func clear() { try? FileManager.default.removeItem(at: url) }
}
#endif
