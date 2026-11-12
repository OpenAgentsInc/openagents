// Firm reason: 6 (a Swift-only SDK: AppKit's sheets, CryptoKit and the Secure Enclave through AppAuth.swift and DeviceKey.swift)
import AppKit
import CryptoKit
import Foundation

/// The Mac app's account login (#13229, docs/auth/identity-and-methods.md "Per-platform flows"):
/// OpenAgents ▸ Log In… asks for an email and the code we send, or logs in with this Mac's key.
/// The login is the iPhone app's own, `AppAuth` (apps/shared/host/apple/AppAuth.swift): the same
/// requests, an access and refresh token pair whose refreshes are bound to a Secure Enclave key, and
/// "Log in again." when the server ends it. On the Mac the tokens and the device key's blob are
/// files under `~/.openagents/desktop/` (0600), never the Keychain, so a rebuilt app never asks for
/// the login password.
///
/// This Mac's key is the one the openagents command keeps (`~/.openagents/cli/keys/PROFILE`, mode
/// 0600; PROFILE is OPENAGENTS_PROFILE or "default"), so the app and the CLI are one identity. When
/// there is none, Log In offers to make it. The file's format and the NIP-98 event are lab-sign.coax's.
///
/// OPENAGENTS_API_ORIGIN points it at staging (https://staging.openagents.com) or a local API on
/// 127.0.0.1; anything else is openagents.com.
enum MacKey {
    struct Missing: Error {}

    static var profile: String {
        let env = ProcessInfo.processInfo.environment["OPENAGENTS_PROFILE"] ?? ""
        return !env.isEmpty && withText(env, { coaxial_account_profile_ok($0) }) ? env : "default"
    }
    static func file(home: URL = MacAccount.home) -> URL {
        home.appendingPathComponent(".openagents/cli/keys", isDirectory: true).appendingPathComponent(profile)
    }

    /// The key in the file, or nil when there is none (or it isn't a key).
    static func load(_ url: URL) -> [UInt8]? {
        guard let data = try? Data(contentsOf: url), data.count <= 128 else { return nil }
        var text = [UInt8](data)
        defer { wipe(&text) }
        var key = [UInt8](repeating: 0, count: 32)
        let ok = text.withUnsafeBufferPointer { t in
            key.withUnsafeMutableBufferPointer { k in
                coaxial_account_key_parse(coax_string(data: UnsafeRawPointer(t.baseAddress!).assumingMemoryBound(to: CChar.self), count: Int64(t.count)),
                                          coax_v_u8(data: k.baseAddress, count: 32))
            }
        }
        if !ok { wipe(&key); return nil }
        return key
    }

    /// A new key in the file (0600, its folder 0700); never over one that is there.
    static func make(_ url: URL) throws -> [UInt8] {
        var key = [UInt8](repeating: 0, count: 32)
        for _ in 0..<4 {
            guard SecRandomCopyBytes(kSecRandomDefault, 32, &key) == errSecSuccess else { throw AppAuth.Failure.said("This Mac couldn't make a key.") }
            if key.withUnsafeMutableBufferPointer({ coaxial_account_key_ok(coax_v_u8(data: $0.baseAddress, count: 32)) }) { break }
        }
        var text = [UInt8](repeating: 0, count: 81)
        defer { wipe(&text) }
        let n = key.withUnsafeMutableBufferPointer { k in text.withUnsafeMutableBufferPointer { t in
            coaxial_account_key_text(coax_v_u8(data: k.baseAddress, count: 32), coax_v_u8(data: t.baseAddress, count: 81)) } }
        guard n == 81 else { wipe(&key); throw AppAuth.Failure.said("This Mac couldn't make a key.") }
        let dir = url.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let fd = open(url.path, O_WRONLY | O_CREAT | O_EXCL, 0o600)
        guard fd >= 0 else { wipe(&key); throw AppAuth.Failure.said("This Mac couldn't keep its key in \(url.path).") }
        let wrote = text.withUnsafeBytes { write(fd, $0.baseAddress, 81) }
        close(fd)
        guard wrote == 81 else { unlink(url.path); wipe(&key); throw AppAuth.Failure.said("This Mac couldn't keep its key in \(url.path).") }
        return key
    }

    /// The Authorization value for a POST of body to url, signed with the key in the file.
    static func signer(_ url: URL) -> AppAuth.Sign {
        { target, body, now in
            guard var key = load(url) else { throw Missing() }
            defer { wipe(&key) }
            return try nip98(key: key, url: target, body: body, now: now)
        }
    }

