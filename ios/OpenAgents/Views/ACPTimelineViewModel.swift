import Foundation
import Combine
import OpenAgentsCore

#if os(iOS)

@MainActor
final class ACPTimelineViewModel: ObservableObject {
    enum Item: Identifiable, Equatable {
        case message(role: Role, text: String, ts: Int64)
        case toolCall(ACPToolCall)
        case toolResult(ACPToolResult)

        enum Role { case user, assistant }

        var id: String {
            switch self {
            case .message(let role, let text, let ts):
                return "msg_\(role == .user ? "u" : "a")_\(ts)_\(text.hashValue)"
            case .toolCall(let call):
                return "call_\(call.id)_\(call.ts ?? 0)"
            case .toolResult(let result):
                return "res_\(result.call_id)_\(result.ts ?? 0)"
            }
        }
    }

    @Published private(set) var items: [Item] = []

    private var cancellables: Set<AnyCancellable> = []
    private weak var bridge: BridgeManager?
    private var isAttached = false

    func attach(bridge: BridgeManager) {
        guard !isAttached else { return }
        isAttached = true
        self.bridge = bridge

        // Recompute whenever updates change
        bridge.$updates
            .receive(on: DispatchQueue.main)
            .sink { [weak self] updates in
                guard let self = self else { return }
                self.recompute(from: updates, currentSession: bridge.currentSessionId)
            }
            .store(in: &cancellables)

        // Also refresh on session switch
        bridge.$currentSessionId
            .receive(on: DispatchQueue.main)
            .sink { [weak self] sid in
                guard let self = self else { return }
                self.recompute(from: bridge.updates, currentSession: sid)
            }
            .store(in: &cancellables)
    }

    private func recompute(from updates: [ACP.Client.SessionNotificationWire], currentSession: ACPSessionId?) {
        var out: [Item] = []
        var monoMs: Int64 = 0
        var seenCalls: Set<String> = []
        var seenResults: Set<String> = []

        let filtered: [ACP.Client.SessionNotificationWire]
        if let sid = currentSession {
            filtered = updates.filter { $0.session_id == sid }
        } else {
            // No active session yet: show only optimistic local echoes (session_id == "pending").
            filtered = updates.filter { $0.session_id.value == "pending" }
        }


        for note in filtered {
            monoMs += 1000
            switch note.update {
            case .userMessageChunk(let chunk):
                if case let .text(t) = chunk.content {
                    let trimmed = t.text.trimmingCharacters(in: .whitespacesAndNewlines)
                    // Filter a spurious first message "Warmup" sometimes emitted by providers/boot paths
                    // Only drop if it's the very first item to avoid hiding legitimate content
                    let lower = trimmed.lowercased()
                    if out.isEmpty && (lower == "warmup" || lower == "warm up") {
                        break
                    }
                    out.append(.message(role: .user, text: trimmed, ts: monoMs))
                }
            case .agentMessageChunk(let chunk):
                if case let .text(t) = chunk.content {
                    out.append(.message(role: .assistant, text: t.text, ts: monoMs))
                }
            case .toolCall(let wire):
                // Only add each tool call once
                if !seenCalls.contains(wire.call_id) {
                    seenCalls.insert(wire.call_id)
                    let argsJV: JSONValue = wire.arguments.map { dict in
                        var out: [String: JSONValue] = [:]
                        for (k, v) in dict {
                            out[k] = v.toJSONValue()
                        }
                        return JSONValue.object(out)
                    } ?? .object([:])
                    let call = ACPToolCall(id: wire.call_id, tool_name: wire.name, arguments: argsJV, ts: monoMs)
                    // Don't show TodoWrite as a tool call (converted to plan elsewhere)
                    if wire.name.lowercased() != "todowrite" {
                        out.append(.toolCall(call))
                    }
                }
            case .toolCallUpdate(let upd):
                // Only add each result once per status
                let key = "\(upd.call_id)|\(upd.status.rawValue)"
                if !seenResults.contains(key) {
                    seenResults.insert(key)
                    let result = ACPToolResult(
                        call_id: upd.call_id,
                        ok: upd.status == .completed,
                        result: upd.output?.toJSONValue(),
                        error: upd.error,
                        ts: monoMs
                    )
                    out.append(.toolResult(result))
                }
            default:
                continue // Ignore other types for simplified view
            }
        }

        // Keep bounded (200 like bridge ring buffer)
        if out.count > 200 { out = Array(out.suffix(200)) }
        self.items = out
    }
}

#endif
