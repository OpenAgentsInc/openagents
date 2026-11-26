import AuthenticationServices
import CryptoKit
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
///
/// Security notices (#13233, docs/auth "Notify"): the app gives the server the push token Apple
/// gave this install (`pushTarget`), and the account's security notices then reach this device as
/// push. A notice's push opens the app's Security screen (`noticeOpens`), which reads what
/// happened (`security`) and can cancel a recovery or freeze the account with this login
/// (`cancelRecovery`, `freeze`). The push carries no link or token; the login is the proof.
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
    private var owner=UUID()
    /// A login lifetime, unchanged by token renewal; SDK callbacks bind to it.
    var sessionOwner:UUID {lock.lock();defer {lock.unlock()};return owner}
    @discardableResult private func forgetSession(ifOwner expected:UUID?=nil)->Bool {
        lock.lock()
        if let expected,expected != owner {lock.unlock();return false}
        owner=UUID();refreshing?.cancel();refreshing=nil;store.clear();lock.unlock()
        device?.forget();NotificationCenter.default.post(name:AppAuth.didChangeSession,object:self);return true
    }

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
    /// The lean iPhone app (GitHub #13645) holds no Nostr key: it logs in with an email code. An app
    /// that has one passes its own signer.
    static let defaultSign: Sign = { _, _, _ in throw Failure.said("This phone has no key to log in with.") }
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

    // MARK: WorkOS authentication (docs/auth/2026-11-26-workos-coax-sdk.md, #14101)

    struct WorkOSConfig: Equatable { let enabled: Bool; let clientID: String; let providers: [String] }
    struct WorkOSOrg: Equatable { let id: String; let name: String }
    enum WorkOSAuthResult: Equatable {
        case loggedIn(Tokens)
        case needOrg(pending: String, organizations: [WorkOSOrg])
    }
    struct WorkOSDiscovery: Equatable { let sso: Bool; let connectionID: String; let organizationID: String; let organizationName: String }
    struct WorkOSPKCE: Equatable {
        let verifier: String
        let challenge: String
        let state: String
        let redirectURI: String
        let authorizeURL: URL
    }

    /// Reads whether WorkOS is enabled on this API origin and its public client_id.
    func workosConfig() async throws -> WorkOSConfig {
        guard let url = URL(string: origin + "/api/auth/workos/config") else { throw Failure.unreachable }
        let r = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 10)
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw Failure.unreachable }
        guard (200..<300).contains(status), let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return WorkOSConfig(enabled: false, clientID: "", providers: [])
        }
        let enabled = (o["enabled"] as? Bool) == true
        let cid = o["client_id"] as? String ?? ""
        let providers = o["providers"] as? [String] ?? []
        return WorkOSConfig(enabled: enabled && !cid.isEmpty, clientID: cid, providers: providers)
    }

    /// Sends a 6-digit WorkOS Magic Auth code; returns the pending handle for workosMagicVerify.
    func workosMagicStart(_ email: String) async throws -> String {
        let answer = try await said { try await self.post("/api/auth/workos/magic/start", ["email": email, "client": "app"]) }
        guard let pending = answer["pending"] as? String, !pending.isEmpty else { throw Failure.said("That didn't work. Try again.") }
        return pending
    }

    /// Verifies a 6-digit WorkOS Magic Auth code, binding the Option A1 session to this device key.
    func workosMagicVerify(code: String, pending: String) async throws -> WorkOSAuthResult {
        let answer = try await said { try await self.post("/api/auth/workos/magic/verify", ["code": code, "pending": pending, "client": "app"]) }
        return try parseWorkOSAuthResult(answer)
    }

    /// Completes multi-organization selection after a `needOrg` response.
    @discardableResult
    func workosSelectOrg(pending: String, organizationID: String) async throws -> Tokens {
        try keep(try await said { try await self.post("/api/auth/workos/org/select", ["pending": pending, "organization_id": organizationID, "client": "app"]) })
    }

    /// Discovers whether an email domain has an active WorkOS Enterprise SSO connection.
    func workosDiscoverSSO(email: String) async throws -> WorkOSDiscovery {
        let answer = try await said { try await self.post("/api/auth/workos/sso/discover", ["email": email]) }
        return WorkOSDiscovery(
            sso: (answer["sso"] as? Bool) == true,
            connectionID: answer["connection_id"] as? String ?? "",
            organizationID: answer["organization_id"] as? String ?? "",
            organizationName: answer["organization_name"] as? String ?? ""
        )
    }

    /// Builds a fresh RFC 7636 PKCE S256 challenge, CSRF state, and WorkOS authorize URL for native Apple OAuth/SSO.
    static func workosPKCEStart(
        clientID: String,
        provider: String? = nil,
        connectionID: String? = nil,
        organizationID: String? = nil,
        loginHint: String? = nil,
        redirectURI: String = "openagents://auth/callback",
        workosOrigin: String = "https://api.workos.com"
    ) throws -> WorkOSPKCE {
        guard !clientID.isEmpty, !redirectURI.isEmpty else { throw Failure.said("WorkOS sign-in is not configured.") }
        var vBytes = Data(count: 32), sBytes = Data(count: 32)
        let r1 = vBytes.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }
        let r2 = sBytes.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }
        guard r1 == errSecSuccess, r2 == errSecSuccess else { throw Failure.said("Could not generate sign-in randomness.") }
        let verifier = b64url(vBytes)
        let challenge = b64url(Data(SHA256.hash(data: Data(verifier.utf8))))
        let state = b64url(sBytes)
        let base = workosOrigin.hasSuffix("/") ? String(workosOrigin.dropLast()) : workosOrigin
        guard var comps = URLComponents(string: base + "/user_management/authorize") else { throw Failure.said("Invalid WorkOS origin.") }
        var items: [URLQueryItem] = [
            URLQueryItem(name: "client_id", value: clientID),
            URLQueryItem(name: "redirect_uri", value: redirectURI),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
            URLQueryItem(name: "state", value: state),
        ]
        if let p = provider, !p.isEmpty { items.append(URLQueryItem(name: "provider", value: p)) }
        if let c = connectionID, !c.isEmpty { items.append(URLQueryItem(name: "connection_id", value: c)) }
        if let o = organizationID, !o.isEmpty { items.append(URLQueryItem(name: "organization_id", value: o)) }
        if let h = loginHint, !h.isEmpty { items.append(URLQueryItem(name: "login_hint", value: h)) }
        comps.queryItems = items
        guard let url = comps.url else { throw Failure.said("Could not construct WorkOS sign-in URL.") }
        return WorkOSPKCE(verifier: verifier, challenge: challenge, state: state, redirectURI: redirectURI, authorizeURL: url)
    }

    /// Constant-time state verification and authorization code extraction from a native OAuth callback URL.
    static func workosCallbackCode(callbackURL: URL, expectedState: String) throws -> String {
        guard let items = URLComponents(url: callbackURL, resolvingAgainstBaseURL: false)?.queryItems else {
            throw Failure.said("Sign-in did not return an authorization code.")
        }
        let map = Dictionary(items.compactMap { item in item.value.map { (item.name, $0) } }, uniquingKeysWith: { first, _ in first })
        if let err = map["error"], !err.isEmpty {
            throw Failure.said(map["error_description"] ?? "Sign-in was cancelled or denied.")
        }
        guard let gotState = map["state"], !gotState.isEmpty, constantTimeEqual(gotState, expectedState) else {
            throw Failure.said("Sign-in state did not match. Try again.")
        }
        guard let code = map["code"], !code.isEmpty else {
            throw Failure.said("Sign-in did not return an authorization code.")
        }
        return code
    }

    /// Constant-time ASCII/UTF-8 string comparison for CSRF state validation.
    private static func constantTimeEqual(_ a: String, _ b: String) -> Bool {
        let ab = Array(a.utf8), bb = Array(b.utf8)
        guard ab.count == bb.count, !ab.isEmpty else { return false }
        var diff: UInt8 = 0
        for i in 0..<ab.count { diff |= ab[i] ^ bb[i] }
        return diff == 0
    }

    /// Exchanges a native PKCE (code, code_verifier) at oa-api for Option A1 device-bound oa_at_/oa_rt_ tokens.
    func workosExchangePKCE(code: String, pkce: WorkOSPKCE) async throws -> WorkOSAuthResult {
        let answer = try await said {
            try await self.post("/api/auth/workos/exchange", [
                "code": code,
                "code_verifier": pkce.verifier,
                "redirect_uri": pkce.redirectURI,
                "client": "app",
            ])
        }
        return try parseWorkOSAuthResult(answer)
    }

    /// Lists WorkOS organizations linked to the signed-in account.
    func workosOrgs() async throws -> [WorkOSOrg] {
        let o = try await get("/api/auth/workos/orgs")
        let raw = o["organizations"] as? [[String: Any]] ?? []
        return raw.compactMap { row in
            guard let id = row["id"] as? String, !id.isEmpty else { return nil }
            return WorkOSOrg(id: id, name: (row["name"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? id)
        }
    }

    /// Switches the active WorkOS organization context for the signed-in account.
    @discardableResult
    func workosSwitchOrg(_ organizationID: String) async throws -> String {
        let answer = try await said { try await self.authed("/api/auth/workos/orgs/switch", ["organization_id": organizationID]) }
        return (answer["organization_id"] as? String) ?? organizationID
    }

    /// Opens ASWebAuthenticationSession with prefersEphemeralWebBrowserSession = false (shared SSO cookies)
    /// and returns the validated authorization code for `workosExchangePKCE`.
    @MainActor
    static func workosWebAuthorize(pkce: WorkOSPKCE, anchor: ASPresentationAnchor?) async throws -> String {
        let scheme = URL(string: pkce.redirectURI)?.scheme ?? "openagents"
        let callbackURL: URL = try await withCheckedThrowingContinuation { cont in
            let provider = WorkOSWebAnchor(anchor: anchor)
            let session = ASWebAuthenticationSession(url: pkce.authorizeURL, callbackURLScheme: scheme) { url, error in
                _ = provider // keep strong reference until completion fires
                if let url {
                    cont.resume(returning: url)
                } else if let nsErr = error as NSError?, nsErr.domain == ASWebAuthenticationSessionErrorDomain,
                          nsErr.code == ASWebAuthenticationSessionError.canceledLogin.rawValue {
                    cont.resume(throwing: Failure.said("Sign-in cancelled."))
                } else {
                    cont.resume(throwing: Failure.said(error?.localizedDescription ?? "Sign-in did not complete."))
                }
            }
            session.presentationContextProvider = provider
            session.prefersEphemeralWebBrowserSession = false
            if !session.start() {
                cont.resume(throwing: Failure.said("Could not open the sign-in browser."))
            }
        }
        return try workosCallbackCode(callbackURL: callbackURL, expectedState: pkce.state)
    }

    private func parseWorkOSAuthResult(_ answer: [String: Any]) throws -> WorkOSAuthResult {
        if (answer["state"] as? String) == "need-org", let pending = answer["pending"] as? String, !pending.isEmpty {
            let raw = answer["organizations"] as? [[String: Any]] ?? []
            let orgs: [WorkOSOrg] = raw.compactMap { row in
                guard let id = row["id"] as? String, !id.isEmpty else { return nil }
                return WorkOSOrg(id: id, name: (row["name"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? id)
            }
            return .needOrg(pending: pending, organizations: orgs)
        }
        return .loggedIn(try keep(answer))
    }

    /// Logs in with this device's key: a challenge, signed (NIP-98) over the login route and body.
    @discardableResult
    func keyLogin(signer: Sign? = nil) async throws -> Tokens {
        let challenge = try await said { try await self.post("/api/auth/nostr/challenge", ["client": "app"]) }
        guard let ch = challenge["challenge"] as? String else { throw Failure.said("That didn't work. Try again.") }
        let body = "{\"challenge\":\"\(ch)\",\"label\":\"app\"}"
        let url = origin + "/api/auth/nostr"
        let auth = try (signer ?? sign)(url, body, clock())
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
    // MARK: security notices (#13233)

    /// Posted when a login completes (not a refresh), so the app can give its push target.
    static let didLogIn = Notification.Name("OAAppAuthDidLogIn")
    static let didChangeSession = Notification.Name("OAAppAuthDidChangeSession")
    /// Which of Apple's push services this build's token is for: a development build's token is
    /// only good on Apple's sandbox (coax/web/api/auth-device-policy.coax, dk_push_service).
    /// The Mac build sets OA_PUSH_SANDBOX when it is signed with a development profile (#13388).
    #if DEBUG || OA_PUSH_SANDBOX
    static let pushService = "apns_sandbox"
    #else
    static let pushService = "apns"
    #endif
    /// Apple's device token as the server keeps it: lower-case hex.
    static func pushToken(_ token: Data) -> String { token.map { String(format: "%02x", $0) }.joined() }
    /// Gives the server this install's push token, so the account's security notices reach this
    /// device (POST /api/auth/devices/push). Does nothing when nobody is logged in. The server
    /// refuses a login that isn't tied to this device's key, and a limited one.
    func pushTarget(_ token: Data) async throws {
        guard loggedIn else { return }
        _ = try await said { try await self.authed("/api/auth/devices/push", ["service": AppAuth.pushService, "token": AppAuth.pushToken(token)]) }
    }
    /// The page a notice's push names (proto/auth/push-notice-v1.json, oa.open). The app opens its
    /// Security screen for that and nothing else, whatever a push says.
    static let noticePage = "/settings/security"
    static func noticeOpens(_ userInfo: [AnyHashable: Any]) -> Bool {
        ((userInfo["oa"] as? [String: Any])?["open"] as? String) == noticePage
    }
    /// One security notice the account was sent: when, its kind and its words.
    struct Notice: Equatable { let atMs: Int64; let what: String; let words: String }
    /// What the Security screen shows: the delayed recovery's state (none, pending, ready,
    /// cancelled, done), when it is ready, and the newest notices.
    struct Security: Equatable {
        let recovery: String; let readyMs: Int64; let notices: [Notice]
        /// A recovery is waiting, so it can be cancelled.
        var recoveryOpen: Bool { recovery == "pending" || recovery == "ready" }
        /// What the Security screen says about the recovery, with its time in the device's zone.
        func recoveryWords(zone: TimeZone = .current) -> String {
            guard recoveryOpen else { return "No recovery is waiting." }
            if recovery == "ready" { return "A recovery of this account is ready to finish: a new key can replace every key and passkey on it." }
            let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.timeZone = zone; f.dateFormat = "MMM d, h:mm a zzz"
            return "A recovery of this account is waiting. After \(f.string(from: Date(timeIntervalSince1970: Double(readyMs) / 1000))) a new key can replace every key and passkey on it."
        }
        /// The newest notices (at most `most`), one a paragraph: when, then its words.
        func noticeWords(most: Int = 5, zone: TimeZone = .current) -> String {
            guard !notices.isEmpty else { return "None yet. We send one for every change to how you log in." }
            let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.timeZone = zone; f.dateFormat = "MMM d, h:mm a"
            return notices.prefix(most).map { f.string(from: Date(timeIntervalSince1970: Double($0.atMs) / 1000)) + "\n" + $0.words }.joined(separator: "\n\n")
        }
    }
    /// The Security screen's words, the same on the iPhone and the Mac.
    static let recoveryCancelledWords = "The recovery was cancelled. Nothing changed on the account."
    static let freezeAskTitle = "Freeze this account?"
    static let freezeAskWords = "Every login on it ends, this one too, and any recovery is cancelled."
    static let frozenWords = "Your account is frozen: every login on it ended, and any recovery is cancelled. Log in with your key to unfreeze it."
    func security() async throws -> Security {
        let r = try await get("/api/auth/recovery")
        let n = try await get("/api/auth/notices")
        let notices = (n["notices"] as? [[String: Any]] ?? []).prefix(10).map {
            Notice(atMs: ($0["at_ms"] as? NSNumber)?.int64Value ?? 0, what: $0["what"] as? String ?? "", words: $0["words"] as? String ?? "")
        }
        return Security(recovery: r["state"] as? String ?? "none", readyMs: (r["ready_ms"] as? NSNumber)?.int64Value ?? 0, notices: Array(notices))
    }
    /// Reads the signed-in account's service credit balance in millisatoshis (GET /api/auth/life).
    func creditMsat() async throws -> Int64 {
        let life = try await get("/api/auth/life")
        return max(0, (life["credit_msat"] as? NSNumber)?.int64Value ?? 0)
    }
    /// Cancels the account's waiting recovery with this login (any login of the account may).
    func cancelRecovery() async throws {
        _ = try await said { try await self.authed("/api/auth/recovery/cancel", [:]) }
    }
    /// "This wasn't me": freezes the account. Every login ends, this one too, so the app forgets it.
    func freeze() async throws {
        let current=sessionOwner
        _ = try await said { try await self.authed("/api/auth/freeze", [:]) }
        forgetSession(ifOwner:current)
    }
    /// A GET with this login's access token: the answer's object.
    private func get(_ path: String) async throws -> [String: Any] {
        guard let url = URL(string: origin + path) else { throw Failure.unreachable }
        let r = try await authorize(URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: 20))
        let data: Data, status: Int
        do { (data, status) = try await send(r) } catch { throw Failure.unreachable }
        guard (200..<300).contains(status), let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Failure.said(AppAuth.errorWords(data) ?? (status == 401 ? "Log in again." : "That didn't work. Try again."))
        }
        return o
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
        let t=tokens;forgetSession()
        if let t {
            _ = try? await post("/api/auth/logout", [:], authorization: "Bearer " + t.access)
        }
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
        let current=owner
        guard let old=store.load() else {lock.unlock();throw Failure.said("Log in first.")}
        let task = Task<Tokens, Error> { [self] in
            do {
                return try keep(try await renew(old), account: old.account, bound: old.deviceBound,expectedOwner:current)
            } catch let r as Refused where r.status == 401 {
                // The login ended (a reused token, idle, logged out elsewhere, or its device was
                // removed): forget it, and its device key with it.
                forgetSession(ifOwner:current)
                throw Failure.said(r.words)
            } catch let r as Refused {
                throw Failure.said(r.words)   // a limit and the like: the login stays
            }
        }
        refreshing = task
        lock.unlock()
        defer { lock.lock();if owner==current {refreshing = nil};lock.unlock() }
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
    private func keep(_ answer: [String: Any], account: String? = nil, bound: Bool? = nil,expectedOwner:UUID?=nil) throws -> Tokens {
        guard let access = answer["access_token"] as? String, access.hasPrefix("oa_at_"),
              let refresh = answer["refresh_token"] as? String, refresh.hasPrefix("oa_rt_") else { throw Failure.said("That didn't work. Try again.") }
        let seconds = (answer["expires_in"] as? NSNumber)?.doubleValue ?? 3600
        let t = Tokens(access: access, refresh: refresh, accessUntil: clock().addingTimeInterval(seconds),
                       account: (answer["account"] as? String) ?? account ?? tokens?.account ?? "",
                       deviceBound: (answer["device_bound"] as? Bool) == true ? true : bound)
        lock.lock()
        if let expectedOwner,expectedOwner != owner {lock.unlock();throw CancellationError()}
        if account==nil {owner=UUID();refreshing?.cancel();refreshing=nil}
        store.save(t);lock.unlock()
        if account == nil {NotificationCenter.default.post(name:AppAuth.didChangeSession,object:self);NotificationCenter.default.post(name: AppAuth.didLogIn, object: self)}   // a login, not a refresh
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
/// `account` names another item of the same kind: the own-computer device key and its pairings
/// (apps/shared/native/AppDeviceLink.swift).
struct KeychainDeviceKeyStore: DeviceKeyStore {
    var account = "device-key-v1"
    private var base: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "com.openagents.app.auth",
         kSecAttrAccount as String: account,
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

@MainActor
private final class WorkOSWebAnchor: NSObject, ASWebAuthenticationPresentationContextProviding {
    private let anchor: ASPresentationAnchor?
    init(anchor: ASPresentationAnchor?) { self.anchor = anchor }
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        if let anchor { return anchor }
        #if os(iOS)
        return ASPresentationAnchor()
        #else
        return NSApplication.shared.keyWindow ?? NSApplication.shared.windows.first ?? ASPresentationAnchor()
        #endif
    }
}
