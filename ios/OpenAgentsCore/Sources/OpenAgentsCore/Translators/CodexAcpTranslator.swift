import Foundation

public enum CodexAcpTranslator {
    public struct Options {
        public var sourceId: String // e.g., file path or logical source key
        public init(sourceId: String) { self.sourceId = sourceId }
    }

    /// Translate Codex JSONL lines into an ACP thread timeline.
    /// Lines should be provider-native JSON objects, one per line.
    public static func translateLines(_ lines: [String], options: Options) -> ACPThread {
        var events: [ACPEvent] = []
        var threadId: String?
        var title: String?
        var tsMin: Int64?
        var tsMax: Int64?
        var callIdsSeen: Set<String> = []
        var nextSeq = 0

        func nextEventId() -> String {
            defer { nextSeq += 1 }
            return ACPId.stableId(namespace: "codex-event:\(options.sourceId)", seed: "\(nextSeq)")
        }
        func stableCallId(seed: String) -> String {
            return ACPId.stableId(namespace: "codex-call:\(options.sourceId)", seed: seed)
        }
        func touchTs(_ t: Int64) { if tsMin == nil || t < tsMin! { tsMin = t }; if tsMax == nil || t > tsMax! { tsMax = t } }

        for line in lines {
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }

            let type = (obj["type"] as? String) ?? (obj["event"] as? String) ?? ""
            let ts = Self.int64(obj["ts"]) ?? Self.int64((obj["payload"] as? [String: Any])?["ts"]) // optional

            // Session/thread metadata
            if type == "session_meta" {
                if let payload = obj["payload"] as? [String: Any] {
                    if let tid = payload["id"] as? String { threadId = tid }
                    if let t = payload["title"] as? String { title = t }
                    if let ct = Self.int64(payload["created_at"]) { touchTs(ct) }
                }
                if let t = ts { touchTs(t) }
                continue
            }
            if type == "thread.started" {
                if let tid = obj["thread_id"] as? String { threadId = tid }
                if let t = ts { touchTs(t) }
                continue
            }

            // Normalized container: many Codex lines carry an `item` or `msg` payload
            let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any])
            let itemType = item?["type"] as? String ?? ""

            // Messages
            if itemType == "user_message" || type == "user_message" || Self.asRole(item?["role"]) == .user {
                let text = Self.extractText(from: item)
                let parts: [ACPContentPart] = text.map { [.text(ACPText(text: $0))] } ?? []
                if !parts.isEmpty {
                    let t = ts ?? Self.int64(item?["ts"]) ?? 0
                    touchTs(t)
                    let mid = ACPId.stableId(namespace: "codex-msg:\(options.sourceId)", seed: "u:\(nextSeq)")
                    let msg = ACPMessage(id: mid, thread_id: threadId, role: .user, parts: parts, ts: t)
                    events.append(ACPEvent(id: nextEventId(), ts: t, message: msg))
                }
                continue
            }
            if itemType == "agent_message" || itemType == "assistant_message" || type == "agent_message" || Self.asRole(item?["role"]) == .assistant {
                let text = Self.extractText(from: item)
                let parts: [ACPContentPart] = text.map { [.text(ACPText(text: $0))] } ?? []
                if !parts.isEmpty {
                    let t = ts ?? Self.int64(item?["ts"]) ?? 0
                    touchTs(t)
                    let mid = ACPId.stableId(namespace: "codex-msg:\(options.sourceId)", seed: "a:\(nextSeq)")
                    let msg = ACPMessage(id: mid, thread_id: threadId, role: .assistant, parts: parts, ts: t)
                    events.append(ACPEvent(id: nextEventId(), ts: t, message: msg))
                }
                continue
            }

            // Tool calls
            if itemType == "tool_call" || type == "tool_call" {
                let tool = (item?["tool_name"] as? String) ?? (item?["tool"] as? String) ?? "tool"
                let argsJV = Self.jsonValue(from: item?["arguments"]) ?? Self.jsonValue(from: item?["args"]) ?? .object([:])
                var callId = (item?["id"] as? String) ?? (item?["call_id"] as? String)
                if callId == nil { callId = stableCallId(seed: "\(nextSeq):\(tool)") }
                if let cid = callId {
                    callIdsSeen.insert(cid)
                    let t = ts ?? Self.int64(item?["ts"]) ?? 0
                    touchTs(t)
                    let call = ACPToolCall(id: cid, tool_name: tool, arguments: argsJV, ts: t)
                    events.append(ACPEvent(id: nextEventId(), ts: t, tool_call: call))
                }
                continue
            }

            // Tool results
            if itemType == "tool_result" || type == "tool_result" {
                let callId = (item?["call_id"] as? String) ?? (item?["id"] as? String) ?? stableCallId(seed: "\(nextSeq):res")
                let ok = (item?["ok"] as? Bool) ?? (item?["success"] as? Bool) ?? true
                let result = Self.jsonValue(from: item?["result"]) ?? Self.jsonValue(from: item?["output"]) ?? Self.jsonValue(from: item?["data"])
                let err = item?["error"] as? String
                let t = ts ?? Self.int64(item?["ts"]) ?? 0
                touchTs(t)
                let res = ACPToolResult(call_id: callId, ok: ok, result: result, error: err, ts: t)
                events.append(ACPEvent(id: nextEventId(), ts: t, tool_result: res))
                continue
            }

            // Plan state updates
            if itemType == "plan_state" || type == "plan_state" || type == "plan.updated" {
                let statusStr = (item?["status"] as? String) ?? (item?["state"] as? String) ?? "running"
                let status = ACPPlanStatus(rawValue: statusStr) ?? .running
                let summary = item?["summary"] as? String
                let steps = item?["steps"] as? [String]
                let t = ts ?? Self.int64(item?["ts"]) ?? 0
                touchTs(t)
                let ps = ACPPlanState(status: status, summary: summary, steps: steps, ts: t)
                events.append(ACPEvent(id: nextEventId(), ts: t, plan_state: ps))
                continue
            }
        }

        // Sort by timestamp asc to form a timeline.
        events.sort { $0.ts < $1.ts }
        let tid = threadId ?? ACPId.stableId(namespace: "codex-thread", seed: options.sourceId)
        let thread = ACPThread(id: tid, title: title, created_at: tsMin, updated_at: tsMax, events: events)
        return thread
    }

    // MARK: - Helpers
    static func int64(_ any: Any?) -> Int64? {
        if let i = any as? Int { return Int64(i) }
        if let d = any as? Double { return Int64(d) }
        if let s = any as? String, let d = Double(s) { return Int64(d) }
        return nil
    }
    static func asRole(_ any: Any?) -> ACPRole? {
        guard let s = any as? String else { return nil }
        return ACPRole(rawValue: s)
    }
    static func extractText(from item: [String: Any]?) -> String? {
        guard let item = item else { return nil }
        if let t = item["text"] as? String { return t }
        if let content = item["content"] as? String { return content }
        if let parts = item["parts"] as? [String] { return parts.joined(separator: "\n") }
        if let blocks = item["content"] as? [[String: Any]] {
            var buf: [String] = []
            for b in blocks {
                if let t = b["text"] as? String { buf.append(t) }
                else if let t = b["content"] as? String { buf.append(t) }
            }
            if !buf.isEmpty { return buf.joined(separator: "\n") }
        }
        return nil
    }
    static func jsonValue(from any: Any?) -> JSONValue? {
        guard let any = any else { return nil }
        if let s = any as? String { return .string(s) }
        if let b = any as? Bool { return .bool(b) }
        if let i = any as? Int { return .number(Double(i)) }
        if let d = any as? Double { return .number(d) }
        if let arr = any as? [Any] { return .array(arr.compactMap { jsonValue(from: $0) }) }
        if let dict = any as? [String: Any] {
            var obj: [String: JSONValue] = [:]
            for (k, v) in dict { if let jv = jsonValue(from: v) { obj[k] = jv } }
            return .object(obj)
        }
        return nil
    }
}

