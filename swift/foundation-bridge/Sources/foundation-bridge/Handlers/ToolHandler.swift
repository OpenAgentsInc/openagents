import Foundation
import FoundationModels

/// Handler for tool management endpoints
struct ToolHandler {
    private let toolRegistry: ToolRegistry
    private let sessionStore: SessionStore

    init(toolRegistry: ToolRegistry, sessionStore: SessionStore) {
        self.toolRegistry = toolRegistry
        self.sessionStore = sessionStore
    }

    /// POST /v1/sessions/{id}/tools - Register tools for a session
    func registerTools(sessionId: String, body: String?) async throws -> HTTPResponse {
        guard let bodyData = body?.data(using: .utf8) else {
            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Invalid request body\"}"
            )
        }

        // Verify session exists
        guard await sessionStore.getSession(sessionId) != nil else {
            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Session not found\"}"
            )
        }

        let decoder = JSONDecoder()
        guard let request = try? decoder.decode(RegisterToolsRequest.self, from: bodyData) else {
            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Invalid request format\"}"
            )
        }

        // Register tools
        await toolRegistry.registerTools(sessionId: sessionId, tools: request.tools)

        let response = RegisterToolsResponse(
            sessionId: sessionId,
            count: request.tools.count
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// GET /v1/sessions/{id}/tools - List tools for a session
    func listTools(sessionId: String) async throws -> HTTPResponse {
        // Verify session exists
        guard await sessionStore.getSession(sessionId) != nil else {
            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Session not found\"}"
            )
        }

        let tools = await toolRegistry.getTools(sessionId: sessionId) ?? []

        let response = ListToolsResponse(
            sessionId: sessionId,
            tools: tools,
            count: tools.count
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// DELETE /v1/sessions/{id}/tools - Remove tools from a session
    func removeTools(sessionId: String) async throws -> HTTPResponse {
        // Verify session exists
        guard await sessionStore.getSession(sessionId) != nil else {
            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Session not found\"}"
            )
        }

        await toolRegistry.removeTools(sessionId: sessionId)

        let response = RemoveToolsResponse(
            sessionId: sessionId,
            removed: true
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// Complete with tool calling support
    func completeWithTools(
        sessionId: String,
        prompt: String,
        toolCallResults: [ToolCallResponse]?
    ) async throws -> CompletionWithToolsResponse {
        // Get session
        guard let session = await sessionStore.getSession(sessionId) else {
            throw CompletionError.sessionNotFound
        }

        // Get tools for this session
        let tools = try await toolRegistry.getFoundationModelsTools(sessionId: sessionId)

        // If there are tool call results, append them to transcript first
        if let results = toolCallResults {
            // TODO: Append tool results to session transcript
            // This requires extending SessionStore to handle tool messages
        }

        // Perform completion with tools
        let response: Response<String>

        if let tools = tools, !tools.isEmpty {
            response = try await session.respond(
                to: prompt,
                tools: tools
            )
        } else {
            response = try await session.respond(to: prompt)
        }

        let content = response.content as? String ?? ""

        // Check if model wants to call tools
        // Note: FoundationModels doesn't expose tool calls directly in Response
        // This is a limitation - we'd need to parse the content for tool call markers
        // or wait for Apple to add tool_calls to the Response type

        // For now, return standard response
        return CompletionWithToolsResponse(
            content: content,
            toolCalls: nil,
            finishReason: "stop"
        )
    }
}

// MARK: - Request/Response Types

struct RegisterToolsRequest: Codable {
    let tools: [ToolRegistry.ToolDefinition]
}

struct RegisterToolsResponse: Codable {
    let sessionId: String
    let count: Int

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case count
    }
}

struct ListToolsResponse: Codable {
    let sessionId: String
    let tools: [ToolRegistry.ToolDefinition]
    let count: Int

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case tools
        case count
    }
}

struct RemoveToolsResponse: Codable {
    let sessionId: String
    let removed: Bool

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case removed
    }
}

struct CompletionWithToolsResponse: Codable {
    let content: String
    let toolCalls: [ToolCall]?
    let finishReason: String

    enum CodingKeys: String, CodingKey {
        case content
        case toolCalls = "tool_calls"
        case finishReason = "finish_reason"
    }
}
