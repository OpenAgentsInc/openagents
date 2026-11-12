import Foundation
import Security

/// The app's login to openagents.com (coax/web/api/auth.c, docs/auth/; #13206, #13229), one file
/// for the iPhone app and the Mac app: an email code, a passkey (#13215) or this device's Nostr key
/// logs in, and the server answers an access token (oa_at_, an hour) and a
/// refresh token (oa_rt_). Requests carry `Authorization: Bearer <access>`; a stale access token is
/// renewed first with the refresh token, which the server replaces each time. A refresh token used
/// twice ends the whole login on the server: then the app forgets it and says "Log in again."
///
/// Where the tokens are kept:
/// - iPhone: in the Keychain like the phone's key (NostrKey): a generic password, service
///   `com.openagents.app.auth`, account `tokens-v1`, this device only, after first unlock.
/// - Mac: a file only the user can read (`FileTokenStore`, `~/.openagents/<app>/login.json`, 0600),
///   never the Keychain: an app an agent rebuilds has a new code signature each build, and a
///   Keychain item would ask for the login password every time (the device key is kept the same
///   way, `FileDeviceKeyStore`).
/// Nothing here logs a token or a code.
///
/// The login is tied to this phone (#13216, docs/auth "Native: refresh tokens bound to a device
/// key"): every login request carries the public half of a device key (`DeviceKeys`, in the Secure
/// Enclave of the iPhone or Mac; made silently, no prompt), the server binds the session to it, and each refresh then
/// carries `OA-Device-Proof`, the key's signature over the request, the time and the refresh
/// token's hash. A refresh token copied off the phone is useless without the key. One key lasts
/// one login: logging out, or the server ending the login, forgets it.
final class AppAuth: @unchecked Sendable {
    #if os(iOS)
    static let shared = AppAuth()
    #endif

    /// What the server answered for a login or a refresh: the pair and when the access one ends.
    struct Tokens: Codable, Equatable {
        var access: String
        var refresh: String
        var accessUntil: Date
        var account: String
        /// The server bound this login to the device key: its refreshes need a proof.
        var deviceBound: Bool? = nil
    }

    /// What a request should do with the tokens it has (pure, for the unit tests).
    enum Next: Equatable { case loggedOut, useAccess(String), refresh(String) }
    /// The access token while it has more than a minute left, else a refresh.
    static func next(_ tokens: Tokens?, now: Date) -> Next {
        guard let t = tokens else { return .loggedOut }
        if t.accessUntil.timeIntervalSince(now) > 60 { return .useAccess(t.access) }
        return .refresh(t.refresh)
    }

    enum Failure: Error, LocalizedError, Equatable {
        /// The server's own words (auth.c's {"error"}), shown as given.
        case said(String)
        case unreachable
        var errorDescription: String? {
            switch self {
            case .said(let words): return words
            case .unreachable: return "Couldn't reach openagents.com. Check the connection and try again."
            }
        }
    }

    /// One HTTP exchange: the answer's bytes and status. Tests give a fake.
    typealias Send = @Sendable (URLRequest) async throws -> (Data, Int)
    /// A NIP-98 Authorization value ("Nostr " + base64 event) for a POST of body to url, signed with
    /// this device's Nostr key: the phone's (PhoneSignIn.nip98) or the Mac's (the desktop app's).
    typealias Sign = @Sendable (_ url: String, _ body: String, _ now: Date) throws -> String
    static let network: Send = { request in
        let (data, response) = try await URLSession.shared.data(for: request)
        return (data, (response as? HTTPURLResponse)?.statusCode ?? 0)
    }

    /// A refusal with its status, inside this file; callers see `Failure.said` with the same words.
    /// proof: why a device proof was refused (auth-device.c's "device_proof"); nowMs: the server's
    /// clock, sent when ours is more than a minute off.
    struct Refused: Error { let status: Int; let words: String; var proof: String? = nil; var nowMs: Int64? = nil }

    let origin: String
    private let send: Send
    private let store: TokenStore
    private let clock: @Sendable () -> Date
    private let device: DeviceKeys?
    private let sign: Sign
    private var refreshing: Task<Tokens, Error>?
    private let lock = NSLock()

    /// Debug builds take -OAAPIOrigin http://127.0.0.1:PORT for tests.
    static var defaultOrigin: String {
        #if DEBUG
        if let o = UserDefaults.standard.string(forKey: "OAAPIOrigin"), !o.isEmpty { return o }
        #endif
        return "https://openagents.com"
    }

