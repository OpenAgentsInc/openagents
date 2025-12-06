import Foundation
import Network

/// Simple HTTP server using Network.framework
actor HTTPServer {
    private let port: UInt16
    private var listener: NWListener?
    private let chatHandler: ChatHandler

    init(port: UInt16, chatHandler: ChatHandler) {
        self.port = port
        self.chatHandler = chatHandler
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

        // Keep running
        try await Task.sleep(for: .seconds(.max))
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
                    let response = await self.processRequest(data)
                    await self.sendResponse(connection: connection, response: response)
                }
            } else if let error = error {
                print("Receive error: \(error)")
                connection.cancel()
            }
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

        switch (method, path) {
        case ("GET", "/health"):
            return await handleHealth()

        case ("GET", "/v1/models"), ("GET", "/models"):
            return await handleModels()

        case ("POST", "/v1/chat/completions"), ("POST", "/chat/completions"):
            return await handleChatCompletions(body: body)

        default:
            return buildErrorResponse(status: 404, message: "Not found: \(method) \(path)")
        }
    }

    // MARK: - Route Handlers

    private func handleHealth() async -> Data {
        let (available, message) = await chatHandler.getAvailabilityStatus()
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
