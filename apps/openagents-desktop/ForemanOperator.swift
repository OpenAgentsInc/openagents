// Foreman operator API: what a program outside the Foreman chat needs to manage Foreman.
// Two credentials reach the control server (Foreman.swift). The Foreman worker holds one and acts
// under its permission tier. A script, a test or another agent holds the operator one, which it
// reads from the descriptor file this app writes at launch. Routes marked operator-only refuse the
// worker's credential, so Foreman cannot change its own tier, prompt itself or reset itself.
// The reference is docs/coder/foreman-control-api.md.
import Darwin
import Foundation

/// Who sent a request, decided by which token it carried.
enum ForemanPrincipal: String {
    case foreman
    case operatorClient = "operator"
}

/// One route of the control API, as GET /v1 lists it.
struct ForemanRoute {
    let method: String
    let path: String
    let operatorOnly: Bool
    let summary: String

    func asDictionary() -> [String: Any] {
        ["method": method, "path": path, "principal": operatorOnly ? "operator" : "any", "summary": summary]
    }
}

enum ForemanAPI {
    /// Goes up when a route or a field changes in a way a client has to know about.
    static let version = 1
    /// UserDefaults key: a person allowed programs to raise the tier (the Foreman row's menu). Off unless set.
    static let operatorTierRaiseKey = "foremanOperatorTierRaise"

    static let routes: [ForemanRoute] = [
        ForemanRoute(method: "GET", path: "/v1", operatorOnly: false, summary: "API version, the caller's principal and this route table"),
        ForemanRoute(method: "GET", path: "/v1/status", operatorOnly: false, summary: "Tier, workspace, chat and run counts, event cursor"),
        ForemanRoute(method: "GET", path: "/v1/config", operatorOnly: false, summary: "Tier and whether a program may raise it"),
        ForemanRoute(method: "GET", path: "/v1/tier", operatorOnly: false, summary: "The permission tier"),
        ForemanRoute(method: "POST", path: "/v1/tier", operatorOnly: true, summary: "Set the tier: {tier: observe|managed|autonomous}"),
        ForemanRoute(method: "GET", path: "/v1/foreman", operatorOnly: true, summary: "The Foreman chat: shown, busy, history, managed chats"),
        ForemanRoute(method: "POST", path: "/v1/foreman/open", operatorOnly: true, summary: "Show the Foreman chat in the window"),
        ForemanRoute(method: "POST", path: "/v1/foreman/prompt", operatorOnly: true, summary: "Send Foreman a message: {prompt, steer?}"),
        ForemanRoute(method: "POST", path: "/v1/foreman/stop", operatorOnly: true, summary: "Stop Foreman's own reply; managed chats keep running"),
        ForemanRoute(method: "POST", path: "/v1/foreman/reset", operatorOnly: true, summary: "Clear Foreman's history; managed chats are kept"),
        ForemanRoute(method: "POST", path: "/v1/foreman/wait", operatorOnly: true, summary: "Wait for Foreman's reply to end: {timeout_seconds?}"),
        ForemanRoute(method: "POST", path: "/v1/events", operatorOnly: false, summary: "Lifecycle events: {since_id?, chat_id?, limit?}"),
        ForemanRoute(method: "POST", path: "/v1/audit", operatorOnly: false, summary: "The audit log's tail: {limit?}"),
        ForemanRoute(method: "POST", path: "/v1/wait", operatorOnly: false, summary: "Wait for chats to end: {chat_ids?, mode?, timeout_seconds?}"),
        ForemanRoute(method: "POST", path: "/v1/chats/list", operatorOnly: false, summary: "List chats: {filter?, managed_only?, folder?, limit?}"),
        ForemanRoute(method: "POST", path: "/v1/chats/read", operatorOnly: false, summary: "Read a chat: {chat_id, tail?, include_tools?}"),
        ForemanRoute(method: "POST", path: "/v1/chats/create", operatorOnly: false, summary: "Start a managed chat: {prompt, title?, folder?, budget_usd?, goal?, pin?}"),
        ForemanRoute(method: "POST", path: "/v1/chats/send", operatorOnly: false, summary: "Message a chat: {chat_id, prompt, mode?}"),
        ForemanRoute(method: "POST", path: "/v1/chats/control", operatorOnly: false, summary: "Stop, rename, pin, delete, budget or goal: {chat_id, action, ...}"),
        ForemanRoute(method: "POST", path: "/v1/chats/stop-all", operatorOnly: false, summary: "Stop running managed chats: {all?}"),
        ForemanRoute(method: "POST", path: "/v1/search/chats", operatorOnly: false, summary: "Search kept chats: {query, limit?, folder?, date_from?, date_to?}"),
        ForemanRoute(method: "POST", path: "/v1/search/repo", operatorOnly: false, summary: "Search the repository: {query, folder?, glob?, max?}"),
        ForemanRoute(method: "GET", path: "/v1/gym", operatorOnly: false, summary: "The Gym (3D PLE Workbench): open, run, step, layers, selected element, Live"),
        ForemanRoute(method: "POST", path: "/v1/gym", operatorOnly: false, summary: "Steer the Gym: {run?, camera?, only?, show?, hide?, step?, select?, live?, open?, id?}"),
        ForemanRoute(method: "GET", path: "/v1/wallet/status", operatorOnly: false, summary: "Bitcoin/Lightning Spark wallet status, balance in sats, Spark address, and daily limit"),
        ForemanRoute(method: "POST", path: "/v1/wallet/receive", operatorOnly: false, summary: "Generate a BOLT11 invoice, Spark address, or Bitcoin address: {kind?, amount_sats?, description?}"),
        ForemanRoute(method: "POST", path: "/v1/wallet/quote", operatorOnly: false, summary: "Prepare a real Spark send quote: {destination, amount_sats?, comment?}"),
        ForemanRoute(method: "POST", path: "/v1/wallet/pay", operatorOnly: false, summary: "Request a Bitcoin/Lightning/x402 spend (always gated by pay/wallet-rpc.coax + interactive user approval): {destination, amount_sats?, comment?, reason?, x402_challenge?}")
    ]

