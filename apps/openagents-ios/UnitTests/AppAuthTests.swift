import AuthenticationServices
import CryptoKit
import XCTest
@testable import OpenAgents

/// AppAuth's token handling against a fake server: a fresh access token is used as is, a stale one
/// is renewed once however many requests ask, a refresh token the server says was reused ends the
/// login (and the app forgets it), and a limit on refreshing leaves the login alone.
final class AppAuthTests: XCTestCase {
    final class Memory: TokenStore, @unchecked Sendable {
        var tokens: AppAuth.Tokens?
        func load() -> AppAuth.Tokens? { tokens }
        func save(_ t: AppAuth.Tokens) { tokens = t }
        func clear() { tokens = nil }
    }
    /// Answers by path; keeps what it was asked.
    final class Server: @unchecked Sendable {
        let lock = NSLock()
        var asked: [(path: String, auth: String, body: String)] = []
        var requests: [URLRequest] = []
        var answers: [String: [(Int, String)]] = [:]
        func send(_ r: URLRequest) async throws -> (Data, Int) {
            let path = r.url?.path ?? ""
            lock.lock(); defer { lock.unlock() }
            asked.append((path, r.value(forHTTPHeaderField: "Authorization") ?? "", String(decoding: r.httpBody ?? Data(), as: UTF8.self)))
            requests.append(r)
            var queue = answers[path] ?? []
            guard !queue.isEmpty else { return (Data("{\"error\":\"Not found.\"}".utf8), 404) }
            let a = queue.removeFirst(); answers[path] = queue
            return (Data(a.1.utf8), a.0)
        }
        func count(_ path: String) -> Int { lock.lock(); defer { lock.unlock() }; return asked.filter { $0.path == path }.count }
    }

    let now = Date(timeIntervalSince1970: 1_800_000_000)
    func pair(_ n: Int) -> String {
        "{\"access_token\":\"oa_at_A\(n)\",\"refresh_token\":\"oa_rt_R\(n)\",\"expires_in\":3600}"
    }
    func make(_ server: Server, _ store: Memory, at: Date? = nil, device: DeviceKeys? = nil) -> AppAuth {
        let t = at ?? now
        return AppAuth(origin: "https://example.test", send: { try await server.send($0) }, store: store, clock: { t }, device: device)
    }
    /// Where a test's device key is kept, and a software key in it (no Secure Enclave in a test).
    final class KeyMemory: DeviceKeyStore, @unchecked Sendable {
        var blob: Data?
        func load() -> Data? { blob }
        func save(_ b: Data) { blob = b }
        func clear() { blob = nil }
    }
    func keys(_ store: KeyMemory) -> DeviceKeys { DeviceKeys(store: store, enclave: false) }
    /// True when value is "v1 <ms> <r || s in hex>" by key over this refresh of token at ms.
    func proves(_ value: String?, _ key: DeviceKey, token: String, ms: Int64) -> Bool {
        let parts = (value ?? "").split(separator: " ").map(String.init)
        guard parts.count == 3, parts[0] == "v1", parts[1] == String(ms), parts[2].count == 128 else { return false }
        var raw = Data(); var at = parts[2].startIndex
        while at < parts[2].endIndex { let next = parts[2].index(at, offsetBy: 2); raw.append(UInt8(parts[2][at..<next], radix: 16) ?? 0); at = next }
        let message = DeviceKeys.message(method: "POST", url: "https://example.test/api/auth/refresh", ms: ms, token: token)
        guard let pub = try? P256.Signing.PublicKey(x963Representation: key.publicKey), let sig = try? P256.Signing.ECDSASignature(rawRepresentation: raw) else { return false }
        return pub.isValidSignature(sig, for: Data(message.utf8))
    }
    func request() -> URLRequest { URLRequest(url: URL(string: "https://example.test/api/v1/inference")!) }

    func testNext() {
        XCTAssertEqual(AppAuth.next(nil, now: now), .loggedOut)
        let t = AppAuth.Tokens(access: "oa_at_A", refresh: "oa_rt_R", accessUntil: now.addingTimeInterval(600), account: "acct_x")
        XCTAssertEqual(AppAuth.next(t, now: now), .useAccess("oa_at_A"))
        XCTAssertEqual(AppAuth.next(t, now: now.addingTimeInterval(541)), .refresh("oa_rt_R"))   // under a minute left
        XCTAssertEqual(AppAuth.next(t, now: now.addingTimeInterval(700)), .refresh("oa_rt_R"))
    }

