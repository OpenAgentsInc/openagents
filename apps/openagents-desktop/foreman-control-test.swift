// Checks of the Foreman control API's credentials, route policy, descriptor file and server.
// Run by foreman-control-test.sh, which compiles this with Foreman.swift and ForemanOperator.swift.
import Foundation

var failures = 0
func check(_ ok: Bool, _ what: String) {
    if ok { print("ok   \(what)") } else { failures += 1; print("FAIL \(what)") }
}

func request(_ base: String, _ method: String, _ path: String, token: String?, header: String = "X-Foreman-Token") -> (Int, [String: Any]) {
    var req = URLRequest(url: URL(string: base + path)!)
    req.httpMethod = method
    req.timeoutInterval = 10
    if let token { req.setValue(token, forHTTPHeaderField: header) }
    if method != "GET" { req.httpBody = Data("{}".utf8); req.setValue("application/json", forHTTPHeaderField: "Content-Type") }
    let sem = DispatchSemaphore(value: 0)
    var out: (Int, [String: Any]) = (0, [:])
    URLSession.shared.dataTask(with: req) { data, resp, _ in
        let body = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] } ?? [:]
        out = ((resp as? HTTPURLResponse)?.statusCode ?? 0, body)
        sem.signal()
    }.resume()
    sem.wait()
    return out
}

