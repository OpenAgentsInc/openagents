import Foundation
import Combine
import OpenAgentsCore

final class TimelineStore: ObservableObject, TimelineStoring {
    // Published state
    @Published private(set) var updates: [ACP.Client.SessionNotificationWire] = []
    @Published private(set) var availableCommands: [ACP.Client.AvailableCommand] = []
    @Published private(set) var currentMode: ACPSessionModeId = .default_mode
    @Published private(set) var toolCallNames: [String: String] = [:]
    @Published private(set) var rawJSONByCallId: [String: String] = [:]
    @Published private(set) var outputJSONByCallId: [String: String] = [:]

    // Internal indices for coalescing tool_call_update events
    private var lastUpdateRowIndexByCallId: [String: Int] = [:]

    // Expose publishers for coordinator to subscribe and mirror if needed
    var updatesPublisher: AnyPublisher<[ACP.Client.SessionNotificationWire], Never> { $updates.eraseToAnyPublisher() }
    var availableCommandsPublisher: AnyPublisher<[ACP.Client.AvailableCommand], Never> { $availableCommands.eraseToAnyPublisher() }
    var currentModePublisher: AnyPublisher<ACPSessionModeId, Never> { $currentMode.eraseToAnyPublisher() }
    var toolCallNamesPublisher: AnyPublisher<[String: String], Never> { $toolCallNames.eraseToAnyPublisher() }
    var rawJSONByCallIdPublisher: AnyPublisher<[String: String], Never> { $rawJSONByCallId.eraseToAnyPublisher() }
    var outputJSONByCallIdPublisher: AnyPublisher<[String: String], Never> { $outputJSONByCallId.eraseToAnyPublisher() }

    private let ringLimit = 200

    func clearAll() {
        updates.removeAll()
        availableCommands.removeAll()
        currentMode = .default_mode
        toolCallNames.removeAll()
        rawJSONByCallId.removeAll()
        outputJSONByCallId.removeAll()
        lastUpdateRowIndexByCallId.removeAll()
    }

    func replaceAll(with updates: [ACP.Client.SessionNotificationWire]) {
        self.updates = updates
        // Reset indices map; rebuild indices for tool_call_update if desired (skip for simplicity)
        lastUpdateRowIndexByCallId.removeAll()
    }

    func appendOptimisticUserMessage(text: String, sessionId: ACPSessionId) -> Int {
        let userChunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
        let userUpdate = ACP.Client.SessionUpdate.userMessageChunk(userChunk)
        let optimistic = ACP.Client.SessionNotificationWire(session_id: sessionId, update: userUpdate)
        pushUpdate(optimistic)
        return updates.count - 1
    }

    func applySessionUpdatePayload(_ payload: Data) {
        guard let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: payload) else { return }
        switch note.update {
        case .toolCall(let call):
            toolCallNames[call.call_id] = call.name
            pushUpdate(note)
        case .toolCallUpdate(let upd):
            if let s = String(data: payload, encoding: .utf8) { rawJSONByCallId[upd.call_id] = s }
            if let out = Self.extractOutputJSON(from: payload) { outputJSONByCallId[upd.call_id] = out }
            if let idx = lastUpdateRowIndexByCallId[upd.call_id], idx < updates.count {
                updates[idx] = note
                objectWillChange.send()
            } else {
                pushUpdate(note)
                lastUpdateRowIndexByCallId[upd.call_id] = updates.count - 1
            }
        case .agentMessageChunk(let chunk):
            // Coalesce streaming agent chunks into the last agent bubble for this session
            if let last = updates.last, last.session_id == note.session_id {
                if case .agentMessageChunk(let prevChunk) = last.update,
                   case .text(let prevText) = prevChunk.content,
                   case .text(let newText) = chunk.content {
                    // Concatenate with newline separator to preserve line breaks
                    let merged = ACP.Client.ContentChunk(content: .text(.init(text: prevText.text + "\n" + newText.text)))
                    let mergedNote = ACP.Client.SessionNotificationWire(session_id: note.session_id, update: .agentMessageChunk(merged))
                    updates[updates.count - 1] = mergedNote
                    objectWillChange.send()
                } else {
                    pushUpdate(note)
                }
            } else {
                pushUpdate(note)
            }
        default:
            pushUpdate(note)
        }

        // Side-channel updates used by UI
        switch note.update {
        case .availableCommandsUpdate(let ac):
            availableCommands = ac.available_commands
        case .currentModeUpdate(let cm):
            currentMode = cm.current_mode_id
        default:
            break
        }
    }

    private func pushUpdate(_ note: ACP.Client.SessionNotificationWire) {
        if updates.count >= ringLimit {
            updates.removeFirst()
            // adjust coalescing indices
            var newMap: [String: Int] = [:]
            for (k, v) in lastUpdateRowIndexByCallId where v > 0 { newMap[k] = v - 1 }
            lastUpdateRowIndexByCallId = newMap
        }
        updates.append(note)
        objectWillChange.send() // force notify observers for ring-buffer boundary
    }

    // Local JSON helper (mirrors BridgeManager.extractOutputJSON)
    static func extractOutputJSON(from payload: Data) -> String? {
        guard let obj = try? JSONSerialization.jsonObject(with: payload) as? [String: Any],
              let params = obj["params"] as? [String: Any],
              let update = params["update"] as? [String: Any],
              let kind = update["sessionUpdate"] as? String, kind == "tool_call_update",
              let tcu = update["tool_call_update"] as? [String: Any],
              let output = tcu["output"]
        else { return nil }
        if let data = try? JSONSerialization.data(withJSONObject: output, options: [.prettyPrinted, .sortedKeys]),
           let text = String(data: data, encoding: .utf8) {
            return text
        }
        return nil
    }
}
