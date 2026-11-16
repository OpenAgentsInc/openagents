// Firm reason: 6 (the Swift check of the Swift native services; it needs Foundation's URL loading).
// The Mac's check of a compiled app's native services (app-services-check.sh builds and runs it):
// AppHTTP against a stand-in network, AppAccount against a stand-in server with the tokens in
// memory, AppWake, and AppEffects against a stand-in outbox. Nothing touches the Keychain or a
// real server.
import Foundation

func fail(_ words: String, line: Int = #line) -> Never { FileHandle.standardError.write(Data(("app-services-check: line \(line): " + words + "\n").utf8)); exit(1) }
func expect(_ ok: Bool, _ words: String, line: Int = #line) { if !ok { fail(words, line: line) } }

/// The network: answers each request from `answer`, and records what it was asked.
final class Network: URLProtocol {
    struct Reply { var status = 200; var headers: [String: String] = ["Content-Type": "application/json"]; var parts: [Data] = []; var error: URLError.Code?; var hang = false }
    nonisolated(unsafe) static var answer: (URLRequest, Data) -> Reply = { _, _ in Reply() }
    nonisolated(unsafe) static var seen: [(URLRequest, Data)] = []
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}
    override func startLoading() {
        var body = Data()
        if let s = request.httpBodyStream {
            s.open(); var buffer = [UInt8](repeating: 0, count: 4096)
            while s.hasBytesAvailable { let n = s.read(&buffer, maxLength: buffer.count); if n <= 0 { break }; body.append(buffer, count: n) }
            s.close()
        } else if let b = request.httpBody { body = b }
        let request = self.request, client = self.client, me = self
        DispatchQueue.main.async {
            Network.seen.append((request, body))
            let reply = Network.answer(request, body)
            if reply.hang { return }
            if let code = reply.error { client?.urlProtocol(me, didFailWithError: URLError(code)); return }
            let response = HTTPURLResponse(url: request.url!, statusCode: reply.status, httpVersion: "HTTP/1.1", headerFields: reply.headers)!
            if (300..<400).contains(reply.status), let to = reply.headers["Location"], let url = URL(string: to) {
                // A refused redirect leaves the 3xx as the answer, as the real loader does.
                client?.urlProtocol(me, wasRedirectedTo: URLRequest(url: url), redirectResponse: response)
            }
            client?.urlProtocol(me, didReceive: response, cacheStoragePolicy: .notAllowed)
            for part in reply.parts { client?.urlProtocol(me, didLoad: part) }
            client?.urlProtocolDidFinishLoading(me)
        }
    }
}

final class MemoryTokens: TokenStore, @unchecked Sendable {
    var held: AppAuth.Tokens?
    func load() -> AppAuth.Tokens? { held }
    func save(_ tokens: AppAuth.Tokens) { held = tokens }
    func clear() { held = nil }
}

@MainActor final class Outbox: AppOutbox {
    var waiting: [AppRequest] = []
    var got: [AppCompletion] = []
    var draining = false, completing = false
    /// What a completion makes the app ask next.
    var react: (AppCompletion) -> [AppRequest] = { _ in [] }
    func drain() -> [AppRequest] {
        expect(!completing, "the outbox was drained inside a completion")
        draining = true; defer { draining = false; waiting = [] }
        return waiting
    }
    func complete(_ completion: AppCompletion) {
        expect(!draining && !completing, "a completion arrived inside a dispatch")
        completing = true; got.append(completion); waiting += react(completion); completing = false
    }
}
/// A stand-in for the shared app core's host ABI: queued requests, the results it heard.
@MainActor final class FakeCore: AppCoreABI {
    struct Queued { var op: Int64; var fields: [Int64]; var path = ""; var text = ""; var body = "" }
    struct Done: Equatable { var op: Int64; var generation: Int64; var outcome: Int32; var status: Int32; var flag: Bool; var text: String; var more: String; var value: Int64 }
    var queued: [Queued] = [], cancels: [Int64] = [], taken: [(Int64, Bool)] = [], done: [Done] = [], lifecycles: [Int32] = []
    var wakeAt: Int64 = -1, clocks = 0
    func find(_ op: Int64) -> Queued? { queued.first { $0.op == op } }
    func next() -> Int64 { queued.first?.op ?? 0 }
    func request(_ op: Int64) -> [Int64]? { find(op)?.fields }
    func path(_ op: Int64) -> Data? { find(op).map { Data($0.path.utf8) } }
    func text(_ op: Int64) -> Data? { find(op).map { Data($0.text.utf8) } }
    func body(_ op: Int64) -> Data? { find(op).map { Data($0.body.utf8) } }
    func taken(_ op: Int64, accepted: Bool) { taken.append((op, accepted)); queued.removeAll { $0.op == op } }
    func cancelNext() -> Int64 { cancels.isEmpty ? 0 : cancels.removeFirst() }
    func done(op: Int64, generation: Int64, outcome: Int32, status: Int32, flag: Bool, text: Data, more: Data, value: Int64) {
        done.append(.init(op: op, generation: generation, outcome: outcome, status: status, flag: flag, text: String(decoding: text, as: UTF8.self), more: String(decoding: more, as: UTF8.self), value: value))
    }
    func wake() -> Int64 { wakeAt }
    func clock() { clocks += 1; wakeAt = -1 }
    func lifecycle(_ code: Int32) { lifecycles.append(code) }
    /// A request in APP_HOST_ field order: kind, generation, method, limit, timeout, owner.
    static func fields(_ kind: Int64, _ generation: Int64, method: Int64 = 0, limit: Int64 = 0, timeout: Int64 = 0) -> [Int64] { [kind, generation, method, limit, timeout, 1] }
}

@MainActor final class Platform: AppPlatform {
    var copied: [String] = [], opened: [String] = []
    func copy(_ text: String) -> Bool { copied.append(text); return true }
    func open(_ link: String, done: @escaping (Bool) -> Void) { opened.append(link); done(link.hasPrefix("https://")) }
}

@MainActor func settle(_ what: String, seconds: Double = 5, _ until: () -> Bool) async {
    let end = Date().addingTimeInterval(seconds)
    while !until() { if Date() > end { fail("timed out waiting for " + what) }; try? await Task.sleep(nanoseconds: 2_000_000) }
}
@MainActor func quiet(_ ms: UInt64 = 60) async { try? await Task.sleep(nanoseconds: ms * 1_000_000) }

@main enum AppServicesCheck {
    static let origin = "https://openagents.test"
    @MainActor static func main() async {
        await http(); await account(); await wake(); await effects(); await bridge(); focus()
        print("app-services-check: ok")
    }

    /// The shared core's requests reach the services, and their results come back as its fields.
    @MainActor static func bridge() async {
        let store = MemoryTokens()
        let auth = AppAuth(origin: origin, send: { r in
            r.url!.path == "/api/auth/email/start" ? (Data("{\"pending\":\"pend_secret\"}".utf8), 200)
                : (Data("{\"error\":\"That code has expired.\"}".utf8), 400)
        }, store: store, device: nil)
        let account = AppAccount(auth: auth)
        let http = AppHTTP(policy: .init(origin: origin, admit: { _, path, _ in path.hasPrefix("/api/chat/") }), authorize: AppAccount.authorizer(auth), protocols: [Network.self])
        let core = FakeCore(), bridge = AppCoreBridge(core: core), platform = Platform()
        let e = AppEffects(outbox: bridge, http: http, account: account, wake: AppWake(), platform: platform)
        bridge.effects = e
        Network.seen = []; Network.answer = { _, body in .init(parts: [Data("{\"echo\":".utf8), body, Data("}".utf8)]) }
        core.queued = [.init(op: 1, fields: FakeCore.fields(1, 3, method: 2, limit: 4096, timeout: 5000), path: "/api/chat/submit/v2alpha1", body: "{\"a\":1}"),
                       .init(op: 2, fields: FakeCore.fields(2, 3)), .init(op: 3, fields: FakeCore.fields(6, 3)), .init(op: 4, fields: FakeCore.fields(7, 3), body: "copied"),
                       .init(op: 5, fields: FakeCore.fields(99, 3)), .init(op: 6, fields: FakeCore.fields(1, 3, method: 7), path: "/api/chat/x"),
                       .init(op: 7, fields: FakeCore.fields(3, 3), text: "a@b.test")]
        bridge.heard()
        expect(core.taken.map(\.0) == [1, 2, 3, 4, 5, 6, 7] && core.taken.map(\.1) == [true, true, true, true, true, false, true], "taken and refused: \(core.taken)")
        expect(core.done.isEmpty, "a result arrived inside the call")
        await settle("the bridge's results") { core.done.count == 6 }
        let by = Dictionary(uniqueKeysWithValues: core.done.map { ($0.op, $0) })
        expect(by[1] == .init(op: 1, generation: 3, outcome: 1, status: 200, flag: false, text: "{\"echo\":{\"a\":1}}", more: "", value: 0), "the HTTP result: \(String(describing: by[1]))")
        expect(Network.seen.first?.0.httpMethod == "POST" && Network.seen.first?.0.timeoutInterval == 5, "the method and the request's own timeout")
        expect(by[2]?.outcome == 1 && by[2]?.flag == false, "the account state")
        expect(by[3]?.text.count == 32 && by[3]?.more.count == 32 && by[3]?.text != by[3]?.more && (by[3]?.value ?? 0) > 0, "the identities")
        expect(by[4]?.outcome == 1 && platform.copied == ["copied"], "the clipboard")
        expect(by[5]?.outcome == 5, "an unknown kind is unsupported")
        expect(by[7]?.outcome == 1 && by[7]?.flag == true, "the email step")
        expect(!core.done.contains { $0.text.contains("pend_secret") || $0.more.contains("pend_secret") }, "the email attempt crossed into the core")

        // A wrong code: the server's words come back as the result's text.
        core.done = []; core.queued = [.init(op: 8, fields: FakeCore.fields(4, 3), text: "000000")]
        bridge.heard()
        await settle("the code step") { core.done.count == 1 }
        expect(core.done[0].outcome == 2 && !core.done[0].flag && !core.done[0].text.isEmpty, "a refused code: \(core.done[0])")

        // A cancellation the core asks for stops the request, which completes as cancelled.
        core.done = []; Network.answer = { _, _ in .init(hang: true) }
        core.queued = [.init(op: 9, fields: FakeCore.fields(1, 4, method: 1), path: "/api/chat/state")]
        bridge.heard()
        core.cancels = [9]; bridge.heard()
        await settle("the cancelled request") { core.done.count == 1 }
        await quiet(60)
        expect(core.done == [.init(op: 9, generation: 4, outcome: 4, status: 0, flag: false, text: "", more: "", value: 0)] && http.active == 0, "the cancellation: \(core.done)")

        // The core's wake becomes a timer, and then the clock; lifecycle facts go in.
        var now: Int64 = 1000; bridge.now = { now }
        core.wakeAt = 1030; bridge.heard()
        await settle("the wake") { core.clocks == 1 }
        now = 2000; core.wakeAt = -1; bridge.lifecycle(foreground: false); bridge.lifecycle(foreground: true)
        expect(core.lifecycles == [2, 1], "the lifecycle codes")
        bridge.retire(); e.retire()
        print("ok bridge")
    }

    /// A tap outside the focused field ends editing; one on it, or with no field focused, does not.
    static func focus() {
        let field = CGRect(x: 16, y: 700, width: 370, height: 44)
        expect(KitFocus.tapEndsEditing(at: CGPoint(x: 200, y: 300), field: field), "a tap above the field")
        expect(KitFocus.tapEndsEditing(at: CGPoint(x: 395, y: 720), field: field), "a tap on a control beside the field")
        expect(!KitFocus.tapEndsEditing(at: CGPoint(x: 200, y: 720), field: field), "a tap in the field")
        expect(!KitFocus.tapEndsEditing(at: CGPoint(x: 16, y: 744), field: field), "a tap on the field's edge")
        expect(!KitFocus.tapEndsEditing(at: CGPoint(x: 200, y: 300), field: nil), "a tap with no field focused")
        expect(!KitFocus.tapEndsEditing(at: CGPoint(x: 200, y: 300), field: .null), "a tap with an unplaced field")
        print("ok focus")
    }

    @MainActor static func http() async {
        // A path names a place on the origin and nothing else.
        expect(AppHTTP.url(origin: origin, path: "/api/chat/x?y=1")?.absoluteString == origin + "/api/chat/x?y=1", "a plain path")
        for bad in ["", "api", "//evil.test/x", "@evil.test/x", ".evil.test/x", ":80@evil.test/", "/\\evil.test"] { expect(AppHTTP.url(origin: origin, path: bad) == nil, "admitted " + bad) }
        expect(AppHTTP.neverSent(URLError(.cannotConnectToHost)) && AppHTTP.neverSent(URLError(.notConnectedToInternet)), "offline errors mean never sent")
        expect(!AppHTTP.neverSent(URLError(.timedOut)) && !AppHTTP.neverSent(URLError(.networkConnectionLost)), "a timeout may have been sent")

        nonisolated(unsafe) var token: String? = "oa_at_check"
        var admitted: [String] = []
        let policy = AppHTTP.Policy(origin: origin, admit: { method, path, bytes in admitted.append(method + " " + path); return path.hasPrefix("/api/chat/") && bytes <= 64 }, maxResponse: 100, timeout: 5)
        let h = AppHTTP(policy: policy, authorize: { r in
            guard let token else { return nil }
            if token == "ended" { throw AppHTTP.Blocked.login }
            if token == "offline" { throw AppHTTP.Blocked.offline }
            var with = r; with.setValue("Bearer " + token, forHTTPHeaderField: "Authorization"); return with
        }, protocols: [Network.self])
        var got: [AppHTTP.Outcome] = []
        func ask(_ op: UInt64, _ r: AppHTTP.Request, generation: UInt64 = 1) { h.start(op: op, generation: generation, r) { got.append($0) } }

        // A GET: the session attached, no cookie, the body and status answered, later.
        Network.answer = { _, _ in .init(status: 200, headers: ["Content-Type": "application/json", "Set-Cookie": "sid=1; Path=/"], parts: [Data("{\"ok\":true}".utf8)]) }
        ask(1, .init(path: "/api/chat/state"))
        expect(got.isEmpty, "an answer arrived inside start")
        await settle("the GET") { got.count == 1 }
        expect(got[0] == .answer(status: 200, body: Data("{\"ok\":true}".utf8), contentType: "application/json"), "the GET's answer: \(got[0])")
        var sent = Network.seen[0].0
        expect(sent.value(forHTTPHeaderField: "Authorization") == "Bearer oa_at_check" && sent.url?.host == "openagents.test" && sent.httpMethod == "GET", "the GET as sent")
        expect(sent.value(forHTTPHeaderField: "Origin") == nil && sent.value(forHTTPHeaderField: "Content-Type") == nil, "a GET has no Origin or Content-Type")

        // A POST: its body, Content-Type and Origin; the cookie the server set is not sent back.
        got = []; Network.seen = []
        ask(2, .init(method: "POST", path: "/api/chat/send", body: Data("{\"text\":\"hi\"}".utf8)))
        await settle("the POST") { got.count == 1 }
        sent = Network.seen[0].0
        expect(Network.seen[0].1 == Data("{\"text\":\"hi\"}".utf8) && sent.value(forHTTPHeaderField: "Content-Type") == "application/json" && sent.value(forHTTPHeaderField: "Origin") == origin, "the POST as sent")
        expect(sent.value(forHTTPHeaderField: "Cookie") == nil, "a cookie was sent back")
        expect(Set(sent.allHTTPHeaderFields?.keys.map { $0.lowercased() } ?? []).isSubset(of: ["accept", "content-type", "origin", "authorization", "content-length"]), "an extra header was sent: \(sent.allHTTPHeaderFields ?? [:])")

        // Refused before anything is sent: the policy, the method, the body limit, a live op.
        got = []; Network.seen = []
        ask(3, .init(path: "/api/auth/account")); ask(4, .init(method: "DELETE", path: "/api/chat/x")); ask(5, .init(method: "POST", path: "/api/chat/send", body: Data(count: 65)))
        ask(6, .init(path: "//evil.test/api/chat/x"))
        await settle("the refusals") { got.count == 4 }
        await quiet()
        expect(got.allSatisfy { $0 == .failed(.refused) } && Network.seen.isEmpty, "a refused request reached the network")
        expect(admitted.contains("GET /api/auth/account"), "the policy was not asked")

        // Nobody logged in: the request goes without a session. A login that ended: nothing is sent.
        got = []; token = nil
        ask(7, .init(path: "/api/chat/public"))
        await settle("the anonymous GET") { got.count == 1 }
        expect(Network.seen.last?.0.value(forHTTPHeaderField: "Authorization") == nil, "a session was attached with nobody logged in")
        got = []; Network.seen = []; token = "ended"; ask(8, .init(path: "/api/chat/state"))
        await settle("the ended login") { got.count == 1 }
        token = "offline"; ask(9, .init(path: "/api/chat/state"))
        await settle("the unreachable refresh") { got.count == 2 }
        expect(got == [.failed(.login), .failed(.offline)] && Network.seen.isEmpty, "an ended login: \(got)")
        token = "oa_at_check"
        // account: false sends no session even when someone is logged in.
        got = []; ask(10, .init(path: "/api/chat/public", account: false))
        await settle("the plain GET") { got.count == 1 }
        expect(Network.seen.last?.0.value(forHTTPHeaderField: "Authorization") == nil, "account: false attached the session")

        // A redirect is not followed: the 3xx is the answer and the other host is never asked.
        got = []; Network.seen = []
        Network.answer = { r, _ in r.url?.host == "openagents.test" ? .init(status: 302, headers: ["Location": "https://evil.test/take"]) : .init(status: 200, parts: [Data("stolen".utf8)]) }
        ask(11, .init(path: "/api/chat/state"))
        await settle("the redirect") { got.count == 1 }
        await quiet()
        if case .answer(let status, _, _) = got[0] { expect(status == 302, "the redirect's status \(status)") } else { fail("the redirect: \(got[0])") }
        expect(Network.seen.allSatisfy { $0.0.url?.host == "openagents.test" }, "the redirect was followed")

        // An answer over the limit ends as unknown; transport errors say whether it was sent.
        got = []
        Network.answer = { _, _ in .init(parts: [Data(count: 60), Data(count: 60)]) }
        ask(12, .init(path: "/api/chat/state"))
        await settle("the large answer") { got.count == 1 }
        expect(got[0] == .unknown(.tooLarge), "the large answer: \(got[0])")
        got = []; Network.answer = { _, _ in .init(error: .cannotConnectToHost) }; ask(13, .init(path: "/api/chat/state"))
        await settle("offline") { got.count == 1 }
        Network.answer = { _, _ in .init(error: .networkConnectionLost) }; ask(14, .init(method: "POST", path: "/api/chat/send"))
        await settle("a lost connection") { got.count == 2 }
        expect(got == [.failed(.offline), .unknown(.lost)], "transport errors: \(got)")

        // A stream answers its parts in order, then its end. An error's body is not streamed.
        got = []
        Network.answer = { _, _ in .init(headers: ["Content-Type": "text/event-stream"], parts: [Data("a".utf8), Data("b".utf8)]) }
        ask(15, .init(path: "/api/chat/stream", stream: true))
        await settle("the stream") { got.last?.final == true }
        let parts = got.compactMap { o -> Data? in if case .part(let d) = o { return d }; return nil }.reduce(Data(), +)
        expect(parts == Data("ab".utf8) && got.last == .answer(status: 200, body: Data(), contentType: "text/event-stream"), "the stream: \(got)")
        expect(Network.seen.last?.0.value(forHTTPHeaderField: "Accept") == "text/event-stream", "a stream's Accept")
        got = []; Network.answer = { _, _ in .init(status: 503, parts: [Data("{\"error\":\"busy\"}".utf8)]) }
        ask(16, .init(path: "/api/chat/stream", stream: true))
        await settle("the failed stream") { got.count == 1 }
        expect(got[0] == .answer(status: 503, body: Data("{\"error\":\"busy\"}".utf8), contentType: "application/json"), "a failed stream: \(got[0])")

        // Cancel by op and by generation: neither answers, and a later op with the same number works.
        got = []; Network.answer = { _, _ in .init(hang: true) }
        ask(20, .init(path: "/api/chat/state"), generation: 7); ask(21, .init(path: "/api/chat/state"), generation: 7); ask(22, .init(path: "/api/chat/state"), generation: 8)
        await quiet()
        expect(h.active == 3, "three requests are waiting")
        h.cancel(op: 22); h.cancel(generation: 7)
        await quiet(120)
        expect(h.active == 0 && got.isEmpty, "a cancelled request answered: \(got)")
        Network.answer = { _, _ in .init(parts: [Data("{}".utf8)]) }
        ask(20, .init(path: "/api/chat/state"))
        await settle("the reused op") { got.count == 1 }
        print("ok http")
    }

    @MainActor static func account() async {
        let store = MemoryTokens()
        nonisolated(unsafe) var asked: [(String, [String: Any], String?)] = []
        nonisolated(unsafe) var refuse = false
        let auth = AppAuth(origin: origin, send: { r in
            let body = (try? JSONSerialization.jsonObject(with: r.httpBody ?? Data())) as? [String: Any] ?? [:]
            let path = r.url!.path
            asked.append((path, body, r.value(forHTTPHeaderField: "Authorization")))
            switch path {
            case "/api/auth/email/start": return (Data("{\"pending\":\"pend_secret\"}".utf8), 200)
            case "/api/auth/email/verify":
                if body["code"] as? String != "123456" { return (Data("{\"error\":\"That code is wrong.\"}".utf8), 400) }
                return (Data("{\"access_token\":\"oa_at_one\",\"refresh_token\":\"oa_rt_one\",\"account\":\"acct_1\",\"expires_in\":3600}".utf8), 200)
            case "/api/auth/refresh":
                if refuse { return (Data("{\"error\":\"Log in again.\"}".utf8), 401) }
                return (Data("{\"access_token\":\"oa_at_two\",\"refresh_token\":\"oa_rt_two\",\"expires_in\":3600}".utf8), 200)
            default: return (Data("{}".utf8), 200)
            }
        }, store: store, device: nil)
        let a = AppAccount(auth: auth)
        var changes = 0; a.changed = { changes += 1 }
        expect(!a.loggedIn, "starts logged out")
        expect(await a.emailVerify("123456") == .said("Ask for a new code."), "a code with no attempt")
        expect(await a.emailStart(" ada@example.com ") == .done, "the email start")
        expect(asked[0].1["email"] as? String == "ada@example.com", "the email is trimmed")
        expect(await a.emailVerify("000000") == .said("That code is wrong."), "the server's words are shown as given")
        expect(await a.emailVerify("123456") == .done && a.loggedIn, "the code logs in")
        expect(asked.last?.1["pending"] as? String == "pend_secret", "the pending handle goes back with the code")
        await settle("the login notice") { changes == 1 }
        // The session for a request, and none of it anywhere else.
        let authorize = AppAccount.authorizer(auth)
        let with = try? await authorize(URLRequest(url: URL(string: origin + "/api/chat/state")!))
        expect(with?.value(forHTTPHeaderField: "Authorization") == "Bearer oa_at_one", "the session is attached")
        // A stale access token is renewed; a refresh the server refuses ends the login.
        store.held?.accessUntil = Date().addingTimeInterval(-10)
        expect(await a.refresh() && store.held?.access == "oa_at_two", "a stale token is renewed")
        store.held?.accessUntil = Date().addingTimeInterval(-10); refuse = true
        do { _ = try await authorize(URLRequest(url: URL(string: origin + "/api/chat/state")!)); fail("an ended login authorized a request") }
        catch { expect((error as? AppHTTP.Blocked) == .login, "an ended login: \(error)") }
        expect(!a.loggedIn && store.held == nil, "the ended login is forgotten")
        await settle("the logout notice") { changes == 2 }
        let none = try? await authorize(URLRequest(url: URL(string: origin)!)); expect(none == nil, "nobody logged in: no session")
        // Log out forgets the tokens and tells the server.
        refuse = false
        expect(await a.emailStart("ada@example.com") == .done, "a second start"); expect(await a.emailVerify("123456") == .done, "a second login")
        await a.logout()
        expect(!a.loggedIn && store.held == nil && asked.last?.0 == "/api/auth/logout" && asked.last?.2 == "Bearer oa_at_one", "log out")
        expect(await a.emailVerify("123456") == .said("Ask for a new code."), "log out forgets the attempt")
        print("ok account")
    }

    @MainActor static func wake() async {
        let w = AppWake()
        var fired: [Int] = []
        let start = AppWake.steadyMs()
        w.after(op: 1, generation: 1, milliseconds: 40) { fired.append(1) }
        w.after(op: 2, generation: 1, milliseconds: 20) { fired.append(2) }
        w.after(op: 3, generation: 2, milliseconds: 20) { fired.append(3) }
        w.after(op: 4, generation: 3, milliseconds: 20) { fired.append(4) }
        expect(fired.isEmpty && w.waiting == 4, "a wake fired at once")
        w.cancel(op: 2); w.cancel(generation: 2)
        await settle("the wakes") { fired.count == 2 }
        await quiet()
        expect(fired == [4, 1] && w.waiting == 0, "the wakes: \(fired)")
        expect(AppWake.steadyMs() - start >= 39, "the wake came early")
        // A second wake with a live op replaces the first.
        fired = []; w.after(op: 5, generation: 1, milliseconds: 20) { fired.append(5) }; w.after(op: 5, generation: 1, milliseconds: 30) { fired.append(6) }
        await settle("the replaced wake") { !fired.isEmpty }
        await quiet()
        expect(fired == [6], "a replaced wake: \(fired)")
        let a = AppWake.randomHex(16), b = AppWake.randomHex(16)
        expect(a?.count == 32 && a != b && a!.allSatisfy { $0.isHexDigit }, "random identities")
        expect(abs(AppWake.wallMs() - Int64(Date().timeIntervalSince1970 * 1000)) < 1000, "the wall clock")
        print("ok wake")
    }

    @MainActor static func effects() async {
        let store = MemoryTokens()
        let auth = AppAuth(origin: origin, send: { r in
            r.url!.path == "/api/auth/email/start" ? (Data("{\"pending\":\"pend_secret\"}".utf8), 200)
                : (Data("{\"access_token\":\"oa_at_one\",\"refresh_token\":\"oa_rt_one\",\"account\":\"acct_1\",\"expires_in\":3600}".utf8), 200)
        }, store: store, device: nil)
        let account = AppAccount(auth: auth)
        let http = AppHTTP(policy: .init(origin: origin, admit: { _, path, _ in path.hasPrefix("/api/chat/") }), authorize: AppAccount.authorizer(auth), protocols: [Network.self])
        let outbox = Outbox(), platform = Platform(), wake = AppWake()
        let e = AppEffects(outbox: outbox, http: http, account: account, wake: wake, platform: platform)
        var changes = 0; e.changed = { changes += 1 }
        func text(_ c: AppCompletion) -> String { String(decoding: c.body, as: UTF8.self) }

        // Services that answer at once still complete in a later turn, in order.
        outbox.waiting = [.init(op: 1, generation: 1, call: .authState), .init(op: 2, generation: 1, call: .identities), .init(op: 3, generation: 1, call: .clipboard("copied text")),
                          .init(op: 4, generation: 1, call: .link("https://openagents.com/docs")), .init(op: 5, generation: 1, call: .link("javascript:alert(1)")), .init(op: 6, generation: 1, call: .unsupported)]
        e.pump()
        expect(outbox.got.isEmpty, "a completion arrived inside pump")
        await settle("the first completions") { outbox.got.count == 6 }
        expect(outbox.got.map(\.op) == [1, 2, 3, 4, 5, 6] && changes == 6, "the order: \(outbox.got.map(\.op))")
        expect(outbox.got.map(\.state) == [.done, .done, .done, .done, .failed, .unsupported], "the states: \(outbox.got.map(\.state))")
        expect(text(outbox.got[0]) == "{\"loggedIn\":false}", "the state: " + text(outbox.got[0]))
        let ids = (try? JSONSerialization.jsonObject(with: outbox.got[1].body)) as? [String: Any] ?? [:]
        expect((ids["command_id"] as? String)?.count == 32 && (ids["session_id"] as? String)?.count == 32 && ids["command_id"] as? String != ids["session_id"] as? String && (ids["now_ms"] as? NSNumber)?.int64Value ?? 0 > 0, "the identities")
        expect(platform.copied == ["copied text"] && platform.opened.count == 2, "the platform's effects")

        // A login: the email, then the code. No token or handle is in any completion.
        outbox.got = []; outbox.waiting = [.init(op: 10, generation: 2, call: .authEmailStart("ada@example.com"))]
        outbox.react = { c in c.op == 10 ? [.init(op: 11, generation: 2, call: .authEmailVerify("123456"))] : [] }
        e.pump()
        await settle("the login") { outbox.got.count == 2 }
        expect(outbox.got.map(\.state) == [.done, .done] && text(outbox.got[1]) == "{\"loggedIn\":true}" && account.loggedIn, "the login's completions")
        expect(!outbox.got.contains { text($0).contains("pend_secret") || text($0).contains("oa_at_") || text($0).contains("oa_rt_") }, "a secret entered a completion")

        // A request made by a completion is performed (the outbox is pumped after each one), with the session.
        outbox.got = []; Network.seen = []
        Network.answer = { _, _ in .init(parts: [Data("{\"state\":\"running\"}".utf8)]) }
        outbox.waiting = [.init(op: 20, generation: 3, call: .wake(milliseconds: 10))]
        outbox.react = { c in c.op == 20 ? [.init(op: 21, generation: 3, call: .http(.init(path: "/api/chat/state")))] : [] }
        e.pump()
        await settle("the wake then the request") { outbox.got.count == 2 }
        expect(outbox.got[1] == AppCompletion(op: 21, generation: 3, state: .done, status: 200, body: Data("{\"state\":\"running\"}".utf8), contentType: "application/json"), "the request's completion: \(outbox.got[1])")
        expect(Network.seen.last?.0.value(forHTTPHeaderField: "Authorization") == "Bearer oa_at_one", "the session on the request")

        // Failed and unknown carry the reason; a refused path never reaches the network.
        outbox.got = []; outbox.react = { _ in [] }; Network.seen = []
        Network.answer = { _, _ in .init(error: .timedOut) }
        outbox.waiting = [.init(op: 30, generation: 3, call: .http(.init(path: "/api/auth/account"))), .init(op: 31, generation: 3, call: .http(.init(method: "POST", path: "/api/chat/send")))]
        e.pump()
        await settle("failed and unknown") { outbox.got.count == 2 }
        expect(outbox.got[0].state == .failed && outbox.got[0].status == AppHTTP.Reason.refused.rawValue && outbox.got[1].state == .unknown && outbox.got[1].status == AppHTTP.Reason.lost.rawValue && Network.seen.count == 1, "failed and unknown: \(outbox.got)")

        // A cancelled request completes once as cancelled, and its own answer never arrives.
        outbox.got = []; Network.answer = { _, _ in .init(hang: true) }
        outbox.waiting = [.init(op: 40, generation: 4, call: .http(.init(path: "/api/chat/state"))), .init(op: 41, generation: 4, call: .wake(milliseconds: 30)),
                          .init(op: 42, generation: 5, call: .wake(milliseconds: 30)), .init(op: 43, generation: 6, call: .wake(milliseconds: 30))]
        e.pump()
        outbox.waiting = [.init(op: 44, generation: 6, call: .cancelGeneration(4)), .init(op: 45, generation: 6, call: .cancel(op: 42)), .init(op: 46, generation: 6, call: .cancel(op: 999))]
        e.pump()
        await settle("the surviving wake") { outbox.got.count == 4 }
        await quiet(100)
        expect(Set(outbox.got.prefix(3).map(\.op)) == [40, 41, 42] && outbox.got.prefix(3).allSatisfy { $0.state == .cancelled } && outbox.got.prefix(3).map(\.generation).sorted() == [4, 4, 5], "the cancelled completions: \(outbox.got)")
        expect(outbox.got.count == 4 && outbox.got[3].op == 43 && outbox.got[3].state == .done && http.active == 0 && wake.waiting == 0, "after the cancels: \(outbox.got.map(\.op))")

        // Log out through the outbox; retire drops what is in flight.
        outbox.got = []; outbox.waiting = [.init(op: 50, generation: 6, call: .authLogout)]
        e.pump()
        await settle("log out") { outbox.got.count == 1 }
        expect(text(outbox.got[0]) == "{\"loggedIn\":false}" && !account.loggedIn && store.held == nil, "log out")
        outbox.got = []; outbox.waiting = [.init(op: 60, generation: 7, call: .wake(milliseconds: 20)), .init(op: 61, generation: 7, call: .authState)]
        e.pump(); e.retire()
        await quiet(100)
        expect(outbox.got.isEmpty, "a completion arrived after retire")
        print("ok effects")
    }
}