    static func nip98(key: [UInt8], url: String, body: String, now: Date) throws -> String {
        let payload = DeviceKeys.hex(Data(SHA256.hash(data: Data(body.utf8))))
        var k = key, aux = [UInt8](repeating: 0, count: 32), out = [UInt8](repeating: 0, count: 2048)
        defer { wipe(&k); wipe(&aux) }
        guard SecRandomCopyBytes(kSecRandomDefault, 32, &aux) == errSecSuccess else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        let n = withText(url) { u in withText(payload) { p in
            k.withUnsafeMutableBufferPointer { kb in aux.withUnsafeMutableBufferPointer { ab in out.withUnsafeMutableBufferPointer { ob in
                coaxial_account_nip98(coax_v_u8(data: kb.baseAddress, count: 32), coax_v_u8(data: ab.baseAddress, count: 32), Int64(now.timeIntervalSince1970),
                                      u, p, coax_v_u8(data: ob.baseAddress, count: Int64(ob.count))) } } } } }
        guard n > 0 else { throw AppAuth.Failure.said("That didn't work. Try again.") }
        return "Nostr " + String(decoding: out[0..<Int(n)], as: UTF8.self)
    }

    static func wipe(_ b: inout [UInt8]) { for i in b.indices { b[i] = 0 } }
    static func withText<T>(_ s: String, _ body: (coax_string) -> T) -> T {
        var bytes = Array(s.utf8)
        return bytes.withUnsafeMutableBufferPointer { b in
            body(coax_string(data: b.baseAddress.map { UnsafeRawPointer($0).assumingMemoryBound(to: CChar.self) }, count: Int64(b.count)))
        }
    }
}

/// The app's one login, what its menu says, and the sheets.
final class MacAccount: @unchecked Sendable {
    static let shared = MacAccount()

    /// $HOME, as the CLI and settings.json read it (a check runs the app in a home of its own).
    static var home: URL {
        let h = ProcessInfo.processInfo.environment["HOME"] ?? ""
        return h.hasPrefix("/") ? URL(fileURLWithPath: h, isDirectory: true) : FileManager.default.homeDirectoryForCurrentUser
    }
    static var folder: URL { home.appendingPathComponent(".openagents/desktop", isDirectory: true) }

    static var origin: String {
        let o = ProcessInfo.processInfo.environment["OPENAGENTS_API_ORIGIN"] ?? ""
        if o == "https://staging.openagents.com" || o.hasPrefix("http://127.0.0.1:") || o.hasPrefix("http://localhost:") { return o }
        return "https://openagents.com"
    }

    let keyFile: URL
    let store: TokenStore
    let auth: AppAuth
    private let lock = NSLock()
    private var shown = ""          // what the account answered: the masked email, or "" for a key only
    private var known = false
    var changed: (() -> Void)?

    init(keyFile: URL = MacKey.file(), store: TokenStore = FileTokenStore(url: MacAccount.folder.appendingPathComponent("login.json")),
         device: DeviceKeys? = DeviceKeys(store: FileDeviceKeyStore(url: MacAccount.folder.appendingPathComponent("device-key"))),
         origin: String = MacAccount.origin, send: @escaping AppAuth.Send = AppAuth.network) {
        self.keyFile = keyFile; self.store = store
        auth = AppAuth(origin: origin, send: send, store: store, device: device, sign: MacKey.signer(keyFile))
    }

    var loggedIn: Bool { auth.loggedIn }
    var title: String {
        guard loggedIn else { return "Log In…" }
        lock.lock(); defer { lock.unlock() }
        if !known { return "Logged in" }
        return shown.isEmpty ? "Logged in with this Mac's key" : "Logged in as \(shown)"
    }

    /// Asks the server who is logged in (an authorized request: a stale access token is renewed
    /// first); a login the server ended is forgotten.
    func look() async {
        guard loggedIn else { tell(); return }
        if let a = try? await auth.account() {
            setShown(a.emails.first ?? "")
        }
        tell()
    }
    private func setShown(_ s: String) { lock.lock(); shown = s; known = true; lock.unlock() }
    func forgetShown() { lock.lock(); shown = ""; known = false; lock.unlock() }
    func tell() { DispatchQueue.main.async { self.changed?() } }

    func keyLogin() async throws {
        try await auth.keyLogin()
        forgetShown()
        await look()
    }
    func logout() async {
        await auth.logout()
        forgetShown()
        tell()
    }

    // MARK: the sheets

    @MainActor func showLogin() async {
        let alert = NSAlert()
        alert.messageText = "Log in to OpenAgents"
        alert.informativeText = "Enter your email and we'll send you a code."
        let email = NSTextField(frame: NSRect(x: 0, y: 0, width: 280, height: 24))
        email.placeholderString = "you@example.com"
        email.setAccessibilityIdentifier("account.email")
        alert.accessoryView = email
        alert.addButton(withTitle: "Send Code")
        alert.addButton(withTitle: "Log In with This Mac's Key")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = email
        switch alert.runModal() {
        case .alertFirstButtonReturn:
            let address = email.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !address.isEmpty else { return await showLogin() }
            do {
                let pending = try await auth.emailStart(address)
                await askCode(address: address, pending: pending)
            } catch { say(error) }
        case .alertSecondButtonReturn:
            await loginWithKey()
        default: return
        }
    }