func runChecks() {
    // Tokens.
    let a = ForemanAPI.newToken(), b = ForemanAPI.newToken()
    check(a.count == 48 && a != b, "tokens are 48 hex characters and differ")
    check(ForemanAPI.tokensEqual(a, a) && !ForemanAPI.tokensEqual(a, b), "a token equals itself only")
    check(!ForemanAPI.tokensEqual("", a) && !ForemanAPI.tokensEqual(a, "") && !ForemanAPI.tokensEqual("", ""), "an empty token never matches")
    check(!ForemanAPI.tokensEqual(String(a.dropLast()), a) && !ForemanAPI.tokensEqual(a + "0", a), "a prefix or an extension does not match")
    check(ForemanAPI.principal(for: a, foremanToken: a, operatorToken: b) == .foreman, "the worker's token is the foreman principal")
    check(ForemanAPI.principal(for: b, foremanToken: a, operatorToken: b) == .operatorClient, "the operator's token is the operator principal")
    check(ForemanAPI.principal(for: "nope", foremanToken: a, operatorToken: b) == nil, "another token is no principal")

    // Route policy: Foreman cannot manage itself.
    let operatorOnly = [("POST", "/v1/tier"), ("PUT", "/v1/tier"), ("POST", "/tier"), ("GET", "/v1/foreman"), ("POST", "/v1/foreman/open"),
                        ("POST", "/v1/foreman/prompt"), ("POST", "/v1/foreman/stop"), ("POST", "/v1/foreman/reset"), ("POST", "/v1/foreman/wait"),
                        ("POST", "/v1/foreman/not-added-yet")]
    for (m, p) in operatorOnly {
        check(ForemanAPI.authorize(principal: .foreman, method: m, path: p)?.status == 403, "foreman token is refused \(m) \(p)")
        check(ForemanAPI.authorize(principal: .operatorClient, method: m, path: p) == nil, "operator token may \(m) \(p)")
    }
    let shared = [("GET", "/v1"), ("GET", "/v1/status"), ("GET", "/v1/config"), ("GET", "/v1/tier"), ("POST", "/v1/chats/list"),
                  ("POST", "/v1/chats/create"), ("POST", "/v1/wait"), ("POST", "/v1/events"), ("POST", "/v1/search/repo"),
                  ("GET", "/v1/sessions"), ("POST", "/v1/sessions/list"), ("POST", "/v1/sessions/read"),
                  ("POST", "/v1/sessions/send"), ("POST", "/v1/sessions/spawn"), ("POST", "/v1/sessions/stop"), ("POST", "/v1/sessions/wait")]
    for (m, p) in shared {
        check(ForemanAPI.authorize(principal: .foreman, method: m, path: p) == nil && ForemanAPI.authorize(principal: .operatorClient, method: m, path: p) == nil, "both tokens may \(m) \(p)")
    }
    for r in ForemanAPI.routes where r.operatorOnly {
        check(ForemanAPI.isOperatorOnly(method: r.method, path: r.path), "listed operator route \(r.method) \(r.path) is enforced")
    }

    // Tier changes: lowering always, raising only when a person allowed it.
    check(ForemanAPI.tierChange(from: .autonomous, to: .observe, raiseAllowed: false) == nil, "lowering the tier is allowed")
    check(ForemanAPI.tierChange(from: .managed, to: .managed, raiseAllowed: false) == nil, "keeping the tier is allowed")
    check(ForemanAPI.tierChange(from: .observe, to: .managed, raiseAllowed: false)?.status == 403, "raising the tier is refused by default")
    check(ForemanAPI.tierChange(from: .managed, to: .autonomous, raiseAllowed: false)?.status == 403, "raising to autonomous is refused by default")
    check(ForemanAPI.tierChange(from: .observe, to: .autonomous, raiseAllowed: true) == nil, "raising is allowed once a person turned it on")

    // Description.
    let info = ForemanAPI.describe(principal: .operatorClient)
    check(info["api_version"] as? Int == ForemanAPI.version && info["principal"] as? String == "operator", "GET /v1 names the version and the caller")
    check((info["routes"] as? [[String: Any]])?.count == ForemanAPI.routes.count, "GET /v1 lists every route")

    // Descriptor file.
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("foreman-control-test-\(getpid())")
    let file = dir.appendingPathComponent("foreman-control.json")
    check(ForemanControlDescriptor.write(to: file, url: "http://127.0.0.1:1234", token: b, auditLog: "/tmp/audit.jsonl"), "the descriptor is written")
    let attrs = try? FileManager.default.attributesOfItem(atPath: file.path)
    check((attrs?[.posixPermissions] as? NSNumber)?.intValue == 0o600, "the descriptor is readable by its owner only (0600)")
    let read = ForemanControlDescriptor.read(from: file)
    check(read?["url"] as? String == "http://127.0.0.1:1234" && read?["token"] as? String == b && read?["pid"] as? Int == Int(getpid()) && read?["api_version"] as? Int == ForemanAPI.version, "the descriptor holds the URL, operator token, pid and version")
    check(!ForemanControlDescriptor.write(to: file, url: "", token: b, auditLog: ""), "no descriptor is written for a server that did not start")
    let other = dir.appendingPathComponent("other.json")
    try? Data("{\"pid\": 1, \"url\": \"x\", \"token\": \"y\"}".utf8).write(to: other)
    ForemanControlDescriptor.remove(at: other)
    check(FileManager.default.fileExists(atPath: other.path), "another Coder's descriptor is left alone")
    ForemanControlDescriptor.remove(at: file)
    check(!FileManager.default.fileExists(atPath: file.path), "this process's descriptor is removed")

    // Session Bus descriptor (~/.openagents/session-bus.json, mode 0600, #14067).
    let busFile = dir.appendingPathComponent("session-bus.json")
    check(SessionBusDescriptor.write(to: busFile, port: 43210, url: "http://127.0.0.1:43210", token: b, auditLog: "/tmp/audit.jsonl"), "the session-bus descriptor is written")
    let busAttrs = try? FileManager.default.attributesOfItem(atPath: busFile.path)
    check((busAttrs?[.posixPermissions] as? NSNumber)?.intValue == 0o600, "the session-bus descriptor is 0600")
    let busRead = SessionBusDescriptor.read(from: busFile)
    check(busRead?["port"] as? Int == 43210 && busRead?["url"] as? String == "http://127.0.0.1:43210" && busRead?["token"] as? String == b && busRead?["pid"] as? Int == Int(getpid()) && busRead?["bus_mode"] as? String == "unbounded", "the session-bus descriptor holds port, url, token, pid, and unbounded bus_mode")
    SessionBusDescriptor.remove(at: busFile)
    check(!FileManager.default.fileExists(atPath: busFile.path), "this process's session-bus descriptor is removed")

    try? FileManager.default.removeItem(at: dir)

    // The real server, with the app's authorization step in a stand-in handler.
    let server = ForemanControlServer()
    server.handler = { req, done in
        if let denied = ForemanAPI.authorize(principal: req.principal, method: req.method, path: req.path) { done(denied); return }
        done(ForemanResponse(status: 200, body: ["principal": req.principal.rawValue, "path": req.path]))
    }
    check(server.start() && server.port > 0, "the server listens on a loopback port")
    check(server.token != server.operatorToken, "the two credentials differ")
    let base = server.baseURL
    check(request(base, "GET", "/v1/status", token: nil).0 == 401, "a request with no token is refused")
    check(request(base, "GET", "/v1/status", token: "wrong").0 == 401, "a request with a wrong token is refused")
    let asForeman = request(base, "GET", "/v1/status", token: server.token)
    check(asForeman.0 == 200 && asForeman.1["principal"] as? String == "foreman", "the worker's token reaches the handler as foreman")
    let asOperator = request(base, "GET", "/v1/foreman", token: server.operatorToken)
    check(asOperator.0 == 200 && asOperator.1["principal"] as? String == "operator", "the operator's token reaches the handler as operator")
    let viaBusHeader = request(base, "GET", "/v1/sessions", token: server.operatorToken, header: "X-Session-Bus-Token")
    check(viaBusHeader.0 == 200 && viaBusHeader.1["principal"] as? String == "operator", "X-Session-Bus-Token header authenticates")
    let viaBearer = request(base, "GET", "/v1/sessions", token: "Bearer \(server.token)", header: "Authorization")
    check(viaBearer.0 == 200 && viaBearer.1["principal"] as? String == "foreman", "Authorization: Bearer header authenticates")
    check(request(base, "POST", "/v1/sessions/send", token: server.token, header: "X-Session-Bus-Token").0 == 200, "worker token may POST /v1/sessions/send without 403")
    check(request(base, "POST", "/v1/tier", token: server.token).0 == 403, "the worker's token cannot set the tier over the wire")
    check(request(base, "POST", "/v1/foreman/reset", token: server.token).0 == 403, "the worker's token cannot reset Foreman over the wire")
    check(request(base, "POST", "/v1/tier", token: server.operatorToken).0 == 200, "the operator's token reaches the tier route")
    server.stop()
}

@main
enum ForemanControlTest {
    static func main() {
        DispatchQueue.global().async {
            runChecks()
            print(failures == 0 ? "foreman-control-test: all checks passed" : "foreman-control-test: \(failures) failed")
            exit(failures == 0 ? 0 : 1)
        }
        dispatchMain()
    }
}
