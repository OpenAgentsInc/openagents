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

    // MARK: - FRLM Tool Methods

    /// Select an FRLM tool using guided generation.
    func selectFrlmTool(body: Data) async throws -> HTTPResponse {
        let decoder = JSONDecoder()
        guard let request = try? decoder.decode(FrlmToolSelectRequest.self, from: body) else {
            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Invalid request format\"}"
            )
        }

        // Use guided generation to select the appropriate tool
        // For now, return a simple heuristic-based selection
        let selectedTool = selectToolHeuristic(prompt: request.prompt)

        let response = FrlmToolSelectResponse(
            tool: selectedTool.tool,
            arguments: selectedTool.arguments,
            reasoning: selectedTool.reasoning
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

    /// Execute an FRLM tool by forwarding to Rust backend.
    func executeFrlmTool(body: Data) async throws -> HTTPResponse {
        let decoder = JSONDecoder()
        guard let request = try? decoder.decode(FrlmToolExecuteRequest.self, from: body) else {
            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Invalid request format\"}"
            )
        }

        // Forward to Rust backend for execution
        let result = await executeToolViaRust(tool: request.tool, arguments: request.arguments)

        let response = FrlmToolExecuteResponse(
            result: result.result,
            success: result.success,
            error: result.error
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: result.success ? 200 : 500,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// List all available FRLM tools.
    func listFrlmTools() async -> HTTPResponse {
        let tools = [
            FrlmToolInfo(name: "llm_query_recursive", description: "Make a recursive sub-LM call"),
            FrlmToolInfo(name: "load_environment", description: "Load fragments into execution context"),
            FrlmToolInfo(name: "select_fragments", description: "Select relevant fragments from loaded environment"),
            FrlmToolInfo(name: "execute_parallel", description: "Execute multiple sub-queries in parallel"),
            FrlmToolInfo(name: "verify_results", description: "Verify sub-query results using specified verification tier"),
            FrlmToolInfo(name: "check_budget", description: "Check remaining token budget or update allocation"),
            FrlmToolInfo(name: "get_trace_events", description: "Get execution trace events for debugging")
        ]

        let response = FrlmToolListResponse(tools: tools, count: tools.count)

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(response),
              let json = String(data: data, encoding: .utf8) else {
            return HTTPResponse(
                status: 500,
                headers: ["Content-Type": "application/json"],
                body: "{\"error\":\"Failed to encode response\"}"
            )
        }

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// Heuristic-based tool selection (placeholder for guided generation).
    private func selectToolHeuristic(prompt: String) -> (tool: String, arguments: String, reasoning: String) {
        let lowercasePrompt = prompt.lowercased()

        if lowercasePrompt.contains("budget") || lowercasePrompt.contains("cost") || lowercasePrompt.contains("token") {
            return (
                tool: "check_budget",
                arguments: "{\"action\": \"check\"}",
                reasoning: "Prompt mentions budget/cost/tokens, using check_budget tool"
            )
        }

        if lowercasePrompt.contains("load") || lowercasePrompt.contains("fragment") || lowercasePrompt.contains("context") {
            return (
                tool: "load_environment",
                arguments: "{\"fragments\": []}",
                reasoning: "Prompt mentions loading or fragments, using load_environment tool"
            )
        }

        if lowercasePrompt.contains("parallel") || lowercasePrompt.contains("batch") || lowercasePrompt.contains("multiple") {
            return (
                tool: "execute_parallel",
                arguments: "{\"queries\": []}",
                reasoning: "Prompt mentions parallel/batch execution, using execute_parallel tool"
            )
        }

        if lowercasePrompt.contains("verify") || lowercasePrompt.contains("check") || lowercasePrompt.contains("validate") {
            return (
                tool: "verify_results",
                arguments: "{\"results\": [], \"tier\": \"none\"}",
                reasoning: "Prompt mentions verification, using verify_results tool"
            )
        }

        if lowercasePrompt.contains("trace") || lowercasePrompt.contains("debug") || lowercasePrompt.contains("log") {
            return (
                tool: "get_trace_events",
                arguments: "{}",
                reasoning: "Prompt mentions tracing/debugging, using get_trace_events tool"
            )
        }

        // Default: recursive LLM query
        return (
            tool: "llm_query_recursive",
            arguments: "{\"prompt\": \"\(prompt.replacingOccurrences(of: "\"", with: "\\\""))\"}",
            reasoning: "Default tool for general queries"
        )
    }

    /// Execute a tool via the Rust FRLM backend.
    private func executeToolViaRust(tool: String, arguments: String) async -> (result: String, success: Bool, error: String?) {
        // Forward to Rust backend at localhost:3000 (or configured port)
        let rustBackendUrl = ProcessInfo.processInfo.environment["RUST_BACKEND_URL"] ?? "http://localhost:3000"
        guard let url = URL(string: "\(rustBackendUrl)/v1/tools/execute") else {
            return (result: "", success: false, error: "Invalid backend URL")
        }

        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        let body: [String: Any] = [
            "tool": tool,
            "arguments": arguments
        ]

        do {
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
            let (data, response) = try await URLSession.shared.data(for: request)

            guard let httpResponse = response as? HTTPURLResponse else {
                return (result: "", success: false, error: "Invalid response type")
            }

            let resultString = String(data: data, encoding: .utf8) ?? "{}"

            if httpResponse.statusCode == 200 {
                return (result: resultString, success: true, error: nil)
            } else {
                return (result: "", success: false, error: "Backend returned status \(httpResponse.statusCode)")
            }
        } catch {
            return (result: "", success: false, error: error.localizedDescription)
        }
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

        // Perform completion
        // Note: Tools are registered at session creation time in LanguageModelSession
        // For now, we just do standard completion since dynamic tool passing isn't supported
        _ = tools // Silence unused warning - tools would need to be set at session init

        let response = try await session.respond(to: prompt)
        let content = response.content

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

// MARK: - FRLM Tool Types

struct FrlmToolSelectRequest: Codable {
    let prompt: String
    let availableTools: [String]?

    enum CodingKeys: String, CodingKey {
        case prompt
        case availableTools = "available_tools"
    }
}

struct FrlmToolSelectResponse: Codable {
    let tool: String
    let arguments: String
    let reasoning: String

    enum CodingKeys: String, CodingKey {
        case tool
        case arguments
        case reasoning
    }
}

struct FrlmToolExecuteRequest: Codable {
    let tool: String
    let arguments: String
}

struct FrlmToolExecuteResponse: Codable {
    let result: String
    let success: Bool
    let error: String?
}

struct FrlmToolInfo: Codable {
    let name: String
    let description: String
}

struct FrlmToolListResponse: Codable {
    let tools: [FrlmToolInfo]
    let count: Int
}
