import Foundation
import FoundationModels

actor ChatHandler {
    private struct SessionRecord {
        let session: LanguageModelSession
        let instructions: String?
        let model: SessionModelConfiguration
        let tools: [SessionToolMetadata]
        var isResponding: Bool
    }

    private var sessions: [String: SessionRecord] = [:]

    init() {}

    private let defaultUseCase: SystemLanguageModelUseCase = .general
    private let defaultGuardrails: SystemLanguageModelGuardrails = .default

    func supportedUseCases() -> [SystemLanguageModelUseCase] {
        [.general, .contentTagging]
    }

    func supportedGuardrails() -> [SystemLanguageModelGuardrails] {
        [.default, .permissiveContentTransformations]
    }

    func checkAvailability() -> Bool {
        let model = SystemLanguageModel.default
        return model.availability == .available
    }

    func getAvailabilityStatus()
        -> (
            available: Bool,
            reason: SystemLanguageModelUnavailableReason?,
            message: String
        )
    {
        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            return (true, nil, "Foundation Models is available")
        case .unavailable(let reason):
            switch reason {
            case .deviceNotEligible:
                return (
                    false,
                    .deviceNotEligible,
                    "Device not supported - requires Apple Silicon Mac"
                )
            case .appleIntelligenceNotEnabled:
                return (
                    false,
                    .appleIntelligenceNotEnabled,
                    "Apple Intelligence is not enabled in System Settings"
                )
            case .modelNotReady:
                return (
                    false,
                    .modelNotReady,
                    "Model is not ready - please wait for download to complete"
                )
            @unknown default:
                return (false, .unknown, "Foundation Models unavailable: \(reason)")
            }
        @unknown default:
            return (false, .unknown, "Unknown availability status")
        }
    }

    func createSession(request: SessionCreateRequest) throws -> SessionCreateResponse {
        let sessionID = "sess-\(UUID().uuidString.lowercased())"
        let model = request.model ?? defaultSessionModelConfiguration()
        let session = try makeSession(
            instructions: request.instructions,
            model: model,
            transcriptJSON: request.transcriptJSON
        )
        sessions[sessionID] = SessionRecord(
            session: session,
            instructions: request.instructions,
            model: model,
            tools: request.tools,
            isResponding: false
        )
        return SessionCreateResponse(session: try sessionState(sessionID: sessionID))
    }

    func session(sessionID: String) throws -> SessionState {
        try sessionState(sessionID: sessionID)
    }

    func deleteSession(sessionID: String) throws {
        guard sessions.removeValue(forKey: sessionID) != nil else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
    }

    func resetSession(sessionID: String) throws -> SessionState {
        guard let record = sessions[sessionID] else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                "Apple FM session '\(sessionID)' cannot reset while a request is active"
            )
        }
        return try sessionState(sessionID: sessionID)
    }

    func respond(sessionID: String, request: SessionRespondRequest) async throws -> SessionRespondResponse {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                "Apple FM session '\(sessionID)' already has an in-flight request"
            )
        }

        record.isResponding = true
        sessions[sessionID] = record

        let startTime = Date()
        do {
            let response: LanguageModelSession.Response<String> = try await record.session.respond(
                to: request.prompt
            )
            let content = response.content
            let promptTokens = request.prompt.count / 4
            let completionTokens = content.count / 4

            record.isResponding = false
            sessions[sessionID] = record

            return SessionRespondResponse(
                session: try sessionState(sessionID: sessionID),
                model: record.model.id,
                output: content,
                usage: Usage(
                    promptTokens: promptTokens,
                    completionTokens: completionTokens,
                    totalTokens: promptTokens + completionTokens
                )
            )
        } catch let error as LanguageModelSession.GenerationError {
            record.isResponding = false
            sessions[sessionID] = record
            if case .concurrentRequests = error {
                throw FMError.concurrentRequests(
                    "Apple FM session '\(sessionID)' rejected overlapping requests"
                )
            }
            throw FMError.requestFailed(
                "Foundation Models session request failed after \(Date().timeIntervalSince(startTime))s: \(error.localizedDescription)"
            )
        } catch {
            record.isResponding = false
            sessions[sessionID] = record
            throw FMError.requestFailed(
                "Foundation Models session request failed: \(error.localizedDescription)"
            )
        }
    }

    func handleCompletion(request: ChatCompletionRequest) async throws -> ChatCompletionResponse {
        guard checkAvailability() else {
            let (_, _, message) = getAvailabilityStatus()
            throw FMError.modelUnavailable(message)
        }

        let prompt = buildPrompt(from: request.messages)
        let session = try makeSession(
            instructions: nil,
            model: defaultSessionModelConfiguration(),
            transcriptJSON: nil
        )

        let startTime = Date()
        let response: LanguageModelSession.Response<String>
        do {
            response = try await session.respond(to: prompt)
        } catch {
            throw FMError.requestFailed(
                "Foundation Models request failed: \(error.localizedDescription)"
            )
        }

        let content = response.content
        let promptTokens = prompt.count / 4
        let completionTokens = content.count / 4

        return ChatCompletionResponse(
            id: "fm-\(UUID().uuidString.lowercased())",
            object: "chat.completion",
            created: Int(startTime.timeIntervalSince1970),
            model: defaultSessionModelConfiguration().id,
            choices: [
                Choice(
                    index: 0,
                    message: ResponseMessage(role: "assistant", content: content),
                    finishReason: "stop"
                )
            ],
            usage: Usage(
                promptTokens: promptTokens,
                completionTokens: completionTokens,
                totalTokens: promptTokens + completionTokens
            )
        )
    }

    private func defaultSessionModelConfiguration() -> SessionModelConfiguration {
        SessionModelConfiguration(
            id: "apple-foundation-model",
            useCase: defaultUseCase,
            guardrails: defaultGuardrails
        )
    }

    private func makeSession(
        instructions: String?,
        model: SessionModelConfiguration,
        transcriptJSON: String?
    ) throws -> LanguageModelSession {
        let foundationModel = SystemLanguageModel(
            useCase: model.useCase.foundationModelsValue,
            guardrails: model.guardrails.foundationModelsValue
        )
        let tools: [any Tool] = []
        if let transcriptJSON {
            let transcript = try JSONDecoder().decode(
                Transcript.self,
                from: Data(transcriptJSON.utf8)
            )
            return LanguageModelSession(
                model: foundationModel,
                tools: tools,
                transcript: transcript
            )
        }
        return LanguageModelSession(
            model: foundationModel,
            tools: tools,
            instructions: instructions
        )
    }

    private func sessionState(sessionID: String) throws -> SessionState {
        guard let record = sessions[sessionID] else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
        return SessionState(
            id: sessionID,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            isResponding: record.isResponding,
            transcriptJSON: try transcriptJSONString(for: record.session)
        )
    }

    private func transcriptJSONString(for session: LanguageModelSession) throws -> String? {
        let transcriptData = try JSONEncoder().encode(session.transcript)
        return String(data: transcriptData, encoding: .utf8)
    }

    private func buildPrompt(from messages: [ChatMessage]) -> String {
        var parts: [String] = []

        for message in messages {
            switch message.role {
            case "system":
                parts.append("System: \(message.content)")
            case "user":
                parts.append("User: \(message.content)")
            case "assistant":
                parts.append("Assistant: \(message.content)")
            case "tool":
                parts.append("Tool Result: \(message.content)")
            default:
                parts.append("\(message.role): \(message.content)")
            }
        }

        parts.append("Assistant:")
        return parts.joined(separator: "\n\n")
    }
}
