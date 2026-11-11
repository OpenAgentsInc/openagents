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
        var answers: [String: [(Int, String)]] = [:]
        func send(_ r: URLRequest) async throws -> (Data, Int) {
            let path = r.url?.path ?? ""
            lock.lock(); defer { lock.unlock() }
            asked.append((path, r.value(forHTTPHeaderField: "Authorization") ?? "", String(decoding: r.httpBody ?? Data(), as: UTF8.self)))
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
    func make(_ server: Server, _ store: Memory, at: Date? = nil) -> AppAuth {
        let t = at ?? now
        return AppAuth(origin: "https://example.test", send: { try await server.send($0) }, store: store, clock: { t })
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
}
