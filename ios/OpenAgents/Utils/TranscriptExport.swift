import Foundation
import OpenAgentsCore

struct TranscriptExport {
    static func exportJSONData(updates: [ACP.Client.SessionNotificationWire]) throws -> Data {
        try JSONEncoder().encode(updates)
    }

    static func exportMarkdown(updates: [ACP.Client.SessionNotificationWire]) -> String {
        var out: [String] = ["# OpenAgents Transcript\n"]
        for note in updates {
            switch note.update {
            case .userMessageChunk(let chunk):
                if case .text(let t) = chunk.content { out.append("\n**User**\n\n" + t.text + "\n") }
            case .agentMessageChunk(let chunk):
                if case .text(let t) = chunk.content { out.append("\n**Assistant**\n\n" + t.text + "\n") }
            case .agentThoughtChunk(let chunk):
                if case .text(let t) = chunk.content { out.append("\n> _Thinking:_ " + t.text.replacingOccurrences(of: "\n", with: " ") + "\n") }
            case .plan(let plan):
                let bullets = plan.entries.map { "- " + $0.content }.joined(separator: "\n")
                out.append("\n**Plan**\n\n" + bullets + "\n")
            case .toolCall(let call):
                out.append("\n`tool_call` \(call.name) id=\(call.call_id)\n")
            case .toolCallUpdate(let upd):
                out.append("`tool_call_update` id=\(upd.call_id) status=\(upd.status.rawValue)\n")
            case .availableCommandsUpdate, .currentModeUpdate:
                continue
            }
        }
        return out.joined(separator: "\n")
    }
}

