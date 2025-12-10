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
    /// NOTE: Tool is a protocol with associated types, not a concrete type.
    /// Dynamic tool creation would require concrete implementations.
    /// For now, we just store the definitions and let callers handle conversion.
    func getFoundationModelsTools(sessionId: String) throws -> [ToolDefinition]? {
        // Return our internal ToolDefinition array instead of trying to convert to FM types
        return sessionTools[sessionId]
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
