import Foundation
import FoundationModels

/// Actor that manages tool registrations per session
actor ToolRegistry {
    private var toolsBySession: [String: [ToolDefinition]] = [:]

    /// Register tools for a session
    func registerTools(sessionId: String, tools: [ToolDefinition]) {
        toolsBySession[sessionId] = tools
    }

    /// Get tools for a session
    func getTools(sessionId: String) -> [ToolDefinition]? {
        return toolsBySession[sessionId]
    }

    /// Remove tools from a session
    func removeTools(sessionId: String) {
        toolsBySession.removeValue(forKey: sessionId)
    }

    /// Get Foundation Models tool instances for a session
    /// Note: This is a placeholder - actual tool conversion would need Apple's Tool protocol
    func getFoundationModelsTools(sessionId: String) throws -> [any Sendable] {
        // For now, return empty array since we can't easily create Tool instances
        // Tool support would require more complex integration
        return []
    }

    /// Tool definition that can be registered
    struct ToolDefinition: Codable {
        let name: String
        let description: String
        let parameters: ToolParameters?

        struct ToolParameters: Codable {
            let type: String
            let properties: [String: ToolProperty]?
            let required: [String]?
        }

        struct ToolProperty: Codable {
            let type: String
            let description: String?
            let enumValues: [String]?

            enum CodingKeys: String, CodingKey {
                case type
                case description
                case enumValues = "enum"
            }
        }
    }
}

/// Tool call from model
struct ToolCall: Codable {
    let id: String
    let type: String
    let function: ToolCallFunction

    struct ToolCallFunction: Codable {
        let name: String
        let arguments: String
    }
}

/// Response to a tool call
struct ToolCallResponse: Codable {
    let toolCallId: String
    let content: String

    enum CodingKeys: String, CodingKey {
        case toolCallId = "tool_call_id"
        case content
    }
}