    @MainActor private func askCode(address: String, pending: String) async {
        let alert = NSAlert()
        alert.messageText = "Enter your code"
        alert.informativeText = "We sent a 6-digit code to \(address)."
        let code = NSTextField(frame: NSRect(x: 0, y: 0, width: 160, height: 24))
        code.placeholderString = "123456"
        code.setAccessibilityIdentifier("account.code")
        alert.accessoryView = code
        alert.addButton(withTitle: "Log In")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = code
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        do {
            try await auth.emailVerify(code: code.stringValue.trimmingCharacters(in: .whitespacesAndNewlines), pending: pending)
            forgetShown()
            await look()
        } catch {
            say(error)
            // A wrong code leaves the code good for its other tries; a dead one says so and stops.
            if case AppAuth.Failure.said(let words)? = error as? AppAuth.Failure, words.contains("tries left") { await askCode(address: address, pending: pending) }
        }
    }

    @MainActor private func loginWithKey() async {
        do { try await keyLogin(); return }
        catch is MacKey.Missing {}
        catch { say(error); return }
        let alert = NSAlert()
        alert.messageText = "This Mac has no key yet"
        alert.informativeText = "Make one to log in with it. It's kept in \(keyFile.path), which only you can read, and the openagents command uses the same key. Until you add an email or a passkey, it's the only way in to that account."
        alert.addButton(withTitle: "Make a Key")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        do {
            var made = try MacKey.make(keyFile)
            MacKey.wipe(&made)
            try await keyLogin()
        } catch { say(error) }
    }

    @MainActor private func say(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = (error as? LocalizedError)?.errorDescription ?? "That didn't work. Try again."
        alert.addButton(withTitle: "OK")
        alert.runModal()
        tell()
    }
}

// MARK: main.c's calls (account-mac.h)

@_cdecl("oa_account_start")
func oa_account_start(_ changed: (@convention(c) () -> Void)?) {
    MacAccount.shared.changed = { changed?() }
    Task.detached { await MacAccount.shared.look() }
}

@_cdecl("oa_account_login")
func oa_account_login() {
    Task { @MainActor in await MacAccount.shared.showLogin() }
}

@_cdecl("oa_account_logout")
func oa_account_logout() {
    Task.detached { await MacAccount.shared.logout() }
}

@_cdecl("oa_account_logged_in")
func oa_account_logged_in() -> Int32 { MacAccount.shared.loggedIn ? 1 : 0 }

@_cdecl("oa_account_title")
func oa_account_title(_ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    copyOut(MacAccount.shared.title, out, cap)
}

private func copyOut(_ s: String, _ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    guard let out, cap > 0 else { return 0 }
    let bytes = Array(s.utf8.prefix(Int(cap) - 1))
    for (i, b) in bytes.enumerated() { out[i] = CChar(bitPattern: b) }
    out[bytes.count] = 0
    return Int32(bytes.count)
}

/// coax drive's account operations (main.c's on_drive), for checks against staging without a sheet:
///   account state     logged-in 0|1 and the menu's title
///   account key       log in with this Mac's key (made first when there is none)
///   account refresh   mark the access token stale, then ask who is logged in (a refresh, with its proof)
///   account logout    log out
@_cdecl("oa_account_drive")
func oa_account_drive(_ request: UnsafePointer<CChar>?, _ out: UnsafeMutablePointer<CChar>?, _ cap: Int32) -> Int32 {
    let op = request.map { String(cString: $0) } ?? ""
    let me = MacAccount.shared
    let done = DispatchSemaphore(value: 0)
    let said = SaidBox()
    Task.detached {
        do {
            switch op {
            case "key":
                if MacKey.load(me.keyFile) == nil { var k = try MacKey.make(me.keyFile); MacKey.wipe(&k) }
                try await me.keyLogin()
            case "refresh":
                guard var t = me.store.load() else { throw AppAuth.Failure.said("Log in first.") }
                t.accessUntil = Date(timeIntervalSince1970: 0); me.store.save(t)
                await me.look()
            case "logout": await me.logout()
            case "state": break
            default: said.set("account: state | key | refresh | logout\n")
            }
        } catch { said.set("account: \((error as? LocalizedError)?.errorDescription ?? "\(error)")\n") }
        done.signal()
    }
    // The answer waits for the server, without holding the main thread's own work (the requests run elsewhere).
    while done.wait(timeout: .now() + 0.05) == .timedOut { RunLoop.current.run(until: Date().addingTimeInterval(0.05)) }
    let text = said.get() ?? "account: logged-in \(me.loggedIn ? 1 : 0) \(me.title)\n"
    return copyOut(text, out, cap)
}

private final class SaidBox: @unchecked Sendable {
    private let lock = NSLock(); private var text: String?
    func set(_ s: String) { lock.lock(); text = s; lock.unlock() }
    func get() -> String? { lock.lock(); defer { lock.unlock() }; return text }
}