    /// A token of 48 hex characters from the system's random source.
    static func newToken() -> String {
        var bytes = [UInt8](repeating: 0, count: 24)
        arc4random_buf(&bytes, bytes.count)
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    /// Compares every byte whatever the first difference is, so the time taken says nothing about the token.
    static func tokensEqual(_ a: String, _ b: String) -> Bool {
        let x = Array(a.utf8), y = Array(b.utf8)
        guard !y.isEmpty else { return false }
        var diff = UInt8(x.count == y.count ? 0 : 1)
        for i in 0..<y.count { diff |= (i < x.count ? x[i] : 0) ^ y[i] }
        return diff == 0
    }

    static func principal(for supplied: String, foremanToken: String, operatorToken: String) -> ForemanPrincipal? {
        let isOperator = tokensEqual(supplied, operatorToken), isForeman = tokensEqual(supplied, foremanToken)
        return isOperator ? .operatorClient : isForeman ? .foreman : nil
    }

    /// Every path under /v1/foreman, and any write to the tier, is the operator's alone, whether or
    /// not the table lists it yet.
    static func isOperatorOnly(method: String, path: String) -> Bool {
        if path == "/v1/foreman" || path.hasPrefix("/v1/foreman/") { return true }
        if (path == "/v1/tier" || path == "/tier") && method != "GET" { return true }
        return routes.contains { $0.operatorOnly && $0.method == method && $0.path == path }
    }

    /// The refusal for a request this principal may not make, nil when it may.
    static func authorize(principal: ForemanPrincipal, method: String, path: String) -> ForemanResponse? {
        guard isOperatorOnly(method: method, path: path), principal != .operatorClient else { return nil }
        return ForemanResponse(status: 403, body: [
            "error": "\(method) \(path) needs the operator credential. Foreman cannot manage itself.",
            "principal": principal.rawValue
        ])
    }

    /// The refusal for a tier change, nil when it may go ahead. Lowering always may. Raising needs
    /// the switch a person turns on in the app.
    static func tierChange(from: ForemanTier, to: ForemanTier, raiseAllowed: Bool) -> ForemanResponse? {
        guard to.rawValue > from.rawValue, !raiseAllowed else { return nil }
        return ForemanResponse(status: 403, body: [
            "error": "A program may lower the Foreman tier but not raise it. To allow that, right-click Foreman in Coder's sidebar and choose Let Programs Raise the Tier.",
            "tier": from.wireName,
            "requested": to.wireName,
            "operator_tier_raise": false
        ])
    }

    static func describe(principal: ForemanPrincipal) -> [String: Any] {
        [
            "ok": true,
            "api_version": version,
            "principal": principal.rawValue,
            "tiers": ForemanTier.allCases.map { ["tier": $0.wireName, "level": $0.rawValue, "summary": $0.summary] as [String: Any] },
            "routes": routes.map { $0.asDictionary() }
        ]
    }
}

/// The file a program reads to find this app's control server: its URL and the operator token.
/// Only this user can read it (mode 0600). The newest Coder to launch owns it.
enum ForemanControlDescriptor {
    static let fileName = "foreman-control.json"

    /// CODER_FOREMAN_CONTROL_FILE when set, else foreman-control.json in Application Support/Coder.
    static var defaultURL: URL {
        if let custom = ProcessInfo.processInfo.environment["CODER_FOREMAN_CONTROL_FILE"], !custom.isEmpty {
            return URL(fileURLWithPath: custom)
        }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        return base.appendingPathComponent("Coder").appendingPathComponent(fileName)
    }

    static func body(url: String, token: String, auditLog: String) -> [String: Any] {
        [
            "api_version": ForemanAPI.version,
            "url": url,
            "token": token,
            "principal": ForemanPrincipal.operatorClient.rawValue,
            "pid": Int(getpid()),
            "started": Date().timeIntervalSince1970,
            "audit_log": auditLog
        ]
    }

    /// Writes the file whole: a temporary file made 0600, then renamed over the old one.
    @discardableResult
    static func write(to file: URL = defaultURL, url: String, token: String, auditLog: String) -> Bool {
        guard !url.isEmpty,
              let data = try? JSONSerialization.data(withJSONObject: body(url: url, token: token, auditLog: auditLog), options: [.sortedKeys, .prettyPrinted]) else { return false }
        let fm = FileManager.default
        try? fm.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        let tmp = file.deletingLastPathComponent().appendingPathComponent(".\(file.lastPathComponent).\(getpid())")
        try? fm.removeItem(at: tmp)
        guard fm.createFile(atPath: tmp.path, contents: data, attributes: [.posixPermissions: 0o600]) else { return false }
        guard rename(tmp.path, file.path) == 0 else { try? fm.removeItem(at: tmp); return false }
        return true
    }

    static func read(from file: URL = defaultURL) -> [String: Any]? {
        guard let data = try? Data(contentsOf: file) else { return nil }
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
    }

    /// Removes the file if this process wrote it; a newer Coder's file is left alone.
    static func remove(at file: URL = defaultURL) {
        guard let pid = read(from: file)?["pid"] as? Int, pid == Int(getpid()) else { return }
        try? FileManager.default.removeItem(at: file)
    }
}
