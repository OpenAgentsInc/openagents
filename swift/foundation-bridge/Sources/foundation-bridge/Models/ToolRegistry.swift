import Foundation
import FoundationModels

/// Actor for managing tool definitions per session
actor ToolRegistry {
    private var sessionTools: [String: [ToolDefinition]] = [:]

    /// Tool definition for registration
    struct ToolDefinition: Codable {
        let name: String
        let description: String
        let parameters: ToolParameters
    }

    /// Tool parameters schema
    struct ToolParameters: Codable {
        let type: String
        let properties: [String: PropertyDefinition]
        let required: [String]?
    }

    /// Property definition for tool parameters
    struct PropertyDefinition: Codable {
        let type: String
        let description: String?
        let `enum`: [String]?
    }

    /// Register tools for a session
    func registerTools(sessionId: String, tools: [ToolDefinition]) {
        sessionTools[sessionId] = tools
    }

    /// Get tools for a session
    func getTools(sessionId: String) -> [ToolDefinition]? {
        return sessionTools[sessionId]
    }

    /// Convert tool definitions to FoundationModels.Tool format
    func getFoundationModelsTools(sessionId: String) throws -> [LanguageModelSession.Tool]? {
        guard let tools = sessionTools[sessionId] else {
            return nil
        }

        var fmTools: [LanguageModelSession.Tool] = []

        for tool in tools {
            // Convert parameters to JSON schema string
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

            let parametersData = try encoder.encode(tool.parameters)
            guard let parametersJson = String(data: parametersData, encoding: .utf8) else {
                throw ToolError.invalidSchema
            }

            let fmTool = LanguageModelSession.Tool(
                name: tool.name,
                description: tool.description,
                parametersSchema: parametersJson
            )

            fmTools.append(fmTool)
        }

        return fmTools
    }

    /// Remove tools for a session
    func removeTools(sessionId: String) {
        sessionTools.removeValue(forKey: sessionId)
    }

    /// Clear all tools
    func clearAll() {
        sessionTools.removeAll()
    }
}

/// Tool-related errors
enum ToolError: Error {
    case invalidSchema
    case toolNotFound
    case executionFailed(String)
}

/// Tool call from the model
struct ToolCall: Codable {
    let id: String
    let type: String
    let function: FunctionCall
}

/// Function call details
struct FunctionCall: Codable {
    let name: String
    let arguments: String
}

/// Tool call response (sent back to model)
struct ToolCallResponse: Codable {
    let toolCallId: String
    let output: String

    enum CodingKeys: String, CodingKey {
        case toolCallId = "tool_call_id"
        case output
    }
}