    func testEmailLoginKeepsThePairAndAccount() async throws {
        let server = Server(), store = Memory()
        server.answers["/api/auth/email/start"] = [(200, "{\"state\":\"sent\",\"pending\":\"p1\"}")]
        server.answers["/api/auth/email/verify"] = [(401, "{\"error\":\"That code isn't right. 4 tries left.\"}"),
                                                     (200, "{\"state\":\"logged-in\",\"account\":\"acct_a\",\"access_token\":\"oa_at_A1\",\"refresh_token\":\"oa_rt_R1\",\"expires_in\":3600}")]
        let auth = make(server, store)
        let pending = try await auth.emailStart("ada@example.com")
        XCTAssertEqual(pending, "p1")
        XCTAssertEqual(server.asked[0].body, "{\"client\":\"app\",\"email\":\"ada@example.com\"}")
        do { try await auth.emailVerify(code: "000000", pending: pending); XCTFail("wrong code") }
        catch { XCTAssertEqual(error.localizedDescription, "That code isn't right. 4 tries left.") }
        XCTAssertNil(store.tokens)
        try await auth.emailVerify(code: "482913", pending: pending)
        XCTAssertEqual(store.tokens, AppAuth.Tokens(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(3600), account: "acct_a"))
        XCTAssertEqual(server.asked[2].body, "{\"code\":\"482913\",\"pending\":\"p1\"}")
    }

