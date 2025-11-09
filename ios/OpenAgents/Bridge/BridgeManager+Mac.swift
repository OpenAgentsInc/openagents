import Foundation
import Combine
import OpenAgentsCore

#if os(macOS)
extension BridgeManager {
    private static let workingDirectoryKey = "oa.bridge.working_directory"

    func start() {
        loadWorkingDirectory()
        let conn = DesktopConnectionManager()
        conn.workingDirectoryURL = workingDirectory
        connection = conn

        // Connection events
        conn.statusPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] st in self?.status = st }
            .store(in: &subscriptions)
        conn.logPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] line in self?.log("conn", line) }
            .store(in: &subscriptions)
        conn.connectedClientCountPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] count in self?.connectedClientCount = count }
            .store(in: &subscriptions)

        // Forward session/update notifications into TimelineStore
        conn.notificationPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] evt in
                guard let self = self else { return }
                if evt.method == ACPRPC.sessionUpdate {
                    self.timeline.applySessionUpdatePayload(evt.payload)
                } else if let s = String(data: evt.payload, encoding: .utf8) {
                    self.log("client", "notify \(evt.method): \(s)")
                }
            }
            .store(in: &subscriptions)

        // Mirror timeline state to published fields
        timeline.updatesPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.updates = $0 }
            .store(in: &subscriptions)
        timeline.availableCommandsPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.availableCommands = $0 }
            .store(in: &subscriptions)
        timeline.currentModePublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.currentMode = $0 }
            .store(in: &subscriptions)
        timeline.toolCallNamesPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.toolCallNames = $0 }
            .store(in: &subscriptions)
        timeline.rawJSONByCallIdPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.rawJSONByCallId = $0 }
            .store(in: &subscriptions)
        timeline.outputJSONByCallIdPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.outputJSONByCallId = $0 }
            .store(in: &subscriptions)

        // Initialize dispatcher with local RPC client
        dispatcher = PromptDispatcher(rpc: conn.rpcClient, timeline: timeline)
        conn.start()
    }

    func stop() { connection?.stop(); connection = nil }

    func setWorkingDirectory(_ url: URL) {
        workingDirectory = url
        saveWorkingDirectory(url)
        (connection as? DesktopConnectionManager)?.workingDirectoryURL = url
    }

    func loadWorkingDirectory() {
        if let path = UserDefaults.standard.string(forKey: Self.workingDirectoryKey) {
            let url = URL(fileURLWithPath: path)
            if FileManager.default.fileExists(atPath: path) {
                workingDirectory = url
                log("workdir", "Loaded working directory: \(path)")
            }
        }
    }

    private func saveWorkingDirectory(_ url: URL) {
        UserDefaults.standard.set(url.path, forKey: Self.workingDirectoryKey)
        log("workdir", "Saved working directory: \(url.path)")
    }

    // Published properties are defined in BridgeManager

    // MARK: - Chat controls (shared with iOS patterns)
    func startNewSession(desiredMode: ACPSessionModeId? = nil) {
        // Clear local state immediately
        timeline.clearAll()
        objectWillChange.send()
        currentSessionId = nil

        guard let rpc = connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: "session-new-\(UUID().uuidString)") { (resp: ACP.Agent.SessionNewResponse?) in
            guard let resp = resp else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.currentSessionId = resp.session_id
                if let mode = desiredMode {
                    struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
                    rpc.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: resp.session_id, mode_id: mode), id: "session-set-mode-\(UUID().uuidString)") { (_: ACP.Agent.SetSessionModeResponse?) in }
                }
            }
        }
    }

    func fetchRecentSessions() {
        dispatcher?.fetchRecentSessions { [weak self] items in
            self?.recentSessions = items
        }
    }

    func loadSessionTimeline(sessionId: String) {
        currentSessionId = nil
        timeline.clearAll()
        dispatcher?.loadSessionTimeline(sessionId: sessionId) { [weak self] arr in
            self?.timeline.replaceAll(with: arr)
            self?.currentSessionId = ACPSessionId(sessionId)
            self?.log("history", "Loaded timeline for session \(sessionId): \(arr.count) updates")
        }
    }

    // MARK: - Filesystem fallback for non-Tinyvex sessions
    func loadFilesystemSessionTimeline(sessionId: String, mode: String?) {
        currentSessionId = nil
        timeline.clearAll()
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let provider = (mode == "claude-code" ? "claude-code" : "codex")
            let fm = FileManager.default
            let url: URL? = {
                if provider == "claude-code" {
                    let base = ClaudeCodeScanner.defaultBaseDir()
                    let direct = base.appendingPathComponent(sessionId).appendingPathExtension("jsonl")
                    if fm.fileExists(atPath: direct.path) { return direct }
                    let files = ClaudeCodeScanner.listRecentTopN(at: base, topK: 2000)
                    for f in files {
                        if ClaudeCodeScanner.relativeId(for: f, base: base) == sessionId { return f }
                    }
                    return nil
                } else {
                    let base = CodexScanner.defaultBaseDir()
                    let direct = base.appendingPathComponent(sessionId).appendingPathExtension("jsonl")
                    if fm.fileExists(atPath: direct.path) { return direct }
                    let files = CodexScanner.listRecentTopN(at: base, topK: 2000)
                    for f in files {
                        if self.codexRelativeId(for: f, base: base) == sessionId { return f }
                    }
                    return nil
                }
            }()

            guard let fileURL = url,
                  let data = try? Data(contentsOf: fileURL),
                  let text = String(data: data, encoding: .utf8) else {
                DispatchQueue.main.async {
                    self.log("history", "Filesystem timeline not found for id=\(sessionId) provider=\(provider)")
                    self.currentSessionId = ACPSessionId(sessionId)
                }
                return
            }
            let lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
            let updates = self.makeTypedUpdatesFromLines(lines, provider: provider)
            let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: ACPSessionId(sessionId), update: $0, _meta: nil) }
            DispatchQueue.main.async {
                self.timeline.replaceAll(with: wires)
                self.currentSessionId = ACPSessionId(sessionId)
                self.log("history", "Loaded FS timeline for session \(sessionId): \(wires.count) updates")
            }
        }
    }

    private func makeTypedUpdatesFromLines(_ lines: [String], provider: String) -> [ACP.Client.SessionUpdate] {
        var out: [ACP.Client.SessionUpdate] = []
        if provider == "claude-code" {
            let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "desktop"))
            for event in thread.events {
                if let m = event.message {
                    let text = m.parts.compactMap { part -> String? in
                        if case let .text(tt) = part { return tt.text } else { return nil }
                    }.joined(separator: "\n")
                    guard !text.isEmpty else { continue }
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                    if m.role == .user {
                        out.append(.userMessageChunk(chunk))
                    } else {
                        if m.isThinking == true { out.append(.agentThoughtChunk(chunk)) }
                        else { out.append(.agentMessageChunk(chunk)) }
                    }
                }
                if let call = event.tool_call {
                    let args = jsonValueToAnyDict(call.arguments)
                    let wire = ACPToolCallWire(call_id: call.id, name: call.tool_name, arguments: args, _meta: nil)
                    out.append(.toolCall(wire))
                }
                if let res = event.tool_result {
                    let status: ACPToolCallUpdateWire.Status = res.ok ? .completed : .error
                    let output = res.result.map { jsonValueToAnyEncodable($0) }
                    let wire = ACPToolCallUpdateWire(call_id: res.call_id, status: status, output: output, error: res.error, _meta: nil)
                    out.append(.toolCallUpdate(wire))
                }
                if let ps = event.plan_state {
                    let entries = (ps.steps ?? []).map { ACPPlanEntry(content: $0, priority: .medium, status: .in_progress, _meta: nil) }
                    let plan = ACPPlan(entries: entries, _meta: nil)
                    out.append(.plan(plan))
                }
            }
            return out
        }
        // Codex: per-line mapping
        for line in lines {
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }
            if isReasoningLine(obj) {
                if let t = textFromTranslatedLine(line) {
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: t)))
                    out.append(.agentThoughtChunk(chunk))
                }
                continue
            }
            let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: "desktop"))
            if let m = t.events.compactMap({ $0.message }).first {
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(tt) = part { return tt.text } else { return nil }
                }.joined(separator: "\n")
                guard !text.isEmpty else { continue }
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                out.append(m.role == .user ? .userMessageChunk(chunk) : .agentMessageChunk(chunk))
                continue
            }
            if let call = t.events.compactMap({ $0.tool_call }).first {
                let args = jsonValueToAnyDict(call.arguments)
                let wire = ACPToolCallWire(call_id: call.id, name: call.tool_name, arguments: args, _meta: nil)
                out.append(.toolCall(wire))
                continue
            }
            if let res = t.events.compactMap({ $0.tool_result }).first {
                let status: ACPToolCallUpdateWire.Status = res.ok ? .completed : .error
                let output = res.result.map { jsonValueToAnyEncodable($0) }
                let wire = ACPToolCallUpdateWire(call_id: res.call_id, status: status, output: output, error: res.error, _meta: nil)
                out.append(.toolCallUpdate(wire))
                continue
            }
            if let ps = t.events.compactMap({ $0.plan_state }).first {
                let entries = (ps.steps ?? []).map { ACPPlanEntry(content: $0, priority: .medium, status: .in_progress, _meta: nil) }
                let plan = ACPPlan(entries: entries, _meta: nil)
                out.append(.plan(plan))
                continue
            }
        }
        return out
    }

    private func isReasoningLine(_ obj: [String: Any]) -> Bool {
        let t = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        if t == "event_msg", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any]),
           ((item["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if t == "response_item", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "reasoning" { return true }
        return false
    }

    private func textFromTranslatedLine(_ line: String) -> String? {
        let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: "desktop"))
        guard let m = t.events.compactMap({ $0.message }).first else { return nil }
        let text = m.parts.compactMap { part -> String? in
            if case let .text(tt) = part { return tt.text } else { return nil }
        }.joined(separator: "\n")
        return text.isEmpty ? nil : text
    }

    private func jsonValueToAnyDict(_ v: JSONValue) -> [String: AnyEncodable]? {
        if case .object(let o) = v {
            var out: [String: AnyEncodable] = [:]
            for (k, val) in o { out[k] = jsonValueToAnyEncodable(val) }
            return out
        }
        return nil
    }

    private func jsonValueToAnyEncodable(_ v: JSONValue) -> AnyEncodable {
        switch v {
        case .null: return AnyEncodable([String: String]())
        case .bool(let b): return AnyEncodable(b)
        case .number(let d): return AnyEncodable(d)
        case .string(let s): return AnyEncodable(s)
        case .array(let arr): return AnyEncodable(arr.map { jsonValueToAnyEncodable($0) })
        case .object(let obj): return AnyEncodable(obj.mapValues { jsonValueToAnyEncodable($0) })
        }
    }

    private func codexRelativeId(for url: URL, base: URL) -> String {
        let u = url.deletingPathExtension().resolvingSymlinksInPath().standardizedFileURL.path
        let b = base.resolvingSymlinksInPath().standardizedFileURL.path
        if u.hasPrefix(b + "/") {
            let rel = String(u.dropFirst(b.count + 1))
            return rel
        }
        return url.deletingPathExtension().lastPathComponent
    }
}
#endif
