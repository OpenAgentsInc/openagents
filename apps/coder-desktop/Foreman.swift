// Foreman MVP: persistent orchestration chat, permission tiers, and loopback control bridge.
import AppKit
import Darwin
import Foundation

enum ForemanTier: Int, CaseIterable {
    case observe = 0
    case managed = 1
    case autonomous = 2

    var wireName: String {
        switch self {
        case .observe: return "observe"
        case .managed: return "managed"
        case .autonomous: return "autonomous"
        }
    }

    var label: String {
        switch self {
        case .observe: return "Observe (Read-Only)"
        case .managed: return "Managed (Default)"
        case .autonomous: return "Autonomous"
        }
    }

    var shortLabel: String {
        switch self {
        case .observe: return "Observe"
        case .managed: return "Managed"
        case .autonomous: return "Auto"
        }
    }

    var summary: String {
        switch self {
        case .observe: return "Inspect chats, search repo & chat history only."
        case .managed: return "Create and control Foreman-managed chats."
        case .autonomous: return "Create, send to, and control any workspace chat."
        }
    }

    static func from(name: String) -> ForemanTier? {
        switch name.lowercased() {
        case "observe", "readonly", "read-only", "0": return .observe
        case "managed", "assist", "supervised", "1": return .managed
        case "autonomous", "auto", "2": return .autonomous
        default: return nil
        }
    }

    static func load() -> ForemanTier {
        if UserDefaults.standard.object(forKey: Foreman.tierDefaultsKey) != nil,
           let t = ForemanTier(rawValue: UserDefaults.standard.integer(forKey: Foreman.tierDefaultsKey)) {
            return t
        }
        return .managed
    }

    func save() {
        UserDefaults.standard.set(rawValue, forKey: Foreman.tierDefaultsKey)
    }
}

enum Foreman {
    static let chatID = "foreman"
    static let title = "Foreman"
    static let managedTag = "foreman"
    static let tierDefaultsKey = "foremanTier"

