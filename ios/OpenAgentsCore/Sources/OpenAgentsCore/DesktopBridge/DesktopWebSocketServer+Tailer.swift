#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - Session file tailing helpers
    func findAndTailSessionFile(sessionId: ACPSessionId, client: Client) {
        let sidStr = sessionId.value
        let chosenMode = modeBySession[sidStr] ?? .default_mode
        var picked: URL? = nil
        var provider = "claude-code"
        if chosenMode == .codex {
            let codexBase = CodexScanner.defaultBaseDir()
            picked = CodexScanner.listRecentTopN(at: codexBase, topK: 1).first
            provider = "codex"
        }
        if picked == nil {
            let claudeBase = ClaudeCodeScanner.defaultBaseDir()
            picked = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: 1).first
            provider = "claude-code"
        }

        guard let file = picked else {
            OpenAgentsLog.server.warning("no session file found for \(sessionId.value) (mode=\(chosenMode.rawValue))")
            return
        }

        sessionFiles[sidStr] = file

        OpenAgentsLog.server.debug("tailing session file: \(file.path, privacy: .private)")

        // Update the live tailer to watch this file
        queue.async { [weak self] in
            guard let self = self else { return }
            self.tailURL = file
            self.tailSessionId = sessionId
            self.tailProvider = provider
            let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber)?.uint64Value ?? 0
            self.tailOffset = size
            self.tailBuffer.removeAll(keepingCapacity: true)

            // Start tailer if not already running
            if self.tailTimer == nil {
                let timer = DispatchSource.makeTimerSource(queue: self.queue)
                timer.schedule(deadline: .now() + .milliseconds(100), repeating: .milliseconds(100))
                timer.setEventHandler { [weak self] in self?.pollTail() }
                self.tailTimer = timer
                timer.resume()
                OpenAgentsLog.server.debug("tailer started for session \(sidStr)")
            }
        }
    }

    // MARK: - Live tailer
    func startLiveTailerIfNeeded() {
        guard tailTimer == nil else { return }

        // Prioritize Claude Code sessions, fall back to Codex only if none exist
        let claudeBase = ClaudeCodeScanner.defaultBaseDir()
        let claudeFiles = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: 1)

        var file: URL?
        var provider: String = "claude-code"
        var base: URL = claudeBase
        var tid: String?

        if let clf = claudeFiles.first {
            // Use Claude Code session
            file = clf
            provider = "claude-code"
            base = claudeBase
            tid = ClaudeCodeScanner.scanForSessionID(clf) ?? ClaudeCodeScanner.relativeId(for: clf, base: claudeBase)
        } else {
            // Fallback to Codex only if no Claude Code sessions exist
            let codexBase = CodexScanner.defaultBaseDir()
            let codexFiles = CodexScanner.listRecentTopN(at: codexBase, topK: 1)
            if let cf = codexFiles.first {
                file = cf
                provider = "codex"
                base = codexBase
                tid = CodexScanner.scanForThreadID(cf) ?? CodexScanner.relativeId(for: cf, base: codexBase)
            }
        }

        guard let file = file, let tid = tid else { return }

        tailURL = file
        tailSessionId = ACPSessionId(tid)
        tailProvider = provider
        let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber)?.uint64Value ?? 0
        tailOffset = size
        tailBuffer.removeAll(keepingCapacity: true)

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + .milliseconds(500), repeating: .milliseconds(500))
        timer.setEventHandler { [weak self] in self?.pollTail() }
        tailTimer = timer
        timer.resume()
        OpenAgentsLog.server.debug("tailer started file=\(file.path, privacy: .private) session=\(tid) provider=\(provider)")
    }

    func stopLiveTailer() {
        tailTimer?.cancel(); tailTimer = nil
        tailURL = nil; tailSessionId = nil; tailProvider = nil; tailOffset = 0; tailBuffer.removeAll()
        OpenAgentsLog.server.debug("tailer stopped")
    }

    func pollTail() {
        guard let url = tailURL, let sid = tailSessionId else { return }
        guard let fh = try? FileHandle(forReadingFrom: url) else { return }
        defer { try? fh.close() }
        let fileSize = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.uint64Value ?? 0
        if fileSize <= tailOffset { return }
        let toRead = Int(fileSize - tailOffset)
        do {
            try fh.seek(toOffset: tailOffset)
            let data = try fh.read(upToCount: toRead) ?? Data()
            tailOffset = fileSize
            if !data.isEmpty { tailBuffer.append(data) }
        } catch { return }
        var text = String(data: tailBuffer, encoding: .utf8) ?? String(decoding: tailBuffer, as: UTF8.self)
        var keepRemainder = Data()
        if !text.hasSuffix("\n"), let lastNewline = text.lastIndex(of: "\n") {
            let remainder = String(text[text.index(after: lastNewline)..<text.endIndex])
            keepRemainder = remainder.data(using: .utf8) ?? Data()
            text = String(text[..<text.index(after: lastNewline)])
        }
        let newLines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
        tailBuffer = keepRemainder
        if newLines.isEmpty { return }
        let updates = DesktopWebSocketServer.makeTypedUpdates(from: newLines, provider: tailProvider ?? "codex")
        guard !updates.isEmpty else { return }
        for u in updates { Task { await self.sendSessionUpdate(sessionId: sid, update: u) } }
    }
}

