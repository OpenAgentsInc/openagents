import Foundation
import Combine
import OpenAgentsCore

@MainActor
final class ACPTimelineViewModel: ObservableObject {
    enum Item: Identifiable {
        case message(role: Role, text: String, ts: Int64)
        case toolCall(ACPToolCall)
        case toolResult(ACPToolResult)
        case reasoning(ReasoningSummary)
        case plan(ACPPlan, ts: Int64)

        enum Role { case user, assistant }

        var id: String {
            switch self {
            case .message(let role, let text, let ts):
                return "msg_\(role == .user ? "u" : "a")_\(ts)_\(text.hashValue)"
            case .toolCall(let call):
                return "call_\(call.id)_\(call.ts ?? 0)"
            case .toolResult(let result):
                return "res_\(result.call_id)_\(result.ts ?? 0)"
            case .reasoning(let summary):
                return "reasoning_\(summary.id)"
            case .plan(_, let ts):
                return "plan_\(ts)"
            }
        }
    }

    @Published private(set) var items: [Item] = []
    @Published private(set) var latestPlan: ACPPlan?

    /// Optional callback when title changes (extracted from first user message)
    var onTitleChange: ((String) -> Void)?

    private var cancellables: Set<AnyCancellable> = []
    private weak var bridge: BridgeManager?
    private var isAttached = false
    private var lastTitle: String = ""

    // Reasoning consolidation state
    private var pendingThoughts: [String] = []
    private var pendingThoughtsStart: Int64?

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
        var latestPlanUpdate: ACPPlan?

        // Reset reasoning state
        pendingThoughts = []
        pendingThoughtsStart = nil

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
                // Flush any pending reasoning before user message
                flushPendingReasoning(to: &out, endTs: monoMs)

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
                // Flush any pending reasoning before agent message
                flushPendingReasoning(to: &out, endTs: monoMs)

                if case let .text(t) = chunk.content {
                    out.append(.message(role: .assistant, text: t.text, ts: monoMs))
                }

            case .agentThoughtChunk(let chunk):
                // Accumulate thoughts for consolidation
                if case let .text(t) = chunk.content {
                    if pendingThoughtsStart == nil {
                        pendingThoughtsStart = monoMs
                    }
                    pendingThoughts.append(t.text)

                    // If 5 minutes elapsed, flush
                    if let start = pendingThoughtsStart, (monoMs - start) >= 300_000 {
                        flushPendingReasoning(to: &out, endTs: monoMs)
                    }
                }

            case .plan(let planWire):
                // Flush any pending reasoning before plan
                flushPendingReasoning(to: &out, endTs: monoMs)

                // Store plan and add to timeline
                out.append(.plan(planWire, ts: monoMs))
                latestPlanUpdate = planWire

            case .toolCall(let wire):
                // Flush any pending reasoning before tool call
                flushPendingReasoning(to: &out, endTs: monoMs)

                // Don't show TodoWrite as a tool call (server converts to plan)
                if wire.name.lowercased() == "todowrite" {
                    break
                }

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
                    out.append(.toolCall(call))
                }

            case .toolCallUpdate(let upd):
                // Flush any pending reasoning before tool result
                flushPendingReasoning(to: &out, endTs: monoMs)

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
                continue // Ignore other types
            }
        }

        // Flush any remaining pending reasoning at end
        flushPendingReasoning(to: &out, endTs: monoMs)

        // Keep bounded (200 like bridge ring buffer)
        if out.count > 200 { out = Array(out.suffix(200)) }
        self.items = out
        self.latestPlan = latestPlanUpdate

        // Extract title from first user message
        extractAndNotifyTitle(from: out)
    }

    private func flushPendingReasoning(to items: inout [Item], endTs: Int64) {
        guard !pendingThoughts.isEmpty, let start = pendingThoughtsStart else { return }

        let summary = ReasoningSummary(
            startTs: start,
            endTs: endTs,
            thoughts: pendingThoughts
        )
        items.append(.reasoning(summary))

        // Clear pending state
        pendingThoughts = []
        pendingThoughtsStart = nil
    }

    private func extractAndNotifyTitle(from items: [Item]) {
        // Find first user message
        for item in items {
            if case let .message(role, text, _) = item, role == .user {
                let title = text.prefix(50).trimmingCharacters(in: .whitespacesAndNewlines)
                if title != lastTitle {
                    lastTitle = title
                    onTitleChange?(String(title))
                }
                return
            }
        }

        // No user message yet - use default
        if lastTitle != "New Chat" {
            lastTitle = "New Chat"
            onTitleChange?("New Chat")
        }
    }
}