    static func systemPrompt(tier: ForemanTier, workspace: String, chatCount: Int, runningCount: Int, baseSkills: String) -> String {
        var lines: [String] = []
        lines.append("You are the Foreman: the persistent orchestration agent pinned at the top of Coder Desktop.")
        lines.append("Current permission tier: Tier \(tier.rawValue) (\(tier.label)) — \(tier.summary)")
        lines.append("Active workspace folder: \(workspace) | Chats in store: \(chatCount) | Running chats: \(runningCount)")
        lines.append("Use the `foreman-ctl` command (already on PATH) to inspect and coordinate Coder Desktop chats:")
        lines.append("- `foreman-ctl status` — view tier, active runs, and workspace summary")
        lines.append("- `foreman-ctl list-chats [--limit 50] [--managed-only] [--filter all|active|idle|failed|managed]` — list sidebar chats with status, cost, budget, goal, and managed_by")
        lines.append("- `foreman-ctl read-chat <chat_id> [--tail 20] [--include-tools]` — read a chat's status, cost/budget, goal, and recent turns")
        lines.append("- `foreman-ctl create-chat --title \"...\" --prompt \"...\" [--folder PATH] [--budget USD] [--goal \"...\"] [--pin]` — spawn a new Foreman-managed sidebar chat and start its agent")
        lines.append("- `foreman-ctl send-chat <chat_id> --prompt \"...\" [--mode follow_up|steer|interrupt]` — send a message (attributed From Foreman) to an existing chat")
        lines.append("- `foreman-ctl control-chat <chat_id> --action <stop|resume|rename|pin|unpin|delete|approve|budget|goal> [--title \"...\"] [--budget USD] [--goal \"...\"]` — control a chat")
        lines.append("- `foreman-ctl wait <chat_id...> [--all] [--timeout 60] [--since-id N]` — wait for one or all specified chats to finish or emit a lifecycle event")
        lines.append("- `foreman-ctl events [--since-id N] [--chat-id ID] [--limit 50]` — read recent chat lifecycle events")
        lines.append("- `foreman-ctl audit [--limit 50]` — read the Foreman audit log")
        lines.append("- `foreman-ctl stop-all [--all]` — stop running Foreman-managed chats (or all chats in Full Auto)")
        lines.append("- `foreman-ctl search-repo <query> [--folder PATH] [--glob PATTERN] [--max 50]` — search across the repository")
        lines.append("- `foreman-ctl search-chats <query> [--limit 20] [--folder PATH] [--date-from YYYY-MM-DD] [--date-to YYYY-MM-DD]` — search previous chats in the local Coder store")
        lines.append("Always report chat IDs and titles clearly when spawning or inspecting chats. If an action is blocked by the current permission tier, tell the user which tier is needed and that they can switch it in the Foreman menu.")
        if !baseSkills.isEmpty {
            lines.append("")
            lines.append(baseSkills.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return lines.joined(separator: "\n") + "\n\n"
    }
}

struct ForemanRequest {
    let principal: ForemanPrincipal
    let method: String
    let path: String
    let query: [String: String]
    let payload: [String: Any]
}

struct ForemanResponse {
    let status: Int
    let body: [String: Any]
}

struct ForemanEvent {
    let id: Int
    let ts: Double
    let type: String
    let chatId: String
    let title: String
    let managedBy: String
    let details: [String: Any]

    func asDictionary() -> [String: Any] {
        var out: [String: Any] = [
            "id": id,
            "ts": ts,
            "type": type,
            "chat_id": chatId,
            "title": title,
            "managed_by": managedBy
        ]
        if !details.isEmpty { out["details"] = details }
        return out
    }
    func toDictionary() -> [String: Any] { asDictionary() }
    var chatID: String? { chatId.isEmpty ? nil : chatId }
}

/// In-memory ring buffer and waiter registry for chat lifecycle events.
final class ForemanEventBus {
    private final class Waiter {
        let chatIds: Set<String>
        let predicate: (() -> Bool)?
        let completion: (Bool, [ForemanEvent]) -> Void
        var timer: DispatchSourceTimer?
        var fired = false
        init(chatIds: Set<String>, predicate: (() -> Bool)?, completion: @escaping (Bool, [ForemanEvent]) -> Void) {
            self.chatIds = chatIds
            self.predicate = predicate
            self.completion = completion
        }
    }

    private var events: [ForemanEvent] = []
    private var nextId = 1
    private var waiters: [Waiter] = []
    private var subscribers: [UUID: (ForemanEvent) -> Void] = [:]
    private let maxEvents = 500

    var latestId: Int { events.last?.id ?? 0 }
    var latestID: Int { latestId }

    @discardableResult
    func emit(type: String, chatId: String = "", title: String = "", managedBy: String = "", details: [String: Any] = [:]) -> ForemanEvent {
        let ev = ForemanEvent(id: nextId, ts: Date().timeIntervalSince1970, type: type, chatId: chatId, title: title, managedBy: managedBy, details: details)
        nextId += 1
        events.append(ev)
        if events.count > maxEvents {
            events.removeFirst(events.count - maxEvents)
        }
        for cb in Array(subscribers.values) {
            cb(ev)
        }
        notifyWaiters(event: ev)
        return ev
    }

    @discardableResult
    func emit(type: String, chatID: String?, data: [String: Any] = [:]) -> ForemanEvent {
        let title = (data["title"] as? String) ?? ""
        let managedBy = (data["managed_by"] as? String) ?? ""
        return emit(type: type, chatId: chatID ?? "", title: title, managedBy: managedBy, details: data)
    }

    func subscribe(_ handler: @escaping (ForemanEvent) -> Void) -> UUID {
        let id = UUID()
        subscribers[id] = handler
        return id
    }

    func unsubscribe(_ id: UUID) {
        subscribers.removeValue(forKey: id)
    }

    func since(_ sinceId: Int, limit: Int = 50) -> [ForemanEvent] {
        recent(sinceId: sinceId, limit: limit)
    }

    func recent(sinceId: Int = 0, chatIds: Set<String> = [], limit: Int = 50) -> [ForemanEvent] {
        let filtered = events.filter { ev in
            ev.id > sinceId && (chatIds.isEmpty || chatIds.contains(ev.chatId))
        }
        if filtered.count <= limit { return filtered }
        return Array(filtered.suffix(limit))
    }

    func wait(chatIds: Set<String>, sinceId: Int, timeoutSeconds: Double, readyNow: (() -> Bool)? = nil, completion: @escaping (Bool, [ForemanEvent]) -> Void) {
        let existing = recent(sinceId: sinceId, chatIds: chatIds, limit: 100)
        if !existing.isEmpty, readyNow == nil || readyNow?() == true {
            completion(false, existing)
            return
        }
        if let ready = readyNow, ready() {
            completion(false, existing)
            return
        }
        let waiter = Waiter(chatIds: chatIds, predicate: readyNow, completion: completion)
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + max(0.1, timeoutSeconds))
        timer.setEventHandler { [weak self, weak waiter] in
            guard let self = self, let waiter = waiter, !waiter.fired else { return }
            waiter.fired = true
            waiter.timer?.cancel()
            waiter.timer = nil
            self.waiters.removeAll { $0 === waiter }
            let evs = self.recent(sinceId: sinceId, chatIds: chatIds, limit: 100)
            waiter.completion(true, evs)
        }
        waiter.timer = timer
        waiters.append(waiter)
        timer.resume()
    }

    private func notifyWaiters(event: ForemanEvent) {
        let isLifecycleTerminal = event.type == "chat.completed" || event.type == "chat.failed" || event.type == "chat.stopped" || event.type == "chat.budget_exceeded" || event.type == "chat.deleted"
        var remaining: [Waiter] = []
        for waiter in waiters {
            if waiter.fired { continue }
            let matchesChat = waiter.chatIds.isEmpty || waiter.chatIds.contains(event.chatId)
            let ready = waiter.predicate?() ?? isLifecycleTerminal
            if matchesChat && ready {
                waiter.fired = true
                waiter.timer?.cancel()
                waiter.timer = nil
                waiter.completion(false, [event])
            } else {
                remaining.append(waiter)
            }
        }
        waiters = remaining
    }
}

/// Append-only JSONL audit log for Foreman orchestration actions.
final class ForemanAuditLog {
    let fileURL: URL
    private let queue = DispatchQueue(label: "coder.foreman.audit")