// MARK: - Tail utilities and translators
extension DesktopWebSocketServer {
    static func tailJSONLLines(at url: URL, maxBytes: Int, maxLines: Int) -> [String] {
        guard let fh = try? FileHandle(forReadingFrom: url) else { return [] }
        defer { try? fh.close() }
        let fileSize = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.uint64Value ?? 0
        let readBytes = min(Int(fileSize), maxBytes)
        let start = UInt64(max(Int(fileSize) - readBytes, 0))
        do {
            try fh.seek(toOffset: start)
            let buffer = try fh.read(upToCount: readBytes) ?? Data()
            var text = String(data: buffer, encoding: .utf8) ?? String(decoding: buffer, as: UTF8.self)
            if !text.hasSuffix("\n") { text.append("\n") }
            var lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
            if lines.count > maxLines { lines = Array(lines.suffix(maxLines)) }
            return lines
        } catch {
            return []
        }
    }

    static func pruneHeavyPayloads(in lines: [String]) -> [String] {
        return lines.map { line in
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return line }

            let pruned = pruneObject(obj)
            if let out = try? JSONSerialization.data(withJSONObject: pruned, options: []),
               let text = String(data: out, encoding: .utf8) {
                return text
            }
            return line
        }
    }

    static func capTotalBytes(_ lines: [String], limit: Int) -> [String] {
        var total = 0
        var kept: [String] = []
        for line in lines.reversed() {
            let bytes = line.utf8.count + 1 // include newline/commas overhead
            if total + bytes > limit { break }
            total += bytes
            kept.append(line)
        }
        return kept.reversed()
    }

    private static func pruneObject(_ obj: [String: Any]) -> [String: Any] {
        var out = obj

        // Handle JSON‑RPC session/update envelopes containing tool_call_update
        if let method = out["method"] as? String, method == ACPRPC.sessionUpdate,
           let params = out["params"] as? [String: Any] {
            var p = params
            if let upd = p["update"] as? [String: Any],
               let kind = upd["sessionUpdate"] as? String, kind == "tool_call_update" {
                if var tc = upd["tool_call_update"] as? [String: Any] {
                    if let output = tc["output"], let s = summarize(value: output) { tc["output"] = s }
                    p["update"] = merge(upd, ["tool_call_update": tc])
                }
            }
            out["params"] = pruneRecursive(p)
            return capStrings(out)
        }

        // Generic Codex item payloads under `item`, `msg`, or `payload`
        let keys = ["item", "msg", "payload"]
        for k in keys {
            if var inner = out[k] as? [String: Any] {
                inner = pruneToolShapes(inner)
                out[k] = pruneRecursive(inner)
            }
        }

        return capStrings(out)
    }

    private static func pruneToolShapes(_ obj: [String: Any]) -> [String: Any] {
        var out = obj
        if var o = out["tool_result"] as? [String: Any] {
            if let r = o["result"], let s = summarize(value: r) { o["result"] = s } else { o.removeValue(forKey: "result") }
            if let outp = o["output"], let s = summarize(value: outp) { o["output"] = s }
            out["tool_result"] = o
        }
        if var tc = out["tool_call_update"] as? [String: Any] {
            if let outv = tc["output"], let s = summarize(value: outv) { tc["output"] = s }
            out["tool_call_update"] = tc
        }
        return out
    }

    private static func pruneRecursive(_ any: Any) -> Any {
        if let dict = any as? [String: Any] {
            var out: [String: Any] = [:]
            for (k, v) in dict { out[k] = pruneRecursive(v) }
            return capStrings(out)
        }
        if let arr = any as? [Any] {
            return arr.map { pruneRecursive($0) }
        }
        if let s = any as? String {
            return capString(s)
        }
        return any
    }

    private static func summarize(value: Any) -> String? {
        if let s = value as? String { return capString(s) }
        if let a = value as? [Any] { return "array(len=\(a.count))" }
        if let d = value as? [String: Any] { return "object(keys=\(d.keys.count))" }
        return nil
    }

    private static func capStrings(_ dict: [String: Any]) -> [String: Any] {
        var out: [String: Any] = [:]
        for (k, v) in dict {
            if let s = v as? String { out[k] = capString(s) }
            else if let d = v as? [String: Any] { out[k] = capStrings(d) }
            else if let a = v as? [Any] { out[k] = a.map { pruneRecursive($0) } }
            else { out[k] = v }
        }
        return out
    }

    private static func capString(_ s: String) -> String {
        let limit = 4096
        if s.utf8.count <= limit { return s }
        let idx = s.index(s.startIndex, offsetBy: min(s.count, 3000))
        let prefix = String(s[..<idx])
        return prefix + "… (truncated)"
    }

    private static func merge(_ a: [String: Any], _ b: [String: Any]) -> [String: Any] {
        var out = a
        for (k,v) in b { out[k] = v }
        return out
    }

    // MARK: - Typed updates assembly (Provider JSONL -> ACP SessionUpdate)
    static func makeTypedUpdates(from lines: [String], provider: String = "codex") -> [ACP.Client.SessionUpdate] {
        var updates: [ACP.Client.SessionUpdate] = []

        // Claude Code uses a different structure - translate full conversation at once
        if provider == "claude-code" {
            let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "desktop"))
            for event in thread.events {
                // Map ACPEvent to SessionUpdate
                if let m = event.message {
                    let text = m.parts.compactMap { part -> String? in
                        if case let .text(tt) = part { return tt.text } else { return nil }
                    }.joined(separator: "\n")
                    guard !text.isEmpty else { continue }
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                    if m.role == .user {
                        updates.append(.userMessageChunk(chunk))
                    } else if m.role == .assistant {
                        // Check isThinking metadata to distinguish thinking from regular response
                        if m.isThinking == true {
                            updates.append(.agentThoughtChunk(chunk))
                        } else {
                            updates.append(.agentMessageChunk(chunk))
                        }
                    }
                }
                if let call = event.tool_call {
                    let args = jsonValueToAnyDict(call.arguments)
                    let wire = ACPToolCallWire(call_id: call.id, name: call.tool_name, arguments: args, _meta: nil)
                    updates.append(.toolCall(wire))
                }
                if let res = event.tool_result {
                    let status: ACPToolCallUpdateWire.Status = res.ok ? .completed : .error
                    let output = res.result.map { jsonValueToAnyEncodable($0) }
                    let wire = ACPToolCallUpdateWire(call_id: res.call_id, status: status, output: output, error: res.error, _meta: nil)
                    updates.append(.toolCallUpdate(wire))
                }
                if let ps = event.plan_state {
                    let entries = (ps.steps ?? []).map { ACPPlanEntry(content: $0, priority: .medium, status: .in_progress, _meta: nil) }
                    let plan = ACPPlan(entries: entries, _meta: nil)
                    updates.append(.plan(plan))
                }
            }
            return updates
        }

        // Codex: translate line-by-line (original logic)
        for line in lines {
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }
            // Reasoning first: emit agent_thought_chunk instead of assistant message
            if isReasoningLine(obj) {
                if let text = textFromTranslatedLine(line) {
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                    updates.append(.agentThoughtChunk(chunk))
                }
                continue
            }
            // For everything else, use translator then map to updates
            let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: "desktop"))
            if let m = t.events.compactMap({ $0.message }).first {
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(tt) = part { return tt.text } else { return nil }
                }.joined(separator: "\n")
                guard !text.isEmpty else { continue }
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                updates.append(m.role == .user ? .userMessageChunk(chunk) : .agentMessageChunk(chunk))
                continue
            }
            if let call = t.events.compactMap({ $0.tool_call }).first {
                let args = jsonValueToAnyDict(call.arguments)
                let wire = ACPToolCallWire(call_id: call.id, name: call.tool_name, arguments: args, _meta: nil)
                updates.append(.toolCall(wire))
                continue
            }
            if let res = t.events.compactMap({ $0.tool_result }).first {
                let status: ACPToolCallUpdateWire.Status = res.ok ? .completed : .error
                let output = res.result.map { jsonValueToAnyEncodable($0) }
                let wire = ACPToolCallUpdateWire(call_id: res.call_id, status: status, output: output, error: res.error, _meta: nil)
                updates.append(.toolCallUpdate(wire))
                continue
            }
            if let ps = t.events.compactMap({ $0.plan_state }).first {
                // Represent plan state as a full plan update (simplified): convert steps to ACPPlan
                let entries = (ps.steps ?? []).map { ACPPlanEntry(content: $0, priority: .medium, status: .in_progress, _meta: nil) }
                let plan = ACPPlan(entries: entries, _meta: nil)
                updates.append(.plan(plan))
                continue
            }
        }
        return updates
    }

    private static func isReasoningLine(_ obj: [String: Any]) -> Bool {
        let t = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        if t == "event_msg", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any]),
           ((item["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if t == "response_item", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "reasoning" { return true }
        return false
    }

    private static func textFromTranslatedLine(_ line: String) -> String? {
        let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: "desktop"))
        guard let m = t.events.compactMap({ $0.message }).first else { return nil }
        let text = m.parts.compactMap { part -> String? in
            if case let .text(tt) = part { return tt.text } else { return nil }
        }.joined(separator: "\n")
        return text.isEmpty ? nil : text
    }

    private static func jsonValueToAnyEncodable(_ v: JSONValue) -> AnyEncodable {
        switch v {
        case .string(let s): return AnyEncodable(s)
        case .number(let n): return AnyEncodable(n)
        case .bool(let b): return AnyEncodable(b)
        case .null: return AnyEncodable(Optional<String>.none as String?)
        case .array(let arr):
            let encArr: [AnyEncodable] = arr.map { jsonValueToAnyEncodable($0) }
            return AnyEncodable(encArr)
        case .object(let obj):
            var encDict: [String: AnyEncodable] = [:]
            for (k, vv) in obj { encDict[k] = jsonValueToAnyEncodable(vv) }
            return AnyEncodable(encDict)
        }
    }

    private static func jsonValueToAnyDict(_ v: JSONValue) -> [String: AnyEncodable]? {
        if case .object(let obj) = v {
            var out: [String: AnyEncodable] = [:]
            for (k, vv) in obj { out[k] = jsonValueToAnyEncodable(vv) }
            return out
        }
        return nil
    }
}
#endif