    #if os(iOS)
    /// The phone's device key: the Secure Enclave's wrapped blob in the Keychain beside the tokens
    /// (a software key where there is no enclave, such as the Simulator).
    static let phoneDevice = DeviceKeys(store: KeychainDeviceKeyStore())
    static let defaultStore: TokenStore = KeychainTokenStore()
    static let defaultDevice: DeviceKeys? = phoneDevice
    static let defaultSign: Sign = { try PhoneSignIn.nip98(url: $0, body: $1, now: $2) }
    #else
    /// The Mac's: files under `~/.openagents/desktop/`, never the Keychain. The desktop app passes
    /// its own key's signer; without one, "Log in with a key" says there is none.
    static let defaultStore: TokenStore = FileTokenStore(app: "desktop")
    static let defaultDevice: DeviceKeys? = DeviceKeys(store: FileDeviceKeyStore(app: "desktop"))
    static let defaultSign: Sign = { _, _, _ in throw Failure.said("This Mac has no key to log in with.") }
    #endif

    init(origin: String = AppAuth.defaultOrigin, send: @escaping Send = AppAuth.network, store: TokenStore = AppAuth.defaultStore, clock: @escaping @Sendable () -> Date = { Date() },
         device: DeviceKeys? = AppAuth.defaultDevice, sign: @escaping Sign = AppAuth.defaultSign) {
        self.origin = origin; self.send = send; self.store = store; self.clock = clock; self.device = device; self.sign = sign
    }

    var tokens: Tokens? { store.load() }
    var loggedIn: Bool { tokens != nil }

    // MARK: logging in

    /// Asks for an email code. The answer's pending value goes with the code.
    func emailStart(_ email: String) async throws -> String {
        let answer = try await said { try await self.post("/api/auth/email/start", ["email": email, "client": "app"]) }
        guard let pending = answer["pending"] as? String else { throw Failure.said("That didn't work. Try again.") }
        return pending
    }

    /// The code from the email, with the pending value emailStart gave: logged in.
    @discardableResult
    func emailVerify(code: String, pending: String) async throws -> Tokens {
        try keep(try await said { try await self.post("/api/auth/email/verify", ["code": code, "pending": pending]) })
    }

    /// Logs in with this device's key: a challenge, signed (NIP-98) over the login route and body.
    @discardableResult
    func keyLogin() async throws -> Tokens {
        let challenge = try await said { try await self.post("/api/auth/nostr/challenge", ["client": "app"]) }
        guard let ch = challenge["challenge"] as? String else { throw Failure.said("That didn't work. Try again.") }
        let body = "{\"challenge\":\"\(ch)\",\"label\":\"app\"}"
        let url = origin + "/api/auth/nostr"
        let auth = try sign(url, body, clock())
        return try keep(try await said { try await self.post("/api/auth/nostr", raw: body, authorization: auth) })
    }

    /// A passkey login's challenge (coax/web/api/passkeys.c, #13215): the bytes the passkey signs,
    /// as sent (base64url), and the site the passkey belongs to.
    struct PasskeyChallenge: Equatable { let challenge: Data; let text: String; let rpID: String }
    func passkeyChallenge() async throws -> PasskeyChallenge {
        let answer = try await said { try await self.post("/api/auth/passkey/login/options", ["client": "app"]) }
        guard let text = answer["challenge"] as? String, let bytes = AppAuth.unb64url(text), bytes.count == 32,
              let rp = answer["rp_id"] as? String, !rp.isEmpty else { throw Failure.said("That didn't work. Try again.") }
        return PasskeyChallenge(challenge: bytes, text: text, rpID: rp)
    }
    /// What a passkey answered (ASAuthorizationPlatformPublicKeyCredentialAssertion's fields).
    struct PasskeyAssertion: Equatable { let credentialID: Data; let authenticatorData: Data; let clientData: Data; let signature: Data; let userHandle: Data? }
    /// The body POST /api/auth/passkey/login reads.
    static func passkeyBody(_ c: PasskeyChallenge, _ a: PasskeyAssertion) -> [String: String] {
        ["challenge": c.text, "id": b64url(a.credentialID), "auth_data": b64url(a.authenticatorData),
         "client_data": b64url(a.clientData), "signature": b64url(a.signature), "user_handle": a.userHandle.map(b64url) ?? ""]
    }
    /// Logs in with the passkey's answer to the challenge: logged in.
    @discardableResult
    func passkeyLogin(_ c: PasskeyChallenge, _ a: PasskeyAssertion) async throws -> Tokens {
        try keep(try await said { try await self.post("/api/auth/passkey/login", AppAuth.passkeyBody(c, a)) })
    }
    static func b64url(_ d: Data) -> String {
        d.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    }
    static func unb64url(_ s: String) -> Data? {
        guard !s.isEmpty, s.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") }) else { return nil }
        var t = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        while t.count % 4 != 0 { t += "=" }
        return Data(base64Encoded: t)
    }

