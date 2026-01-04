import Foundation
import Network

/// Simple HTTP server using Network.framework
actor HTTPServer {
    private let port: UInt16
    private var listener: NWListener?
    private let chatHandler: ChatHandler
    private let sessionStore: SessionStore
    private let sessionHandler: SessionHandler
    private let toolRegistry: ToolRegistry
    private let toolHandler: ToolHandler
    private let adapterRegistry: AdapterRegistry
    private let adapterHandler: AdapterHandler

    init(port: UInt16, chatHandler: ChatHandler) {
        self.port = port
        self.chatHandler = chatHandler
        self.sessionStore = SessionStore()
        self.toolRegistry = ToolRegistry()
        self.adapterRegistry = AdapterRegistry()
        self.sessionHandler = SessionHandler(sessionStore: sessionStore)
        self.toolHandler = ToolHandler(toolRegistry: toolRegistry, sessionStore: sessionStore)
        self.adapterHandler = AdapterHandler(adapterRegistry: adapterRegistry)
    }

    /// Start the HTTP server
    func start() async throws {
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true

        let listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: port)!)
        self.listener = listener

        listener.stateUpdateHandler = { state in
            switch state {
            case .ready:
                print("Server listening on port \(self.port)")
            case .failed(let error):
                print("Server failed: \(error)")
            case .cancelled:
                print("Server cancelled")
            default:
                break
            }
        }

        listener.newConnectionHandler = { [weak self] connection in
            guard let self = self else { return }
            Task {
                await self.handleConnection(connection)
            }
        }

        listener.start(queue: .main)

        // Keep running indefinitely using RunLoop
        await withCheckedContinuation { (_: CheckedContinuation<Void, Never>) in
            // Never resume - keeps the task alive forever
            RunLoop.main.run()
        }
    }

    /// Stop the server
    func stop() {
        listener?.cancel()
        listener = nil
    }

    /// Handle an incoming connection
    private func handleConnection(_ connection: NWConnection) async {
        connection.start(queue: .main)

        // Read request data
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [weak self] data, _, isComplete, error in
            guard let self = self else { return }

            if let data = data, !data.isEmpty {
                Task {
                    await self.processRequestAndRespond(data: data, connection: connection)
                }
            } else if let error = error {
                print("Receive error: \(error)")
                connection.cancel()
            }
        }
    }

    /// Process request and send appropriate response (regular or streaming)
    private func processRequestAndRespond(data: Data, connection: NWConnection) async {
        // Parse HTTP request
        guard let requestString = String(data: data, encoding: .utf8) else {
            await sendResponse(connection: connection, response: buildErrorResponse(status: 400, message: "Invalid request encoding"))
            return
        }

        let lines = requestString.split(separator: "\r\n", omittingEmptySubsequences: false)
        guard let requestLine = lines.first else {
            await sendResponse(connection: connection, response: buildErrorResponse(status: 400, message: "Empty request"))
            return
        }

        let parts = requestLine.split(separator: " ")
        guard parts.count >= 2 else {
            await sendResponse(connection: connection, response: buildErrorResponse(status: 400, message: "Invalid request line"))
            return
        }

        let method = String(parts[0])
        let path = String(parts[1])

        // Find body (after empty line)
        var body: String?
        if let emptyLineIndex = lines.firstIndex(of: "") {
            let bodyLines = lines[(emptyLineIndex + 1)...]
            body = bodyLines.joined(separator: "\r\n")
        }

        // Parse path and query string
        let pathComponents = path.split(separator: "?", maxSplits: 1)
        let actualPath = String(pathComponents[0])
        let queryString = pathComponents.count > 1 ? String(pathComponents[1]) : nil

        // Check if this is a streaming request
        if method == "POST" && (actualPath == "/v1/chat/completions" || actualPath == "/chat/completions") {
            // Check if request wants streaming
            if await shouldStream(body: body, queryString: queryString) {
                await handleStreamingRequest(body: body, connection: connection)
                return
            }
        }

        // Regular (non-streaming) request
        let response = await routeRequest(method: method, path: path, body: body)
        await sendResponse(connection: connection, response: response)
    }

    /// Check if request should use streaming
    private func shouldStream(body: String?, queryString: String?) async -> Bool {
        // Check query string for stream=true
        if let query = queryString, query.contains("stream=true") {
            return true
        }

        // Check request body for "stream": true
        if let bodyString = body,
           let bodyData = bodyString.data(using: .utf8),
           let request = try? JSONDecoder().decode(ChatCompletionRequest.self, from: bodyData) {
            return request.stream == true
        }

        return false
    }

    /// Handle streaming SSE request
    private func handleStreamingRequest(body: String?, connection: NWConnection) async {
        guard let bodyString = body, !bodyString.isEmpty else {
            await sendResponse(connection: connection, response: buildErrorResponse(status: 400, message: "Missing request body"))
            return
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            await sendResponse(connection: connection, response: buildErrorResponse(status: 400, message: "Invalid body encoding"))
            return
        }

        do {
            let request = try JSONDecoder().decode(ChatCompletionRequest.self, from: bodyData)
            let stream = try await StreamHandler.handleStreamingCompletion(request: request)

            // Send SSE headers
            let headers = [
                "HTTP/1.1 200 OK",
                "Content-Type: text/event-stream",
                "Cache-Control: no-cache",
                "Connection: keep-alive",
                "Access-Control-Allow-Origin: *",
                "",
                ""
            ].joined(separator: "\r\n")

            connection.send(content: headers.data(using: .utf8)!, completion: .contentProcessed { _ in })

            // Send SSE events
            for await event in stream {
                if let eventData = event.data(using: .utf8) {
                    connection.send(content: eventData, completion: .contentProcessed { _ in })
                }
            }

            // Close connection after stream completes
            connection.cancel()
        } catch let error as FMError {
            let status: Int
            switch error {
            case .modelUnavailable: status = 503
            case .requestFailed: status = 500
            case .invalidRequest: status = 400
            case .serverError: status = 500
            }
            await sendResponse(connection: connection, response: buildJSONResponse(status: status, body: error.errorResponse))
        } catch {
            await sendResponse(connection: connection, response: buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse))
        }
    }

    /// Process an HTTP request and return response data
    private func processRequest(_ data: Data) async -> Data {
        // Parse HTTP request
        guard let requestString = String(data: data, encoding: .utf8) else {
            return buildErrorResponse(status: 400, message: "Invalid request encoding")
        }

        let lines = requestString.split(separator: "\r\n", omittingEmptySubsequences: false)
        guard let requestLine = lines.first else {
            return buildErrorResponse(status: 400, message: "Empty request")
        }

        let parts = requestLine.split(separator: " ")
        guard parts.count >= 2 else {
            return buildErrorResponse(status: 400, message: "Invalid request line")
        }

        let method = String(parts[0])
        let path = String(parts[1])

        // Find body (after empty line)
        var body: String?
        if let emptyLineIndex = lines.firstIndex(of: "") {
            let bodyLines = lines[(emptyLineIndex + 1)...]
            body = bodyLines.joined(separator: "\r\n")
        }

        // Route request
        return await routeRequest(method: method, path: path, body: body)
    }

    /// Route request to appropriate handler
    private func routeRequest(method: String, path: String, body: String?) async -> Data {
        // Handle CORS preflight
        if method == "OPTIONS" {
            return buildCORSResponse()
        }

        // Parse path and query string
        let pathComponents = path.split(separator: "?", maxSplits: 1)
        let actualPath = String(pathComponents[0])
        let queryString = pathComponents.count > 1 ? String(pathComponents[1]) : nil

        switch (method, actualPath) {
        case ("GET", "/health"):
            return await handleHealth()

        case ("GET", "/v1/models"), ("GET", "/models"):
            return await handleModels()

        case ("POST", "/v1/chat/completions"), ("POST", "/chat/completions"):
            return await handleChatCompletions(body: body)

        // Session management endpoints
        case ("POST", "/v1/sessions"):
            return await handleSessionCreate(body: body)

        case ("GET", "/v1/sessions"):
            return await handleSessionList()

        case ("DELETE", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/"):
            let sessionId = String(actualPath.dropFirst("/v1/sessions/".count))
            return await handleSessionDelete(id: sessionId)

        case ("GET", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/") && actualPath.hasSuffix("/transcript"):
            let pathWithoutPrefix = actualPath.dropFirst("/v1/sessions/".count)
            let sessionId = String(pathWithoutPrefix.dropLast("/transcript".count))
            return await handleSessionTranscript(id: sessionId)

        case ("GET", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/"):
            let sessionId = String(actualPath.dropFirst("/v1/sessions/".count))
            return await handleSessionGet(id: sessionId)

        case ("POST", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/") && actualPath.hasSuffix("/complete"):
            let pathWithoutPrefix = actualPath.dropFirst("/v1/sessions/".count)
            let sessionId = String(pathWithoutPrefix.dropLast("/complete".count))
            return await handleSessionComplete(id: sessionId, body: body)

        // Tool management endpoints
        case ("POST", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/") && actualPath.hasSuffix("/tools"):
            let pathWithoutPrefix = actualPath.dropFirst("/v1/sessions/".count)
            let sessionId = String(pathWithoutPrefix.dropLast("/tools".count))
            return await handleToolRegister(sessionId: sessionId, body: body)

        case ("GET", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/") && actualPath.hasSuffix("/tools"):
            let pathWithoutPrefix = actualPath.dropFirst("/v1/sessions/".count)
            let sessionId = String(pathWithoutPrefix.dropLast("/tools".count))
            return await handleToolList(sessionId: sessionId)

        case ("DELETE", "/v1/sessions") where actualPath.hasPrefix("/v1/sessions/") && actualPath.hasSuffix("/tools"):
            let pathWithoutPrefix = actualPath.dropFirst("/v1/sessions/".count)
            let sessionId = String(pathWithoutPrefix.dropLast("/tools".count))
            return await handleToolRemove(sessionId: sessionId)

        // FRLM tool endpoints
        case ("POST", "/v1/tools/select"):
            return await handleToolSelect(body: body)

        case ("POST", "/v1/tools/execute"):
            return await handleToolExecute(body: body)

        case ("GET", "/v1/tools"):
            return await handleToolList()

        // Adapter management endpoints
        case ("POST", "/v1/adapters/load"):
            return await handleAdapterLoad(body: body)

        case ("GET", "/v1/adapters"):
            return await handleAdapterList()

        case ("POST", "/v1/adapters/cleanup"):
            return await handleAdapterCleanup()

        case ("GET", "/v1/adapters/compatible") where actualPath.hasPrefix("/v1/adapters/compatible/"):
            let name = String(actualPath.dropFirst("/v1/adapters/compatible/".count))
            return await handleAdapterCompatible(name: name)

        case ("POST", "/v1/adapters") where actualPath.hasPrefix("/v1/adapters/") && actualPath.hasSuffix("/compile"):
            let pathWithoutPrefix = actualPath.dropFirst("/v1/adapters/".count)
            let adapterId = String(pathWithoutPrefix.dropLast("/compile".count))
            return await handleAdapterRecompile(id: adapterId)

        case ("GET", "/v1/adapters") where actualPath.hasPrefix("/v1/adapters/"):
            let adapterId = String(actualPath.dropFirst("/v1/adapters/".count))
            return await handleAdapterGet(id: adapterId)

        case ("DELETE", "/v1/adapters") where actualPath.hasPrefix("/v1/adapters/"):
            let adapterId = String(actualPath.dropFirst("/v1/adapters/".count))
            return await handleAdapterUnload(id: adapterId)

        default:
            return buildErrorResponse(status: 404, message: "Not found: \(method) \(path)")
        }
    }

    // MARK: - Route Handlers

    private func handleHealth() async -> Data {
        let (available, _) = await chatHandler.getAvailabilityStatus()
        let health = HealthResponse(
            status: available ? "ok" : "degraded",
            modelAvailable: available,
            version: "1.0.0",
            platform: "macOS"
        )
        return buildJSONResponse(status: 200, body: health)
    }

    private func handleModels() async -> Data {
        let models = ModelsResponse(
            object: "list",
            data: [
                ModelInfo(
                    id: "apple-foundation-model",
                    object: "model",
                    created: Int(Date().timeIntervalSince1970),
                    ownedBy: "apple"
                )
            ]
        )
        return buildJSONResponse(status: 200, body: models)
    }

    private func handleChatCompletions(body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Missing request body").errorResponse)
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Invalid body encoding").errorResponse)
        }

        do {
            let request = try JSONDecoder().decode(ChatCompletionRequest.self, from: bodyData)
            let response = try await chatHandler.handleCompletion(request: request)
            return buildJSONResponse(status: 200, body: response)
        } catch let error as FMError {
            let status: Int
            switch error {
            case .modelUnavailable: status = 503
            case .requestFailed: status = 500
            case .invalidRequest: status = 400
            case .serverError: status = 500
            }
            return buildJSONResponse(status: status, body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Invalid JSON: \(error.localizedDescription)").errorResponse)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    // MARK: - Session Handlers

    private func handleSessionCreate(body: String?) async -> Data {
        do {
            let response = try await sessionHandler.createSession(body: body)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleSessionList() async -> Data {
        do {
            let response = try await sessionHandler.listSessions()
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleSessionGet(id: String) async -> Data {
        do {
            let response = try await sessionHandler.getSession(id: id)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleSessionTranscript(id: String) async -> Data {
        do {
            let response = try await sessionHandler.getTranscript(id: id)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleSessionDelete(id: String) async -> Data {
        do {
            let response = try await sessionHandler.deleteSession(id: id)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleSessionComplete(id: String, body: String?) async -> Data {
        do {
            let response = try await sessionHandler.completeWithSession(id: id, body: body)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    /// Convert HTTPResponse to Data
    private func httpResponseToData(_ response: HTTPResponse) -> Data {
        let statusText = httpStatusText(response.status)
        let headers = ["HTTP/1.1 \(response.status) \(statusText)"] + response.headers.map { "\($0.key): \($0.value)" } + ["", ""]
        let headerString = headers.joined(separator: "\r\n")

        var data = headerString.data(using: .utf8)!
        if let bodyData = response.body.data(using: .utf8) {
            data.append(bodyData)
        }
        return data
    }

    // MARK: - Tool Handlers

    private func handleToolRegister(sessionId: String, body: String?) async -> Data {
        do {
            let response = try await toolHandler.registerTools(sessionId: sessionId, body: body)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleToolList(sessionId: String) async -> Data {
        do {
            let response = try await toolHandler.listTools(sessionId: sessionId)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleToolRemove(sessionId: String) async -> Data {
        do {
            let response = try await toolHandler.removeTools(sessionId: sessionId)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    // MARK: - FRLM Tool Handlers

    private func handleToolSelect(body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Missing request body").errorResponse)
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Invalid body encoding").errorResponse)
        }

        do {
            let response = try await toolHandler.selectFrlmTool(body: bodyData)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleToolExecute(body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Missing request body").errorResponse)
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(status: 400, body: FMError.invalidRequest("Invalid body encoding").errorResponse)
        }

        do {
            let response = try await toolHandler.executeFrlmTool(body: bodyData)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleToolList() async -> Data {
        let response = await toolHandler.listFrlmTools()
        return httpResponseToData(response)
    }

    // MARK: - Adapter Handlers

    private func handleAdapterLoad(body: String?) async -> Data {
        do {
            let response = try await adapterHandler.loadAdapter(body: body)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleAdapterList() async -> Data {
        do {
            let response = try await adapterHandler.listAdapters()
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleAdapterGet(id: String) async -> Data {
        do {
            let response = try await adapterHandler.getAdapter(id: id)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleAdapterUnload(id: String) async -> Data {
        do {
            let response = try await adapterHandler.unloadAdapter(id: id)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleAdapterRecompile(id: String) async -> Data {
        do {
            let response = try await adapterHandler.recompileAdapter(id: id)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleAdapterCompatible(name: String) async -> Data {
        do {
            let response = try await adapterHandler.getCompatibleIdentifiers(name: name)
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    private func handleAdapterCleanup() async -> Data {
        do {
            let response = try await adapterHandler.cleanupObsoleteAdapters()
            return httpResponseToData(response)
        } catch {
            return buildJSONResponse(status: 500, body: FMError.serverError(error.localizedDescription).errorResponse)
        }
    }

    // MARK: - Response Builders

    private func buildJSONResponse<T: Encodable>(status: Int, body: T) -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        guard let jsonData = try? encoder.encode(body) else {
            return buildErrorResponse(status: 500, message: "Failed to encode response")
        }

        let statusText = httpStatusText(status)
        let headers = [
            "HTTP/1.1 \(status) \(statusText)",
            "Content-Type: application/json",
            "Content-Length: \(jsonData.count)",
            "Access-Control-Allow-Origin: *",
            "Access-Control-Allow-Methods: GET, POST, OPTIONS",
            "Access-Control-Allow-Headers: Content-Type",
            "Connection: close",
            "",
            ""
        ].joined(separator: "\r\n")

        var response = headers.data(using: .utf8)!
        response.append(jsonData)
        return response
    }

    private func buildErrorResponse(status: Int, message: String) -> Data {
        let error = ErrorResponse(error: ErrorDetail(message: message, type: "error", code: nil))
        return buildJSONResponse(status: status, body: error)
    }

    private func buildCORSResponse() -> Data {
        let headers = [
            "HTTP/1.1 204 No Content",
            "Access-Control-Allow-Origin: *",
            "Access-Control-Allow-Methods: GET, POST, OPTIONS",
            "Access-Control-Allow-Headers: Content-Type",
            "Content-Length: 0",
            "Connection: close",
            "",
            ""
        ].joined(separator: "\r\n")
        return headers.data(using: .utf8)!
    }

    private func sendResponse(connection: NWConnection, response: Data) async {
        connection.send(content: response, completion: .contentProcessed { error in
            if let error = error {
                print("Send error: \(error)")
            }
            connection.cancel()
        })
    }

    private func httpStatusText(_ status: Int) -> String {
        switch status {
        case 200: return "OK"
        case 204: return "No Content"
        case 400: return "Bad Request"
        case 404: return "Not Found"
        case 500: return "Internal Server Error"
        case 503: return "Service Unavailable"
        default: return "Unknown"
        }
    }
}