    convenience init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        self.init(directory: base.appendingPathComponent("Coder"))
    }

    init(directory: URL) {
        fileURL = directory.appendingPathComponent("foreman-audit.jsonl")
    }

    func record(action: String, tier: ForemanTier, chatID: String? = nil, status: Int = 200, detail: [String: Any] = [:]) {
        var summary = action
        if !detail.isEmpty, let data = try? JSONSerialization.data(withJSONObject: detail, options: [.sortedKeys]), let text = String(data: data, encoding: .utf8) {
            summary = "\(action) \(text)"
        }
        record(tier: tier, method: "FOREMAN", path: action, chatId: chatID ?? "", status: status, summary: summary)
    }

    func record(action: String, tier: ForemanTier, chatID: String? = nil, status: Int = 200, details: [String: Any]) {
        record(action: action, tier: tier, chatID: chatID, status: status, detail: details)
    }

    func record(tier: ForemanTier, method: String, path: String, chatId: String = "", status: Int, summary: String = "", error: String = "") {
        var entry: [String: Any] = [
            "ts": Date().timeIntervalSince1970,
            "tier": tier.rawValue,
            "tier_label": tier.label,
            "method": method,
            "path": path,
            "status": status
        ]
        if !chatId.isEmpty { entry["chat_id"] = chatId }
        if !summary.isEmpty { entry["summary"] = summary }
        if !error.isEmpty { entry["error"] = error }
        guard let data = try? JSONSerialization.data(withJSONObject: entry, options: [.sortedKeys]),
              var line = String(data: data, encoding: .utf8) else { return }
        line.append("\n")
        let url = fileURL
        queue.async {
            let fm = FileManager.default
            try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            if !fm.fileExists(atPath: url.path) {
                fm.createFile(atPath: url.path, contents: nil)
            }
            if let fh = try? FileHandle(forWritingTo: url) {
                _ = try? fh.seekToEnd()
                fh.write(Data(line.utf8))
                try? fh.close()
            }
        }
    }