    func testFreshAccessIsUsedWithoutARefresh() async throws {
        let server = Server(), store = Memory()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(3000), account: "acct_a")
        let r = try await make(server, store).authorize(request())
        XCTAssertEqual(r.value(forHTTPHeaderField: "Authorization"), "Bearer oa_at_A1")
        XCTAssertTrue(server.asked.isEmpty)
    }

    func testStaleAccessIsRenewedOnceForManyRequests() async throws {
        let server = Server(), store = Memory()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a")
        server.answers["/api/auth/refresh"] = [(200, pair(2))]
        let auth = make(server, store)
        let got = try await withThrowingTaskGroup(of: String.self) { group in
            for _ in 0..<5 { group.addTask { try await auth.authorize(self.request()).value(forHTTPHeaderField: "Authorization") ?? "" } }
            var all: [String] = []
            for try await g in group { all.append(g) }
            return all
        }
        XCTAssertEqual(Set(got), ["Bearer oa_at_A2"])
        XCTAssertEqual(server.count("/api/auth/refresh"), 1)
        XCTAssertEqual(server.asked[0].body, "{\"refresh_token\":\"oa_rt_R1\"}")
        XCTAssertEqual(store.tokens?.refresh, "oa_rt_R2")
        XCTAssertEqual(store.tokens?.account, "acct_a")   // a refresh answer has no account: it stays
    }

    func testAReusedRefreshTokenEndsTheLogin() async throws {
        let server = Server(), store = Memory()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a")
        server.answers["/api/auth/refresh"] = [(401, "{\"error\":\"This login was ended because its token was used twice. Log in again.\"}")]
        do { _ = try await make(server, store).authorize(request()); XCTFail("ended") }
        catch { XCTAssertEqual(error.localizedDescription, "This login was ended because its token was used twice. Log in again.") }
        XCTAssertNil(store.tokens)
        do { _ = try await make(server, store).authorize(request()); XCTFail("logged out") }
        catch { XCTAssertEqual(error.localizedDescription, "Log in first.") }
    }

    func testALimitOnRefreshingKeepsTheLogin() async throws {
        let server = Server(), store = Memory()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a")
        server.answers["/api/auth/refresh"] = [(429, "{\"error\":\"Too many refreshes. Try again in 12 minutes.\"}")]
        do { _ = try await make(server, store).authorize(request()); XCTFail("limited") }
        catch { XCTAssertEqual(error.localizedDescription, "Too many refreshes. Try again in 12 minutes.") }
        XCTAssertEqual(store.tokens?.refresh, "oa_rt_R1")
    }

    func testUnreachableSaysSo() async {
        let store = Memory()
        let auth = AppAuth(origin: "https://example.test", send: { _ in throw URLError(.notConnectedToInternet) }, store: store, clock: { Date() })
        do { _ = try await auth.emailStart("ada@example.com"); XCTFail() }
        catch { XCTAssertEqual(error.localizedDescription, "Couldn't reach openagents.com. Check the connection and try again.") }
    }

    func testLogoutEndsTheSessionAndForgets() async {
        let server = Server(), store = Memory()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(3000), account: "acct_a")
        server.answers["/api/auth/logout"] = [(200, "{\"state\":\"logged-out\"}")]
        await make(server, store).logout()
        XCTAssertNil(store.tokens)
        XCTAssertEqual(server.asked.first?.auth, "Bearer oa_at_A1")
    }

    // MARK: the device key (#13216)

    func testTheProofSignsTheServersMessage() {
        // auth-device-crypto.coax's own vector ("test" hashes to 9f86...).
        XCTAssertEqual(DeviceKeys.message(method: "POST", url: "https://openagents.com/api/auth/refresh", ms: 1_800_000_000_000, token: "test"),
                       "OA-Device-Proof-v1\nPOST\nhttps://openagents.com/api/auth/refresh\n1800000000000\n9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08")
        XCTAssertEqual(AppAuth.ms(Date(timeIntervalSince1970: 1_800_000_000)), 1_800_000_000_000)
    }

    func testALoginSendsTheDeviceKeyAndRemembersItWasBound() async throws {
        let server = Server(), store = Memory(), kept = KeyMemory()
        server.answers["/api/auth/email/start"] = [(200, "{\"state\":\"sent\",\"pending\":\"p1\"}")]
        server.answers["/api/auth/email/verify"] = [(200, "{\"state\":\"logged-in\",\"account\":\"acct_a\",\"access_token\":\"oa_at_A1\",\"refresh_token\":\"oa_rt_R1\",\"expires_in\":3600,\"device_bound\":true}")]
        let device = keys(kept), auth = make(server, store, device: device)
        let pending = try await auth.emailStart("ada@example.com")
        XCTAssertNotNil(kept.blob)   // made silently, at the first login request
        let key = try XCTUnwrap(device.existing())
        try await auth.emailVerify(code: "482913", pending: pending)
        let sent = server.requests[1]
        XCTAssertEqual(sent.value(forHTTPHeaderField: "OA-Device-Key"), key.publicKeyHex)
        XCTAssertEqual(key.publicKeyHex.count, 130); XCTAssertTrue(key.publicKeyHex.hasPrefix("04"))
        XCTAssertEqual(sent.value(forHTTPHeaderField: "OA-Device-Platform"), "ios")
        XCTAssertEqual(sent.value(forHTTPHeaderField: "OA-Device-Store"), "software")
        XCTAssertFalse((sent.value(forHTTPHeaderField: "OA-Device-Name") ?? "").isEmpty)
        XCTAssertNil(sent.value(forHTTPHeaderField: "OA-Device-Proof"))
        XCTAssertEqual(store.tokens?.deviceBound, true)
    }

    func testABoundLoginRefreshesWithAProofByItsKey() async throws {
        let server = Server(), store = Memory(), kept = KeyMemory()
        let device = keys(kept), key = try device.current()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a", deviceBound: true)
        server.answers["/api/auth/refresh"] = [(200, pair(2))]
        let r = try await make(server, store, device: device).authorize(request())
        XCTAssertEqual(r.value(forHTTPHeaderField: "Authorization"), "Bearer oa_at_A2")
        let sent = server.requests[0]
        XCTAssertTrue(proves(sent.value(forHTTPHeaderField: "OA-Device-Proof"), key, token: "oa_rt_R1", ms: 1_800_000_000_000))
        XCTAssertFalse(proves(sent.value(forHTTPHeaderField: "OA-Device-Proof"), key, token: "oa_rt_R2", ms: 1_800_000_000_000))
        XCTAssertNil(sent.value(forHTTPHeaderField: "OA-Device-Key"))   // a refresh proves the key; it doesn't send it
        XCTAssertEqual(store.tokens?.refresh, "oa_rt_R2")
        XCTAssertEqual(store.tokens?.deviceBound, true)   // a refresh answer doesn't say: it stays bound
        XCTAssertEqual(device.existing()?.publicKey, key.publicKey)
    }

    func testAClockThatIsOffSignsAgainOnTheServersTime() async throws {
        let server = Server(), store = Memory(), kept = KeyMemory()
        let device = keys(kept), key = try device.current()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a", deviceBound: true)
        server.answers["/api/auth/refresh"] = [(400, "{\"error\":\"This device's clock is off by more than a minute. Set the time and try again.\",\"device_proof\":\"clock\",\"now_ms\":1800000300000}"),
                                                (200, pair(2))]
        _ = try await make(server, store, device: device).authorize(request())
        XCTAssertEqual(server.count("/api/auth/refresh"), 2)
        XCTAssertTrue(proves(server.requests[0].value(forHTTPHeaderField: "OA-Device-Proof"), key, token: "oa_rt_R1", ms: 1_800_000_000_000))
        XCTAssertTrue(proves(server.requests[1].value(forHTTPHeaderField: "OA-Device-Proof"), key, token: "oa_rt_R1", ms: 1_800_000_300_000))
        XCTAssertEqual(store.tokens?.refresh, "oa_rt_R2")
        // A second clock refusal isn't tried a third time, and the login stays.
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a", deviceBound: true)
        let clock = (400, "{\"error\":\"This device's clock is off by more than a minute. Set the time and try again.\",\"device_proof\":\"clock\",\"now_ms\":1800000300000}")
        server.answers["/api/auth/refresh"] = [clock, clock, (200, pair(3))]
        do { _ = try await make(server, store, device: device).authorize(request()); XCTFail("refused twice") }
        catch { XCTAssertEqual(error.localizedDescription, "This device's clock is off by more than a minute. Set the time and try again.") }
        XCTAssertEqual(server.count("/api/auth/refresh"), 4)
        XCTAssertEqual(store.tokens?.refresh, "oa_rt_R1")
    }

    func testARefusedProofEndsTheLoginAndForgetsTheKey() async throws {
        let server = Server(), store = Memory(), kept = KeyMemory()
        let device = keys(kept); _ = try device.current()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a", deviceBound: true)
        server.answers["/api/auth/refresh"] = [(401, "{\"error\":\"This device was removed from the account. Log in again.\",\"device_proof\":\"removed\"}")]
        do { _ = try await make(server, store, device: device).authorize(request()); XCTFail("removed") }
        catch { XCTAssertEqual(error.localizedDescription, "This device was removed from the account. Log in again.") }
        XCTAssertNil(store.tokens); XCTAssertNil(kept.blob); XCTAssertNil(device.existing())
    }

    func testAnUnboundLoginSendsNoProofAndLogoutForgetsTheKey() async throws {
        let server = Server(), store = Memory(), kept = KeyMemory()
        let device = keys(kept); _ = try device.current()
        store.tokens = .init(access: "oa_at_A1", refresh: "oa_rt_R1", accessUntil: now.addingTimeInterval(-5), account: "acct_a")
        server.answers["/api/auth/refresh"] = [(200, pair(2))]
        server.answers["/api/auth/logout"] = [(200, "{\"state\":\"logged-out\"}")]
        let auth = make(server, store, device: device)
        _ = try await auth.authorize(request())
        XCTAssertNil(server.requests[0].value(forHTTPHeaderField: "OA-Device-Proof"))
        XCTAssertNil(store.tokens?.deviceBound)
        await auth.logout()
        XCTAssertNil(server.requests[1].value(forHTTPHeaderField: "OA-Device-Key"))
        XCTAssertNil(store.tokens); XCTAssertNil(kept.blob)
        // The next login makes a new key.
        let before = try device.current().publicKey
        device.forget()
        XCTAssertNotEqual(try device.current().publicKey, before)
    }

    func testPasskeyLoginSendsTheAssertionAndKeepsThePair() async throws {
        let server = Server(), store = Memory()
        let ch = Data((0..<32).map { UInt8($0) }), chText = AppAuth.b64url(ch)
        server.answers["/api/auth/passkey/login/options"] = [(200, "{\"challenge\":\"\(chText)\",\"rp_id\":\"openagents.com\",\"timeout\":300000}")]
        server.answers["/api/auth/passkey/login"] = [(401, "{\"error\":\"That passkey request has ended. Try again.\"}"),
                                                      (200, "{\"state\":\"logged-in\",\"account\":\"acct_p\",\"access_token\":\"oa_at_P\",\"refresh_token\":\"oa_rt_P\",\"expires_in\":3600}")]
        let auth = make(server, store)
        let c = try await auth.passkeyChallenge()
        XCTAssertEqual(c, AppAuth.PasskeyChallenge(challenge: ch, text: chText, rpID: "openagents.com"))
        XCTAssertEqual(server.asked[0].body, "{\"client\":\"app\"}")
        let a = AppAuth.PasskeyAssertion(credentialID: Data([0xfb, 0xff]), authenticatorData: Data([1]), clientData: Data("{}".utf8), signature: Data([0x30]), userHandle: nil)
        XCTAssertEqual(AppAuth.passkeyBody(c, a), ["challenge": chText, "id": "-_8", "auth_data": "AQ", "client_data": "e30", "signature": "MA", "user_handle": ""])
        do { try await auth.passkeyLogin(c, a); XCTFail("refused") }
        catch { XCTAssertEqual(error.localizedDescription, "That passkey request has ended. Try again.") }
        XCTAssertNil(store.tokens)
        try await auth.passkeyLogin(c, a)
        XCTAssertEqual(store.tokens?.access, "oa_at_P"); XCTAssertEqual(store.tokens?.account, "acct_p")
    }

    func testPasskeyChallengeRefusesABadAnswer() async {
        let server = Server(), store = Memory()
        server.answers["/api/auth/passkey/login/options"] = [(200, "{\"challenge\":\"not+base64url\",\"rp_id\":\"openagents.com\"}")]
        do { _ = try await make(server, store).passkeyChallenge(); XCTFail("bad challenge") }
        catch { XCTAssertEqual(error.localizedDescription, "That didn't work. Try again.") }
        XCTAssertNil(AppAuth.unb64url("")); XCTAssertEqual(AppAuth.unb64url("-_8"), Data([0xfb, 0xff]))
        XCTAssertEqual(PasskeyAsk.words(ASAuthorizationError(.canceled)), "No passkey was used. Try again.")
    }
}
