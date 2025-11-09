import Foundation
@testable import OpenAgentsCore

enum TestHelpers {
    static func makeTextUpdate(text: String) -> ACP.Client.SessionUpdate {
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
        return .agentMessageChunk(chunk)
    }

    static func makeAvailableCommandsUpdate(commands: [ACP.Client.AvailableCommand]) -> ACP.Client.SessionUpdate {
        let ac = ACP.Client.AvailableCommandsUpdate(available_commands: commands)
        return .availableCommandsUpdate(ac)
    }

    static func makeSessionUpdateNotification(sessionId: String = "test", update: ACP.Client.SessionUpdate) -> ACP.Client.SessionNotificationWire {
        ACP.Client.SessionNotificationWire(session_id: ACPSessionId(sessionId), update: update)
    }
}