    // MARK: passkeys while logged in (the wallet's backup and daily limit, #13217)

    /// A step-up challenge for one of this account's passkeys: answering it proves it is you for
    /// the next five minutes (L2-fresh), which storing a wallet backup, reading it, raising the
    /// daily limit and paying past it each need. Throws the server's words when the account has
    /// no passkey.
    func passkeyStepUpChallenge() async throws -> PasskeyChallenge {
        let answer = try await said { try await self.authed("/api/auth/passkey/step-up/options", ["client": "app"]) }
        guard let text = answer["challenge"] as? String, let bytes = AppAuth.unb64url(text), bytes.count == 32,
              let rp = answer["rp_id"] as? String, !rp.isEmpty else { throw Failure.said("That didn't work. Try again.") }
        return PasskeyChallenge(challenge: bytes, text: text, rpID: rp)
    }
    func passkeyStepUp(_ c: PasskeyChallenge, _ a: PasskeyAssertion) async throws {
        _ = try await said { try await self.authed("/api/auth/passkey/step-up", AppAuth.passkeyBody(c, a)) }
    }
    /// What a new passkey is made from: the challenge, the site, and the account's user handle
    /// and name as the password manager will show them.
    struct PasskeyNew: Equatable { let challenge: Data; let text: String; let rpID: String; let userID: Data; let userIDText: String; let userName: String }
    func passkeyRegisterChallenge() async throws -> PasskeyNew {
        let answer = try await said { try await self.authed("/api/auth/passkey/register/options", ["client": "app"]) }
        guard let text = answer["challenge"] as? String, let bytes = AppAuth.unb64url(text), bytes.count == 32,
              let rp = answer["rp_id"] as? String, !rp.isEmpty,
              let uid = answer["user_id"] as? String, let user = AppAuth.unb64url(uid), !user.isEmpty else { throw Failure.said("That didn't work. Try again.") }
        return PasskeyNew(challenge: bytes, text: text, rpID: rp, userID: user, userIDText: uid, userName: answer["user_name"] as? String ?? "OpenAgents account")
    }
    /// Adds the passkey the phone just made. prf: the passkey said it can do PRF (what a wallet backup needs).
    func passkeyRegister(_ c: PasskeyNew, attestation: Data, clientData: Data, prf: Bool) async throws {
        _ = try await said {
            try await self.authed("/api/auth/passkey/register", ["challenge": c.text, "attestation": AppAuth.b64url(attestation), "client_data": AppAuth.b64url(clientData),
                                                                   "user_id": c.userIDText, "transports": "internal,hybrid", "prf": prf ? "1" : "0"])
        }
    }
    /// How many passkeys the account has.
    func passkeyCount() async throws -> Int {
        guard let url = URL(string: origin + "/api/auth/passkeys") else { throw Failure.unreachable }
        let r = try await authorize(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20))
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw Failure.unreachable }
        guard (200..<300).contains(status), let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Failure.said(AppAuth.errorWords(data) ?? "That didn't work. Try again.")
        }
        return (o["passkeys"] as? [Any])?.count ?? 0
    }
    /// Who is logged in, as GET /api/auth/account says (with this login's access token): the
    /// account, and its emails as the server masks them (a•••@example.com).
    struct Account: Equatable { let account: String; let emails: [String]; let keys: Int }
    func account() async throws -> Account {
        guard let url = URL(string: origin + "/api/auth/account") else { throw Failure.unreachable }
        let r = try await authorize(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20))
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw Failure.unreachable }
        guard (200..<300).contains(status), let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Failure.said(AppAuth.errorWords(data) ?? (status == 401 ? "Log in again." : "That didn't work. Try again."))
        }
        guard o["logged_in"] as? Bool == true, let account = o["account"] as? String else { throw Failure.said("Log in again.") }
        return Account(account: account, emails: o["emails"] as? [String] ?? [], keys: (o["keys"] as? [Any])?.count ?? 0)
    }
    /// A challenge for a key to sign while logged in (purpose: link), and the URL it signs.
    func proofChallenge(purpose: String) async throws -> String {
        let answer = try await said { try await self.authed("/api/auth/proof/challenge", ["purpose": purpose]) }
        guard let url = answer["url"] as? String, !url.isEmpty else { throw Failure.said("That didn't work. Try again.") }
        return url
    }
    /// Links the Nostr key that signed proof (a NIP-98 event over url) to this account.
    func linkNostr(url: String, proof: String, label: String) async throws {
        guard let u = URL(string: url), u.absoluteString.hasPrefix(origin + "/api/auth/link/nostr?c=") else { throw Failure.said("That didn't work. Try again.") }
        let path = String(u.absoluteString.dropFirst(origin.count))
        _ = try await said { try await self.authed(path, ["proof": proof, "label": label]) }
    }
    /// Removes a Nostr key from the account's ways in (the server refuses the last one): its id
    /// is looked up by its public key. Needs a fresh proof that it is you.
    func unlinkNostr(pubkey: String) async throws {
        guard let url = URL(string: origin + "/api/auth/ways") else { throw Failure.unreachable }
        let r = try await authorize(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20))
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw Failure.unreachable }
        guard (200..<300).contains(status), let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Failure.said(AppAuth.errorWords(data) ?? "That didn't work. Try again.")
        }
        let keys = o["keys"] as? [[String: Any]] ?? []
        guard let id = keys.first(where: { ($0["pubkey"] as? String) == pubkey })?["id"] as? NSNumber else { return }   // already gone
        let access = try await authorize(URLRequest(url: url)).value(forHTTPHeaderField: "Authorization") ?? ""
        _ = try await said { try await self.post("/api/auth/unlink", raw: "{\"id\":\(id.int64Value),\"kind\":\"nostr\"}", authorization: access) }
    }
    /// A POST with this login's access token (renewed first when it is stale).
    private func authed(_ path: String, _ json: [String: String]) async throws -> [String: Any] {
        let access: String
        switch AppAuth.next(tokens, now: clock()) {
        case .loggedOut: throw Failure.said("Log in first.")
        case .useAccess(let a): access = a
        case .refresh: access = try await refreshed().access
        }
        return try await post(path, json, authorization: "Bearer " + access)
    }

    /// Ends this login on the server (best effort) and forgets the tokens.
    func logout() async {
        if let t = tokens {
            _ = try? await post("/api/auth/logout", [:], authorization: "Bearer " + t.access)
        }
        store.clear()
        device?.forget()   // one key lasts one login
    }

    // MARK: using it

    /// The request with `Authorization: Bearer`, renewing the access token first when it is stale.
    /// Throws `.said("Log in again.")` (and forgets the login) when the server ended it.
    func authorize(_ request: URLRequest) async throws -> URLRequest {
        var r = request
        let access: String
        switch AppAuth.next(tokens, now: clock()) {
        case .loggedOut: throw Failure.said("Log in first.")
        case .useAccess(let a): access = a
        case .refresh: access = try await refreshed().access
        }
        r.setValue("Bearer " + access, forHTTPHeaderField: "Authorization")
        return r
    }

    /// A new pair from the refresh token, one refresh at a time however many requests ask.
    func refreshed() async throws -> Tokens {
        lock.lock()
        if let running = refreshing { lock.unlock(); return try await running.value }
        // Another request renewed it a moment ago: use that pair.
        if let t = store.load(), case .useAccess = AppAuth.next(t, now: clock()) { lock.unlock(); return t }
        let task = Task<Tokens, Error> { [self] in
            guard let t = store.load() else { throw Failure.said("Log in first.") }
            do {
                return try keep(try await renew(t), account: t.account, bound: t.deviceBound)
            } catch let r as Refused where r.status == 401 {
                // The login ended (a reused token, idle, logged out elsewhere, or its device was
                // removed): forget it, and its device key with it.
                store.clear()
                device?.forget()
                throw Failure.said(r.words)
            } catch let r as Refused {
                throw Failure.said(r.words)   // a limit and the like: the login stays
            }
        }
        refreshing = task
        lock.unlock()
        defer { lock.lock(); refreshing = nil; lock.unlock() }
        return try await task.value
    }

    // MARK: the device key

    static let refreshPath = "/api/auth/refresh"
    static func ms(_ date: Date) -> Int64 { Int64((date.timeIntervalSince1970 * 1000).rounded()) }
    #if os(iOS)
    static let platform = "ios"
    static var deviceName: String {
        var info = utsname(); uname(&info)
        let machine = withUnsafeBytes(of: &info.machine) { String(decoding: $0.prefix(while: { $0 != 0 }), as: UTF8.self) }
        return machine.hasPrefix("iPad") ? "iPad" : machine.hasPrefix("iPhone") ? "iPhone" : "iPhone Simulator"
    }
    #else
    static let platform = "mac"
    /// "Mac", or "Mac virtual machine" (hw.model VirtualMac…): never the computer's own name, which
    /// is the person's and may hold their name.
    static var deviceName: String {
        var size = 0
        guard sysctlbyname("hw.model", nil, &size, nil, 0) == 0, size > 0 else { return "Mac" }
        var raw = [CChar](repeating: 0, count: size)
        guard sysctlbyname("hw.model", &raw, &size, nil, 0) == 0 else { return "Mac" }
        return String(cString: raw).hasPrefix("VirtualMac") ? "Mac virtual machine" : "Mac"
    }
    #endif

    /// What a login request tells the server about this phone's key (auth-device.c): the session is
    /// bound to it. Not sent with a refresh (which proves the key instead) or a logout.
    func deviceHeaders(for path: String) -> [String: String] {
        guard let device, path != AppAuth.refreshPath, path != "/api/auth/logout", let key = try? device.current() else { return [:] }
        return ["OA-Device-Key": key.publicKeyHex, "OA-Device-Platform": AppAuth.platform, "OA-Device-Store": key.store.rawValue, "OA-Device-Name": AppAuth.deviceName]
    }

    /// The refresh request for these tokens, signed at `ms` by the device key when the login is bound.
    private func refresh(_ t: Tokens, ms: Int64) async throws -> [String: Any] {
        var headers: [String: String] = [:]
        if t.deviceBound == true, let key = device?.existing(),
           let proof = try? DeviceKeys.proof(key: key, method: "POST", url: origin + AppAuth.refreshPath, ms: ms, token: t.refresh) {
            headers["OA-Device-Proof"] = proof
        }
        return try await post(AppAuth.refreshPath, ["refresh_token": t.refresh], headers: headers)
    }

    /// A refresh; when the server says our clock is more than a minute off, once more on its time.
    private func renew(_ t: Tokens) async throws -> [String: Any] {
        do { return try await refresh(t, ms: AppAuth.ms(clock())) }
        catch let r as Refused {
            guard r.status == 400, r.proof == "clock", let now = r.nowMs else { throw r }
            return try await refresh(t, ms: now)
        }
    }

    // MARK: pieces

    private func said<T>(_ work: () async throws -> T) async throws -> T {
        do { return try await work() } catch let r as Refused { throw Failure.said(r.words) }
    }

    /// The words of an answer's {"error"}, or nil.
    static func errorWords(_ data: Data) -> String? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let words = obj["error"] as? String, !words.isEmpty else { return nil }
        return words
    }

    /// Keeps a logged-in answer's tokens; refresh answers carry no account, so it stays.
    @discardableResult
    private func keep(_ answer: [String: Any], account: String? = nil, bound: Bool? = nil) throws -> Tokens {
        guard let access = answer["access_token"] as? String, access.hasPrefix("oa_at_"),
              let refresh = answer["refresh_token"] as? String, refresh.hasPrefix("oa_rt_") else { throw Failure.said("That didn't work. Try again.") }
        let seconds = (answer["expires_in"] as? NSNumber)?.doubleValue ?? 3600
        let t = Tokens(access: access, refresh: refresh, accessUntil: clock().addingTimeInterval(seconds),
                       account: (answer["account"] as? String) ?? account ?? tokens?.account ?? "",
                       deviceBound: (answer["device_bound"] as? Bool) == true ? true : bound)
        store.save(t)
        return t
    }

    private func post(_ path: String, _ json: [String: String], authorization: String? = nil, headers: [String: String] = [:]) async throws -> [String: Any] {
        let body = String(decoding: try JSONSerialization.data(withJSONObject: json, options: [.sortedKeys]), as: UTF8.self)
        return try await post(path, raw: body, authorization: authorization, headers: headers)
    }
    private func post(_ path: String, raw body: String, authorization: String?, headers: [String: String] = [:]) async throws -> [String: Any] {
        guard let url = URL(string: origin + path) else { throw Failure.unreachable }
        var r = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20)
        r.httpMethod = "POST"
        r.httpBody = Data(body.utf8)
        r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        if let authorization { r.setValue(authorization, forHTTPHeaderField: "Authorization") }
        for (name, value) in deviceHeaders(for: path) { r.setValue(value, forHTTPHeaderField: name) }
        for (name, value) in headers { r.setValue(value, forHTTPHeaderField: name) }
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw Failure.unreachable }
        guard (200..<300).contains(status) else {
            let said = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            throw Refused(status: status, words: AppAuth.errorWords(data) ?? "That didn't work. Try again.",
                          proof: said?["device_proof"] as? String, nowMs: (said?["now_ms"] as? NSNumber)?.int64Value)
        }
        return (try? JSONSerialization.jsonObject(with: data) as? [String: Any]) ?? [:]
    }
}

