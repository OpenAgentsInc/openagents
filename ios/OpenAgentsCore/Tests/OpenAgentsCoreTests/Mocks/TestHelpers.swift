import Foundation
@testable import OpenAgentsCore

/// Helper utilities for generating test data
enum TestHelpers {
    /// Creates a test session update notification
    static func makeSessionUpdateNotification(
        sessionId: String = "test-session-123",
        update: ACP.Client.SessionUpdate
    ) -> ACP.Client.SessionNotificationWire {
        return ACP.Client.SessionNotificationWire(
            session_id: ACPSessionId(sessionId),
            update: update
        )
    }

    /// Creates a test text content update
    static func makeTextUpdate(text: String, role: ACP.Client.Role = .assistant) -> ACP.Client.SessionUpdate {
        let content = ACP.Client.ContentBlock.text(.init(text: text))
        let message = ACP.Client.Message(role: role, content: [content])
        return .messageUpdated(.init(message: message))
    }

    /// Creates a test tool call update
    static func makeToolCallUpdate(
        toolName: String = "Bash",
        arguments: [String: AnyEncodable] = [:]
    ) -> ACP.Client.SessionUpdate {
        let toolUse = ACP.Client.ToolUse(
            id: ACP.ToolUseId("tool-\(UUID().uuidString)"),
            name: toolName,
            arguments: arguments
        )
        let content = ACP.Client.ContentBlock.toolUse(toolUse)
        let message = ACP.Client.Message(role: .assistant, content: [content])
        return .messageUpdated(.init(message: message))
    }

    /// Creates a test tool result update
    static func makeToolResultUpdate(
        toolUseId: String = "tool-123",
        result: String = "success"
    ) -> ACP.Client.SessionUpdate {
        let toolResult = ACP.Client.ToolResult(
            tool_use_id: ACP.ToolUseId(toolUseId),
            content: [.text(.init(text: result))]
        )
        let content = ACP.Client.ContentBlock.toolResult(toolResult)
        let message = ACP.Client.Message(role: .user, content: [content])
        return .messageUpdated(.init(message: message))
    }

    /// Creates a test thinking block update
    static func makeThinkingUpdate(thinking: String) -> ACP.Client.SessionUpdate {
        let thinkingBlock = ACP.Client.Thinking(thinking: thinking)
        let content = ACP.Client.ContentBlock.thinking(thinkingBlock)
        let message = ACP.Client.Message(role: .assistant, content: [content])
        return .messageUpdated(.init(message: message))
    }

    /// Creates a test available commands update
    static func makeAvailableCommandsUpdate(commands: [ACP.Client.AvailableCommand] = []) -> ACP.Client.SessionUpdate {
        return .availableCommandsUpdate(.init(available_commands: commands))
    }

    /// Creates a test current mode update
    static func makeCurrentModeUpdate(mode: ACPSessionModeId = .default_mode) -> ACP.Client.SessionUpdate {
        return .currentModeUpdate(.init(current_mode_id: mode))
    }

    /// Creates a test JSON-RPC notification payload
    static func makeJSONRPCNotification(method: String, params: Encodable) throws -> Data {
        let encoder = JSONEncoder()
        return try encoder.encode(params)
    }

    /// Creates a test AnyEncodable dictionary for tool arguments
    static func makeToolArguments(_ dict: [String: Any]) -> [String: AnyEncodable] {
        var result: [String: AnyEncodable] = [:]
        for (key, value) in dict {
            if let v = value as? String {
                result[key] = AnyEncodable(v)
            } else if let v = value as? Int {
                result[key] = AnyEncodable(v)
            } else if let v = value as? Bool {
                result[key] = AnyEncodable(v)
            } else if let v = value as? Double {
                result[key] = AnyEncodable(v)
            }
        }
        return result
    }

    /// Waits for a condition to become true with timeout
    static func waitFor(
        timeout: TimeInterval = 2.0,
        pollingInterval: TimeInterval = 0.1,
        condition: () -> Bool
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if condition() {
                return true
            }
            RunLoop.current.run(until: Date().addingTimeInterval(pollingInterval))
        }
        return condition()
    }
}
