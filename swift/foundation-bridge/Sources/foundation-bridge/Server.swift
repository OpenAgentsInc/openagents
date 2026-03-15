import Foundation
import Network

actor HTTPServer {
    private struct ParsedRequest {
        let method: String
        let path: String
        let body: String?
    }

    private enum RouteResponse {
        case buffered(Data)
        case sse(sessionID: String, stream: AsyncThrowingStream<TextStreamEvent, Error>)
    }

    private let port: UInt16
    private var listener: NWListener?
    private let chatHandler: ChatHandler

    init(port: UInt16, chatHandler: ChatHandler) {
        self.port = port
        self.chatHandler = chatHandler
    }

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
            guard let self else { return }
            Task {
                await self.handleConnection(connection)
            }
        }

        listener.start(queue: .main)
        while true {
            try? await Task.sleep(for: .seconds(3600))
        }
    }

    private func handleConnection(_ connection: NWConnection) async {
        connection.start(queue: .main)
        guard let data = await receiveRequestData(connection: connection) else {
            connection.cancel()
            return
        }
        let response = await self.processRequest(data)
        switch response {
        case .buffered(let data):
            await self.sendResponse(connection: connection, response: data)
        case .sse(let sessionID, let stream):
            await self.sendSseResponse(
                connection: connection,
                sessionID: sessionID,
                stream: stream
            )
        }
    }

    private func receiveRequestData(connection: NWConnection) async -> Data? {
        var buffer = Data()
        while true {
            let received = await withCheckedContinuation {
                (
                    continuation: CheckedContinuation<(Data?, Bool, NWError?), Never>
                ) in
                connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) {
                    data,
                    _,
                    isComplete,
                    error in
                    continuation.resume(returning: (data, isComplete, error))
                }
            }

            if let error = received.2 {
                print("Receive error: \(error)")
                return nil
            }
            if let data = received.0, !data.isEmpty {
                buffer.append(data)
            }
            if requestDataIsComplete(buffer) {
                return buffer
            }
            if received.1 {
                return buffer.isEmpty ? nil : buffer
            }
        }
    }

    private func processRequest(_ data: Data) async -> RouteResponse {
        guard let requestString = String(data: data, encoding: .utf8) else {
            return .buffered(
                buildErrorResponse(status: 400, message: "Invalid request encoding")
            )
        }
        guard let request = parseRequest(requestString) else {
            return .buffered(buildErrorResponse(status: 400, message: "Invalid request line"))
        }
        return await routeRequest(request)
    }

    private func parseRequest(_ requestString: String) -> ParsedRequest? {
        let lines = requestString.split(separator: "\r\n", omittingEmptySubsequences: false)
        guard let requestLine = lines.first else {
            return nil
        }

        let parts = requestLine.split(separator: " ")
        guard parts.count >= 2 else {
            return nil
        }

        let method = String(parts[0])
        let path = String(parts[1])

        var body: String?
        if let emptyLineIndex = lines.firstIndex(of: "") {
            let bodyLines = lines[(emptyLineIndex + 1)...]
            body = bodyLines.joined(separator: "\r\n")
        }

        return ParsedRequest(method: method, path: path, body: body)
    }

    private func requestDataIsComplete(_ data: Data) -> Bool {
        guard let headerEnd = data.range(of: Data("\r\n\r\n".utf8)) else {
            return false
        }
        let bodyStart = headerEnd.upperBound
        guard let headersString = String(
            data: data[..<headerEnd.lowerBound],
            encoding: .utf8
        ) else {
            return false
        }
        let contentLength = parseContentLength(headersString) ?? 0
        return data.count >= bodyStart + contentLength
    }

    private func parseContentLength(_ headersString: String) -> Int? {
        for line in headersString.components(separatedBy: "\r\n") {
            let parts = line.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
            guard parts.count == 2 else {
                continue
            }
            if parts[0].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                == "content-length"
            {
                return Int(parts[1].trimmingCharacters(in: .whitespacesAndNewlines))
            }
        }
        return nil
    }

    private func routeRequest(_ request: ParsedRequest) async -> RouteResponse {
        if request.method == "OPTIONS" {
            return .buffered(buildCORSResponse())
        }

        if request.path == "/v1/adapters" || request.path.hasPrefix("/v1/adapters/") {
            return await handleAdapters(request)
        }

        if request.path == "/v1/sessions" || request.path.hasPrefix("/v1/sessions/") {
            return await handleSessions(request)
        }

        switch (request.method, request.path) {
        case ("GET", "/health"):
            return .buffered(await handleHealth())
        case ("GET", "/v1/models"), ("GET", "/models"):
            return .buffered(await handleModels())
        case ("POST", "/v1/chat/completions"), ("POST", "/chat/completions"):
            return .buffered(await handleChatCompletions(body: request.body))
        default:
            return .buffered(
                buildErrorResponse(
                    status: 404,
                    message: "Not found: \(request.method) \(request.path)"
                )
            )
        }
    }

    private func handleHealth() async -> Data {
        let (available, reason, message) = await chatHandler.getAvailabilityStatus()
        let adapters = await chatHandler.listAdapters()
        let health = HealthResponse(
            status: available ? "ok" : "degraded",
            modelAvailable: available,
            version: "1.0.0",
            platform: "macOS",
            availabilityMessage: message,
            unavailableReason: reason,
            defaultUseCase: .general,
            defaultGuardrails: .default,
            supportedUseCases: await chatHandler.supportedUseCases(),
            supportedGuardrails: await chatHandler.supportedGuardrails(),
            appleSiliconRequired: true,
            appleIntelligenceRequired: true,
            adapterInventorySupported: true,
            adapterAttachSupported: adapters.attachSupported ?? true,
            loadedAdapters: adapters.adapters
        )
        return buildJSONResponse(status: 200, body: health)
    }

    private func handleModels() async -> Data {
        let (available, reason, message) = await chatHandler.getAvailabilityStatus()
        let models = ModelsResponse(
            object: "list",
            data: [
                ModelInfo(
                    id: "apple-foundation-model",
                    object: "model",
                    created: Int(Date().timeIntervalSince1970),
                    ownedBy: "apple",
                    defaultUseCase: .general,
                    defaultGuardrails: .default,
                    supportedUseCases: await chatHandler.supportedUseCases(),
                    supportedGuardrails: await chatHandler.supportedGuardrails(),
                    available: available,
                    unavailableReason: reason,
                    availabilityMessage: message
                )
            ]
        )
        return buildJSONResponse(status: 200, body: models)
    }

    private func handleAdapterInventory() async -> Data {
        let response = await chatHandler.listAdapters()
        return buildJSONResponse(status: 200, body: response)
    }

    private func handleAdapterLoad(body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                    .errorResponse
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                    .errorResponse
            )
        }

        do {
            let request = try JSONDecoder().decode(AdapterLoadRequest.self, from: bodyData)
            let response = try await chatHandler.loadAdapter(request: request)
            return buildJSONResponse(status: 200, body: response)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Invalid JSON: \(error.localizedDescription)",
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleAdapterUnload(adapterID: String) async -> Data {
        do {
            try await chatHandler.unloadAdapter(adapterID: adapterID)
            return buildJSONResponse(status: 200, body: ["deleted": true])
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessions(_ request: ParsedRequest) async -> RouteResponse {
        switch (request.method, request.path) {
        case ("POST", "/v1/sessions"):
            return .buffered(await handleSessionCreate(body: request.body))
        default:
            break
        }

        let components = request.path.split(separator: "/")
        guard components.count >= 3 else {
            return .buffered(
                buildErrorResponse(
                    status: 404,
                    message: "Not found: \(request.method) \(request.path)"
                )
            )
        }
        let sessionID = String(components[2])

        if components.count == 3 {
            switch request.method {
            case "GET":
                return .buffered(await handleSessionGet(sessionID: sessionID))
            case "DELETE":
                return .buffered(await handleSessionDelete(sessionID: sessionID))
            default:
                return .buffered(
                    buildErrorResponse(
                        status: 404,
                        message: "Not found: \(request.method) \(request.path)"
                    )
                )
            }
        }

        if components.count == 4 {
            switch (request.method, components[3]) {
            case ("POST", "responses"):
                return .buffered(await handleSessionRespond(sessionID: sessionID, body: request.body))
            case ("POST", "reset"):
                return .buffered(await handleSessionReset(sessionID: sessionID))
            case ("GET", "transcript"):
                return .buffered(await handleSessionTranscript(sessionID: sessionID))
            case ("POST", "adapter"):
                return .buffered(await handleSessionAttachAdapter(sessionID: sessionID, body: request.body))
            case ("DELETE", "adapter"):
                return .buffered(await handleSessionDetachAdapter(sessionID: sessionID))
            default:
                return .buffered(
                    buildErrorResponse(
                        status: 404,
                        message: "Not found: \(request.method) \(request.path)"
                    )
                )
            }
        }

        if components.count == 5 {
            switch (request.method, components[3], components[4]) {
            case ("POST", "responses", "stream"):
                return await handleSessionResponseStream(
                    sessionID: sessionID,
                    body: request.body
                )
            case ("POST", "responses", "structured"):
                return .buffered(
                    await handleSessionStructuredRespond(
                        sessionID: sessionID,
                        body: request.body
                    )
                )
            default:
                return .buffered(
                    buildErrorResponse(
                        status: 404,
                        message: "Not found: \(request.method) \(request.path)"
                    )
                )
            }
        }

        return .buffered(
            buildErrorResponse(
                status: 404,
                message: "Not found: \(request.method) \(request.path)"
            )
        )
    }

    private func handleAdapters(_ request: ParsedRequest) async -> RouteResponse {
        switch (request.method, request.path) {
        case ("GET", "/v1/adapters"):
            return .buffered(await handleAdapterInventory())
        case ("POST", "/v1/adapters/load"):
            return .buffered(await handleAdapterLoad(body: request.body))
        default:
            break
        }

        let components = request.path.split(separator: "/")
        guard components.count == 3 else {
            return .buffered(
                buildErrorResponse(
                    status: 404,
                    message: "Not found: \(request.method) \(request.path)"
                )
            )
        }

        let adapterID = String(components[2])
        if request.method == "DELETE" {
            return .buffered(await handleAdapterUnload(adapterID: adapterID))
        }

        return .buffered(
            buildErrorResponse(
                status: 404,
                message: "Not found: \(request.method) \(request.path)"
            )
        )
    }

    private func handleSessionCreate(body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                    .errorResponse
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                    .errorResponse
            )
        }

        do {
            let request = try JSONDecoder().decode(SessionCreateRequest.self, from: bodyData)
            let response = try await chatHandler.createSession(request: request)
            return buildJSONResponse(status: 200, body: response)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Invalid JSON: \(error.localizedDescription)",
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionGet(sessionID: String) async -> Data {
        do {
            let session = try await chatHandler.session(sessionID: sessionID)
            return buildJSONResponse(status: 200, body: session)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionDelete(sessionID: String) async -> Data {
        do {
            try await chatHandler.deleteSession(sessionID: sessionID)
            return buildJSONResponse(status: 200, body: ["deleted": true])
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionReset(sessionID: String) async -> Data {
        do {
            let session = try await chatHandler.resetSession(sessionID: sessionID)
            return buildJSONResponse(status: 200, body: session)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionTranscript(sessionID: String) async -> Data {
        do {
            let transcript = try await chatHandler.sessionTranscript(sessionID: sessionID)
            return buildJSONResponse(status: 200, body: transcript)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionAttachAdapter(sessionID: String, body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                    .errorResponse
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                    .errorResponse
            )
        }

        do {
            let request = try JSONDecoder().decode(AdapterAttachRequest.self, from: bodyData)
            let session = try await chatHandler.attachSessionAdapter(
                sessionID: sessionID,
                request: request
            )
            return buildJSONResponse(status: 200, body: session)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Invalid JSON: \(error.localizedDescription)",
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionDetachAdapter(sessionID: String) async -> Data {
        do {
            let session = try await chatHandler.detachSessionAdapter(sessionID: sessionID)
            return buildJSONResponse(status: 200, body: session)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionRespond(sessionID: String, body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                    .errorResponse
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                    .errorResponse
            )
        }

        do {
            let request = try JSONDecoder().decode(SessionRespondRequest.self, from: bodyData)
            let response = try await chatHandler.respond(sessionID: sessionID, request: request)
            return buildJSONResponse(status: 200, body: response)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Invalid JSON: \(error.localizedDescription)",
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionStructuredRespond(sessionID: String, body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                    .errorResponse
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                    .errorResponse
            )
        }

        do {
            let request = try JSONDecoder().decode(
                SessionStructuredResponseRequest.self,
                from: bodyData
            )
            let response = try await chatHandler.respondStructured(
                sessionID: sessionID,
                request: request
            )
            return buildJSONResponse(status: 200, body: response)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Invalid JSON: \(error.localizedDescription)",
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func handleSessionResponseStream(
        sessionID: String,
        body: String?
    ) async -> RouteResponse {
        guard let bodyString = body, !bodyString.isEmpty else {
            return .buffered(
                buildJSONResponse(
                    status: 400,
                    body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                        .errorResponse
                )
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return .buffered(
                buildJSONResponse(
                    status: 400,
                    body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                        .errorResponse
                )
            )
        }

        do {
            let request = try JSONDecoder().decode(SessionRespondRequest.self, from: bodyData)
            let stream = try await chatHandler.streamResponse(
                sessionID: sessionID,
                request: request
            )
            return .sse(sessionID: sessionID, stream: stream)
        } catch let error as FMError {
            return .buffered(
                buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
            )
        } catch let error as DecodingError {
            return .buffered(
                buildJSONResponse(
                    status: 400,
                    body: FMError.invalidRequest(
                        FMErrorPayload(
                            message: "Invalid JSON: \(error.localizedDescription)",
                            debugDescription: String(reflecting: error)
                        )
                    ).errorResponse
                )
            )
        } catch {
            return .buffered(
                buildJSONResponse(
                    status: 500,
                    body: FMError.serverError(
                        FMErrorPayload(
                            message: error.localizedDescription,
                            debugDescription: String(reflecting: error)
                        )
                    ).errorResponse
                )
            )
        }
    }

    private func handleChatCompletions(body: String?) async -> Data {
        guard let bodyString = body, !bodyString.isEmpty else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Missing request body"))
                    .errorResponse
            )
        }

        guard let bodyData = bodyString.data(using: .utf8) else {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(FMErrorPayload(message: "Invalid body encoding"))
                    .errorResponse
            )
        }

        do {
            let request = try JSONDecoder().decode(ChatCompletionRequest.self, from: bodyData)
            let response = try await chatHandler.handleCompletion(request: request)
            return buildJSONResponse(status: 200, body: response)
        } catch let error as FMError {
            return buildJSONResponse(status: statusCode(for: error), body: error.errorResponse)
        } catch let error as DecodingError {
            return buildJSONResponse(
                status: 400,
                body: FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Invalid JSON: \(error.localizedDescription)",
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        } catch {
            return buildJSONResponse(
                status: 500,
                body: FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
        }
    }

    private func buildJSONResponse<T: Encodable>(status: Int, body: T) -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        guard let jsonData = try? encoder.encode(body) else {
            return buildErrorResponse(status: 500, message: "Failed to encode response")
        }

        let headers = [
            "HTTP/1.1 \(status) \(httpStatusText(status))",
            "Content-Type: application/json",
            "Content-Length: \(jsonData.count)",
            "Access-Control-Allow-Origin: *",
            "Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS",
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
        let error = ErrorResponse(
            error: ErrorDetail(
                message: message,
                type: "error",
                code: nil,
                toolName: nil,
                underlyingError: nil,
                failureReason: nil,
                recoverySuggestion: nil,
                debugDescription: nil,
                refusalExplanation: nil
            )
        )
        return buildJSONResponse(status: status, body: error)
    }

    private func statusCode(for error: FMError) -> Int {
        switch error {
        case .exceededContextWindowSize:
            return 413
        case .assetsUnavailable:
            return 503
        case .guardrailViolation:
            return 403
        case .unsupportedGuide, .unsupportedLanguageOrLocale, .invalidGenerationSchema:
            return 400
        case .decodingFailure:
            return 422
        case .rateLimited:
            return 429
        case .concurrentRequests:
            return 409
        case .refusal:
            return 403
        case .toolCallFailed:
            return 502
        case .adapterNotFound:
            return 404
        case .adapterIncompatible:
            return 409
        case .serverError:
            return 500
        case .invalidRequest:
            return 400
        }
    }

    private func buildCORSResponse() -> Data {
        let headers = [
            "HTTP/1.1 204 No Content",
            "Access-Control-Allow-Origin: *",
            "Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS",
            "Access-Control-Allow-Headers: Content-Type",
            "Content-Length: 0",
            "Connection: close",
            "",
            ""
        ].joined(separator: "\r\n")
        return headers.data(using: .utf8)!
    }

    private func sendResponse(connection: NWConnection, response: Data) async {
        _ = await sendData(connection: connection, data: response)
        connection.cancel()
    }

    private func sendSseResponse(
        connection: NWConnection,
        sessionID: String,
        stream: AsyncThrowingStream<TextStreamEvent, Error>
    ) async {
        connection.stateUpdateHandler = { [chatHandler] state in
            switch state {
            case .failed, .cancelled:
                Task {
                    await chatHandler.cancelStreamingSession(sessionID: sessionID)
                }
            default:
                break
            }
        }
        let headers = [
            "HTTP/1.1 200 OK",
            "Content-Type: text/event-stream",
            "Cache-Control: no-cache",
            "Connection: close",
            "Access-Control-Allow-Origin: *",
            "Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS",
            "Access-Control-Allow-Headers: Content-Type",
            "",
            ""
        ].joined(separator: "\r\n")
        guard await sendData(connection: connection, data: Data(headers.utf8)) else {
            await chatHandler.cancelStreamingSession(sessionID: sessionID)
            connection.cancel()
            return
        }

        do {
            for try await event in stream {
                let name: String = switch event.kind {
                case .snapshot:
                    "snapshot"
                case .completed:
                    "completed"
                }
                guard let payload = try? JSONEncoder().encode(event),
                    let body = String(data: payload, encoding: .utf8)
                else {
                    continue
                }
                let frame = encodeSseFrame(event: name, data: body)
                guard await sendData(connection: connection, data: Data(frame.utf8)) else {
                    await chatHandler.cancelStreamingSession(sessionID: sessionID)
                    connection.cancel()
                    return
                }
            }
        } catch let error as FMError {
            let payload = buildJSONPayload(error.errorResponse)
            _ = await sendData(
                connection: connection,
                data: Data(encodeSseFrame(event: "error", data: payload).utf8)
            )
        } catch {
            let payload = buildJSONPayload(
                FMError.serverError(
                    FMErrorPayload(
                        message: error.localizedDescription,
                        debugDescription: String(reflecting: error)
                    )
                ).errorResponse
            )
            _ = await sendData(
                connection: connection,
                data: Data(encodeSseFrame(event: "error", data: payload).utf8)
            )
        }
        connection.cancel()
    }

    private func sendData(connection: NWConnection, data: Data) async -> Bool {
        await withCheckedContinuation { continuation in
            connection.send(content: data, completion: .contentProcessed { error in
                if let error {
                    print("Send error: \(error)")
                }
                continuation.resume(returning: error == nil)
            })
        }
    }

    private func encodeSseFrame(event: String, data: String) -> String {
        let dataLines = data.split(separator: "\n", omittingEmptySubsequences: false)
            .map { "data: \($0)" }
            .joined(separator: "\n")
        return "event: \(event)\n\(dataLines)\n\n"
    }

    private func buildJSONPayload<T: Encodable>(_ value: T) -> String {
        let encoder = JSONEncoder()
        guard let data = try? encoder.encode(value),
            let string = String(data: data, encoding: .utf8)
        else {
            return "{\"error\":{\"message\":\"failed_to_encode_payload\",\"type\":\"server_error\"}}"
        }
        return string
    }

    private func httpStatusText(_ status: Int) -> String {
        switch status {
        case 200:
            return "OK"
        case 204:
            return "No Content"
        case 400:
            return "Bad Request"
        case 409:
            return "Conflict"
        case 404:
            return "Not Found"
        case 500:
            return "Internal Server Error"
        case 503:
            return "Service Unavailable"
        default:
            return "Unknown"
        }
    }
}
