import Foundation
import OpenAgentsCore

/// Pretty-print a session notification as JSON for inspection.
func toPrettyJSON(_ note: ACP.Client.SessionNotificationWire) -> String? {
    let enc = JSONEncoder()
    enc.outputFormatting = [.prettyPrinted, .sortedKeys]
    guard let data = try? enc.encode(note) else { return nil }
    return String(data: data, encoding: .utf8)
}

/// Extract a tool call id from a session notification, if present.
func callId(from note: ACP.Client.SessionNotificationWire) -> String? {
    switch note.update {
    case .toolCall(let c): return c.call_id
    case .toolCallUpdate(let u): return u.call_id
    default: return nil
    }
}