/// Where the tokens live.
protocol TokenStore: Sendable {
    func load() -> AppAuth.Tokens?
    func save(_ tokens: AppAuth.Tokens)
    func clear()
}

#if os(iOS)
/// The Keychain item (see AppAuth).
struct KeychainTokenStore: TokenStore {
    private let base: [String: Any] = [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: "com.openagents.app.auth",
        kSecAttrAccount as String: "tokens-v1",
        kSecAttrSynchronizable as String: false,
    ]
    func load() -> AppAuth.Tokens? {
        var q = base
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
        return try? JSONDecoder().decode(AppAuth.Tokens.self, from: data)
    }
    func save(_ tokens: AppAuth.Tokens) {
        guard let data = try? JSONEncoder().encode(tokens) else { return }
        let update = [kSecValueData as String: data]
        if SecItemUpdate(base as CFDictionary, update as CFDictionary) == errSecItemNotFound {
            var item = base
            item[kSecValueData as String] = data
            item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            SecItemAdd(item as CFDictionary, nil)
        }
    }
    func clear() { SecItemDelete(base as CFDictionary) }
}

/// The device key's Keychain item (see `DeviceKeys`): the Secure Enclave's wrapped blob, beside the
/// tokens, this device only, after first unlock, never synced.
struct KeychainDeviceKeyStore: DeviceKeyStore {
    private var base: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "com.openagents.app.auth",
         kSecAttrAccount as String: "device-key-v1",
         kSecAttrSynchronizable as String: false]
    }
    func load() -> Data? {
        var q = base
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var out: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess else { return nil }
        return out as? Data
    }
    func save(_ blob: Data) {
        let update = [kSecValueData as String: blob]
        if SecItemUpdate(base as CFDictionary, update as CFDictionary) == errSecItemNotFound {
            var item = base
            item[kSecValueData as String] = blob
            item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            SecItemAdd(item as CFDictionary, nil)
        }
    }
    func clear() { SecItemDelete(base as CFDictionary) }
}
#else
/// The Mac's tokens: a file only the user can read (0600, its folder 0700), never the Keychain (see
/// AppAuth). `~/.openagents/<app>/login.json`.
struct FileTokenStore: TokenStore {
    let url: URL
    init(url: URL) { self.url = url }
    init(app: String) {
        url = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent(app, isDirectory: true)
            .appendingPathComponent("login.json")
    }
    func load() -> AppAuth.Tokens? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        return try? JSONDecoder().decode(AppAuth.Tokens.self, from: data)
    }
    func save(_ tokens: AppAuth.Tokens) {
        guard let data = try? JSONEncoder().encode(tokens) else { return }
        let fm = FileManager.default
        try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        // Written beside, 0600 from the start, then moved over: never a moment readable by others.
        let next = url.appendingPathExtension("new")
        try? fm.removeItem(at: next)
        guard fm.createFile(atPath: next.path, contents: data, attributes: [.posixPermissions: 0o600]) else { return }
        if rename(next.path, url.path) != 0 { try? fm.removeItem(at: next) }
    }
    func clear() { try? FileManager.default.removeItem(at: url) }
}
#endif
