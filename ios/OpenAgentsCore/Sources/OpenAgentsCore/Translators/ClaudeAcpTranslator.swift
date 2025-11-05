import Foundation

public enum ClaudeAcpTranslator {
    public struct Options {
        public var sourceId: String // e.g., file path or logical source key
        public init(sourceId: String) { self.sourceId = sourceId }
    }

    /// Translate Claude Code JSONL lines into an ACP thread timeline.
    /// Lines should be Claude Code native JSON objects, one per line.
    public static func translateLines(_ lines: [String], options: Options) -> ACPThread {
        var events: [ACPEvent] = []
        var threadId: String?
        var title: String?
        var tsMin: Int64?
        var tsMax: Int64?
        var nextSeq = 0

        func nextEventId() -> String {
            defer { nextSeq += 1 }
            return ACPId.stableId(namespace: "claude-event:\(options.sourceId)", seed: "\(nextSeq)")
        }

        func touchTs(_ t: Int64) {
            if tsMin == nil || t < tsMin! { tsMin = t }
            if tsMax == nil || t > tsMax! { tsMax = t }
        }

        for line in lines {
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }

            let type = obj["type"] as? String ?? ""
            let timestamp = obj["timestamp"] as? Double ?? 0
            let ts = Int64(timestamp) // Claude Code uses ms timestamps

            // Extract session ID from first line
            if threadId == nil, let sid = obj["sessionId"] as? String {
                threadId = sid
            }

            // Skip file history snapshots
            if type == "file-history-snapshot" {
                continue
            }

            guard let message = obj["message"] as? [String: Any] else { continue }
            let role = message["role"] as? String ?? ""

            // User message
            if type == "user" {
                // Check if this is a tool result (array with tool_result blocks)
                if let contentArray = message["content"] as? [[String: Any]],
                   let firstBlock = contentArray.first,
                   firstBlock["type"] as? String == "tool_result" {
                    // Parse tool result
                    let toolUseId = firstBlock["tool_use_id"] as? String ?? ""
                    let content = firstBlock["content"] as? String ?? ""
                    let isError = firstBlock["is_error"] as? Bool ?? false

                    touchTs(ts)
                    let result = ACPToolResult(
                        call_id: toolUseId,
                        ok: !isError,
                        result: .string(content),
                        error: isError ? content : nil,
                        ts: ts
                    )
                    events.append(ACPEvent(id: nextEventId(), ts: ts, tool_result: result))
                } else if let content = message["content"] as? String {
                    // Regular user message
                    let parts: [ACPContentPart] = [.text(ACPText(text: content))]
                    touchTs(ts)
                    let mid = ACPId.stableId(namespace: "claude-msg:\(options.sourceId)", seed: "u:\(nextSeq)")
                    let msg = ACPMessage(id: mid, thread_id: threadId, role: .user, parts: parts, ts: ts)
                    events.append(ACPEvent(id: nextEventId(), ts: ts, message: msg))

                    // Extract title from first user message
                    if title == nil {
                        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
                        let words = trimmed.split(separator: " ", maxSplits: 10)
                        let preview = words.prefix(8).joined(separator: " ")
                        title = preview.count > 60 ? String(preview.prefix(60)) + "..." : preview
                    }
                }
                continue
            }

            // Assistant message
            if type == "assistant" {
                guard let contentArray = message["content"] as? [[String: Any]] else { continue }

                for contentBlock in contentArray {
                    let blockType = contentBlock["type"] as? String ?? ""

                    switch blockType {
                    case "thinking":
                        // Reasoning/thought chunk - emit as agent thought (not shown in main timeline)
                        if let thinkingText = contentBlock["thinking"] as? String {
                            let parts: [ACPContentPart] = [.text(ACPText(text: thinkingText))]
                            touchTs(ts)
                            let mid = ACPId.stableId(namespace: "claude-msg:\(options.sourceId)", seed: "think:\(nextSeq)")
                            let msg = ACPMessage(id: mid, thread_id: threadId, role: .assistant, parts: parts, ts: ts)
                            events.append(ACPEvent(id: nextEventId(), ts: ts, message: msg, isReasoning: true))
                        }

                    case "text":
                        // Text response
                        if let text = contentBlock["text"] as? String, !text.isEmpty {
                            let parts: [ACPContentPart] = [.text(ACPText(text: text))]
                            touchTs(ts)
                            let mid = ACPId.stableId(namespace: "claude-msg:\(options.sourceId)", seed: "a:\(nextSeq)")
                            let msg = ACPMessage(id: mid, thread_id: threadId, role: .assistant, parts: parts, ts: ts)
                            events.append(ACPEvent(id: nextEventId(), ts: ts, message: msg))
                        }

                    case "tool_use":
                        // Tool call
                        if let toolId = contentBlock["id"] as? String,
                           let toolName = contentBlock["name"] as? String,
                           let input = contentBlock["input"] as? [String: Any] {
                            touchTs(ts)
                            let argsJV = Self.jsonValue(from: input)
                            let call = ACPToolCall(id: toolId, tool_name: toolName, arguments: argsJV, ts: ts)
                            events.append(ACPEvent(id: nextEventId(), ts: ts, tool_call: call))
                        }

                    default:
                        // Unknown block type - skip
                        break
                    }
                }
                continue
            }
        }

        // Build thread
        let tid = threadId ?? ACPId.stableId(namespace: "claude-thread", seed: options.sourceId)
        let threadTitle = title ?? "Untitled"
        let created = tsMin ?? 0
        let updated = tsMax ?? created

        return ACPThread(
            id: tid,
            title: threadTitle,
            source: "claude-code",
            created_at: created,
            updated_at: updated,
            events: events
        )
    }

    // MARK: - Helper Methods

    /// Convert Any to JSONValue
    private static func jsonValue(from obj: Any?) -> JSONValue {
        guard let obj = obj else { return .null }

        if let s = obj as? String { return .string(s) }
        if let n = obj as? NSNumber {
            if n.isBool { return .bool(n.boolValue) }
            return .number(n.doubleValue)
        }
        if let arr = obj as? [Any] {
            return .array(arr.map { jsonValue(from: $0) })
        }
        if let dict = obj as? [String: Any] {
            var out: [String: JSONValue] = [:]
            for (k, v) in dict { out[k] = jsonValue(from: v) }
            return .object(out)
        }
        return .null
    }
}

// Extension to check if NSNumber is bool
private extension NSNumber {
    var isBool: Bool {
        return String(cString: self.objCType) == "c"
    }
}

// Extension to ACPEvent to track reasoning
extension ACPEvent {
    init(id: String, ts: Int64, message: ACPMessage? = nil, tool_call: ACPToolCall? = nil, tool_result: ACPToolResult? = nil, plan_state: ACPPlanState? = nil, isReasoning: Bool = false) {
        self.id = id
        self.ts = ts
        self.message = message
        self.tool_call = tool_call
        self.tool_result = tool_result
        self.plan_state = plan_state
        // Note: isReasoning flag could be added to ACPEvent if needed for UI filtering
    }
}