    func tail(limit: Int = 50) -> [[String: Any]] {
        let clamped = max(1, min(500, limit))
        return queue.sync {
            guard let raw = try? String(contentsOf: fileURL, encoding: .utf8) else { return [] }
            let lines = raw.split(separator: "\n").suffix(clamped)
            var out: [[String: Any]] = []
            for line in lines {
                if let data = line.data(using: .utf8),
                   let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                    out.append(obj)
                }
            }
            return out
        }
    }
}

/// Minimal loopback-only HTTP/1.1 JSON server for `foreman-ctl` -> `CoderWindow` orchestration.
final class ForemanControlServer {
    private var listenFD: Int32 = -1
    private let queue = DispatchQueue(label: "coder.foreman.server", attributes: .concurrent)
    private var stopped = false
    let token: String            // the Foreman worker's credential
    let operatorToken: String    // the credential of a program outside the Foreman chat (ForemanOperator.swift)
    private(set) var port: UInt16 = 0
    var handler: ((ForemanRequest, @escaping (ForemanResponse) -> Void) -> Void)?

    var baseURL: String { port > 0 ? "http://127.0.0.1:\(port)" : "" }

    init() {
        token = ForemanAPI.newToken()
        operatorToken = ForemanAPI.newToken()
    }

    @discardableResult
    func start() -> Bool {
        guard listenFD < 0 else { return true }
        let fd = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        var yes: Int32 = 1
        _ = setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))

        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        addr.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))

        let bound = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bound == 0, Darwin.listen(fd, 16) == 0 else {
            Darwin.close(fd)
            return false
        }

        var actual = sockaddr_in()
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &actual) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                getsockname(fd, $0, &len)
            }
        }
        port = UInt16(bigEndian: actual.sin_port)
        listenFD = fd
        stopped = false

        queue.async { [weak self] in
            self?.acceptLoop()
        }
        return true
    }

    func stop() {
        stopped = true
        if listenFD >= 0 {
            Darwin.close(listenFD)
            listenFD = -1
        }
        port = 0
    }

    private func acceptLoop() {
        while !stopped && listenFD >= 0 {
            var clientAddr = sockaddr_in()
            var clientLen = socklen_t(MemoryLayout<sockaddr_in>.size)
            let client = withUnsafeMutablePointer(to: &clientAddr) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                    Darwin.accept(listenFD, $0, &clientLen)
                }
            }
            if client < 0 { break }
            queue.async { [weak self] in
                self?.handleClient(client)
            }
        }
    }

    private func handleClient(_ fd: Int32) {
        defer { Darwin.close(fd) }
        var tv = timeval(tv_sec: 5, tv_usec: 0)
        _ = setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))

        var data = Data()
        var buf = [UInt8](repeating: 0, count: 8192)
        var headerEnd: Int? = nil
        var contentLength = 0

        while data.count < 512 * 1024 {
            let n = Darwin.recv(fd, &buf, buf.count, 0)
            if n <= 0 { break }
            data.append(contentsOf: buf[0..<n])
            if headerEnd == nil, let range = data.range(of: Data([13, 10, 13, 10])) {
                headerEnd = range.upperBound
                let headerStr = String(decoding: data[..<range.lowerBound], as: UTF8.self)
                for line in headerStr.split(separator: "\r\n") {
                    let lower = line.lowercased()
                    if lower.hasPrefix("content-length:") {
                        contentLength = Int(lower.dropFirst("content-length:".count).trimmingCharacters(in: .whitespaces)) ?? 0
                    }
                }
            }
            if let end = headerEnd, data.count - end >= contentLength {
                break
            }
        }

        guard let end = headerEnd else {
            sendJSON(fd, status: 400, body: ["error": "malformed HTTP request"])
            return
        }

        let headerText = String(decoding: data[..<(end - 4)], as: UTF8.self)
        let headerLines = headerText.components(separatedBy: "\r\n")
        guard let requestLine = headerLines.first else {
            sendJSON(fd, status: 400, body: ["error": "missing request line"])
            return
        }
        let parts = requestLine.split(separator: " ")
        guard parts.count >= 2 else {
            sendJSON(fd, status: 400, body: ["error": "invalid request line"])
            return
        }
        let method = String(parts[0])
        let rawTarget = String(parts[1])
        let (path, query) = parseTarget(rawTarget)

        var suppliedToken = ""
        for line in headerLines.dropFirst() {
            if let colon = line.firstIndex(of: ":") {
                let k = line[..<colon].trimmingCharacters(in: .whitespaces).lowercased()
                let v = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
                if k == "x-foreman-token" { suppliedToken = v }
            }
        }
        guard let principal = ForemanAPI.principal(for: suppliedToken, foremanToken: token, operatorToken: operatorToken) else {
            sendJSON(fd, status: 401, body: ["error": "unauthorized"])
            return
        }

        var payload: [String: Any] = [:]
        if contentLength > 0, data.count >= end + contentLength {
            let bodyData = data[end..<(end + contentLength)]
            if let obj = try? JSONSerialization.jsonObject(with: bodyData) as? [String: Any] {
                payload = obj
            }
        }

        let request = ForemanRequest(principal: principal, method: method, path: path, query: query, payload: payload)
        let response: ForemanResponse
        if let handler = handler {
            var result = ForemanResponse(status: 504, body: ["error": "request timed out"])
            let sem = DispatchSemaphore(value: 0)
            var done = false
            let waitCap: Double
            if path == "/v1/wait" || path == "/v1/foreman/wait" {
                let requested = (payload["timeout_seconds"] as? Double) ?? Double((payload["timeout_seconds"] as? Int) ?? 60)
                waitCap = min(315, max(5, requested + 15))
            } else {
                waitCap = 30
            }
            DispatchQueue.main.async {
                handler(request) { resp in
                    if !done {
                        done = true
                        result = resp
                        sem.signal()
                    }
                }
            }
            _ = sem.wait(timeout: .now() + waitCap)
            response = result
        } else {
            response = ForemanResponse(status: 503, body: ["error": "foreman bridge unavailable"])
        }
        sendJSON(fd, status: response.status, body: response.body)
    }

    private func parseTarget(_ raw: String) -> (String, [String: String]) {
        guard let qIdx = raw.firstIndex(of: "?") else { return (raw, [:]) }
        let path = String(raw[..<qIdx])
        let qs = raw[raw.index(after: qIdx)...]
        var out: [String: String] = [:]
        for pair in qs.split(separator: "&") {
            if let eq = pair.firstIndex(of: "=") {
                let k = String(pair[..<eq]).removingPercentEncoding ?? String(pair[..<eq])
                let v = String(pair[pair.index(after: eq)...]).replacingOccurrences(of: "+", with: " ")
                out[k] = v.removingPercentEncoding ?? v
            } else if !pair.isEmpty {
                let k = String(pair).removingPercentEncoding ?? String(pair)
                out[k] = "1"
            }
        }
        return (path, out)
    }

    private func sendJSON(_ fd: Int32, status: Int, body: [String: Any]) {
        let statusText: String
        switch status {
        case 200: statusText = "OK"
        case 400: statusText = "Bad Request"
        case 401: statusText = "Unauthorized"
        case 403: statusText = "Forbidden"
        case 404: statusText = "Not Found"
        case 409: statusText = "Conflict"
        case 504: statusText = "Gateway Timeout"
        default: statusText = "Error"
        }
        let jsonData = (try? JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])) ?? Data("{}".utf8)
        let header = "HTTP/1.1 \(status) \(statusText)\r\nContent-Type: application/json\r\nContent-Length: \(jsonData.count)\r\nConnection: close\r\n\r\n"
        var out = Data(header.utf8)
        out.append(jsonData)
        out.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            var sent = 0
            while sent < out.count {
                let n = Darwin.send(fd, base.advanced(by: sent), out.count - sent, 0)
                if n <= 0 { break }
                sent += n
            }
        }
    }
}
