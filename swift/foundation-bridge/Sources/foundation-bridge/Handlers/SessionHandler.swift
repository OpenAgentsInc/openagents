import Foundation
import FoundationModels

/// Handler for session management endpoints
struct SessionHandler {
    private let sessionStore: SessionStore

    init(sessionStore: SessionStore) {
        self.sessionStore = sessionStore
    }

    /// POST /v1/sessions - Create a new session
    func createSession(body: String?) async throws -> HTTPResponse {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601

        // Parse request if provided
        var model: SystemLanguageModel? = nil
        var transcript: [ChatMessage]? = nil

        if let body = body, let bodyData = body.data(using: .utf8) {
            let decoder = JSONDecoder()
            if let request = try? decoder.decode(CreateSessionRequest.self, from: bodyData) {
                // Model selection if specified
                if let modelName = request.model {
                    // For now, we only support the default model
                    // Future: Support custom adapters
                    model = SystemLanguageModel.default
                }
                transcript = request.transcript
            }
        }

        // Create session
        let sessionId: String
        if let transcript = transcript {
            sessionId = await sessionStore.createSession(model: model, transcript: transcript)
        } else {
            sessionId = await sessionStore.createSession(model: model)
        }

        let response = CreateSessionResponse(
            id: sessionId,
            created: Date()
        )

        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 201,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// GET /v1/sessions - List all sessions
    func listSessions() async throws -> HTTPResponse {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601

        let sessions = await sessionStore.listSessions()
        let response = ListSessionsResponse(
            sessions: sessions,
            count: sessions.count
        )

        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// GET /v1/sessions/{id} - Get session info
    func getSession(id: String) async throws -> HTTPResponse {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601

        guard let sessionInfo = await sessionStore.getSessionInfo(id) else {
            let error = ErrorResponse(error: "Session not found")
            let data = try encoder.encode(error)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Session not found\"}"

            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }

        let data = try encoder.encode(sessionInfo)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// GET /v1/sessions/{id}/transcript - Get session transcript
    func getTranscript(id: String) async throws -> HTTPResponse {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601

        guard let transcript = await sessionStore.getTranscript(id) else {
            let error = ErrorResponse(error: "Session not found")
            let data = try encoder.encode(error)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Session not found\"}"

            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }

        let response = TranscriptResponse(
            sessionId: id,
            messages: transcript
        )

        let data = try encoder.encode(response)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }

    /// DELETE /v1/sessions/{id} - Delete a session
    func deleteSession(id: String) async throws -> HTTPResponse {
        let encoder = JSONEncoder()

        let deleted = await sessionStore.deleteSession(id)

        if deleted {
            let response = DeleteSessionResponse(
                id: id,
                deleted: true
            )

            let data = try encoder.encode(response)
            let json = String(data: data, encoding: .utf8) ?? "{}"

            return HTTPResponse(
                status: 200,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        } else {
            let error = ErrorResponse(error: "Session not found")
            let data = try encoder.encode(error)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Session not found\"}"

            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }
    }

    /// POST /v1/sessions/{id}/complete - Complete with session context
    func completeWithSession(id: String, body: String?) async throws -> HTTPResponse {
        guard let bodyData = body?.data(using: .utf8) else {
            let error = ErrorResponse(error: "Invalid request body")
            let data = try JSONEncoder().encode(error)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Invalid request\"}"

            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }

        let decoder = JSONDecoder()
        guard let request = try? decoder.decode(ChatCompletionRequest.self, from: bodyData) else {
            let error = ErrorResponse(error: "Invalid request format")
            let data = try JSONEncoder().encode(error)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Invalid request format\"}"

            return HTTPResponse(
                status: 400,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }

        // Get session
        guard let session = await sessionStore.getSession(id) else {
            let error = ErrorResponse(error: "Session not found")
            let data = try JSONEncoder().encode(error)
            let json = String(data: data, encoding: .utf8) ?? "{\"error\":\"Session not found\"}"

            return HTTPResponse(
                status: 404,
                headers: ["Content-Type": "application/json"],
                body: json
            )
        }

        // Build prompt from latest message
        let prompt = request.messages.last?.content ?? ""

        // Complete using session
        let response = try await session.respond(to: prompt)
        let content = response.content as? String ?? ""

        // Update transcript
        let newMessages = [
            ChatMessage(role: "user", content: prompt),
            ChatMessage(role: "assistant", content: content)
        ]
        await sessionStore.appendToTranscript(id, messages: newMessages)

        // Build response
        let completionResponse = CompletionResponse(
            id: "fm-\(UUID().uuidString)",
            object: "chat.completion",
            created: Int(Date().timeIntervalSince1970),
            model: "apple-foundation-model",
            choices: [
                Choice(
                    index: 0,
                    message: ResponseMessage(role: "assistant", content: content),
                    finishReason: "stop"
                )
            ],
            usage: nil
        )

        let encoder = JSONEncoder()
        let data = try encoder.encode(completionResponse)
        let json = String(data: data, encoding: .utf8) ?? "{}"

        return HTTPResponse(
            status: 200,
            headers: ["Content-Type": "application/json"],
            body: json
        )
    }
}

/// Response for session deletion
struct DeleteSessionResponse: Codable {
    let id: String
    let deleted: Bool
}

/// Response for transcript retrieval
struct TranscriptResponse: Codable {
    let sessionId: String
    let messages: [ChatMessage]
}
