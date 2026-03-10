import Foundation
import FoundationModels

actor ChatHandler {
    private struct SessionRecord {
        let session: LanguageModelSession
        let instructions: String?
        let model: SessionModelConfiguration
        let tools: [SessionToolMetadata]
        var isResponding: Bool
        var transcriptJSONSnapshot: String?
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
        if request.transcriptJSON != nil && request.transcript != nil {
            throw FMError.invalidRequest(
                "Provide only one of 'transcript_json' or 'transcript' when restoring an Apple FM session"
            )
        }
        let sessionID = "sess-\(UUID().uuidString.lowercased())"
        let model = request.model ?? defaultSessionModelConfiguration()
        let session = try makeSession(
            instructions: request.instructions,
            model: model,
            transcriptJSON: request.transcriptJSON,
            transcript: request.transcript
        )
        let transcriptJSONSnapshot = try transcriptJSONString(for: session)
        sessions[sessionID] = SessionRecord(
            session: session,
            instructions: request.instructions,
            model: model,
            tools: request.tools,
            isResponding: false,
            transcriptJSONSnapshot: transcriptJSONSnapshot
        )
        return SessionCreateResponse(session: try sessionState(sessionID: sessionID))
    }

    func sessionTranscript(sessionID: String) throws -> Transcript {
        guard let record = sessions[sessionID] else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
        if let transcriptJSON = record.transcriptJSONSnapshot {
            return try decodedTranscript(fromJSONString: transcriptJSON)
        }
        return record.session.transcript
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
        sessions[sessionID] = try restoredRecord(from: record)
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
        let options = try bridgeGenerationOptions(from: request.options)
        do {
            let response: LanguageModelSession.Response<String> = try await record.session.respond(
                to: request.prompt,
                options: options
            )
            let content = response.content

            record.isResponding = false
            record.transcriptJSONSnapshot = try transcriptJSONString(for: record.session)
            sessions[sessionID] = record

            return SessionRespondResponse(
                session: try sessionState(sessionID: sessionID),
                model: record.model.id,
                output: content,
                usage: estimatedUsage(prompt: request.prompt, output: content)
            )
        } catch let error as LanguageModelSession.GenerationError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            if case .concurrentRequests = error {
                throw FMError.concurrentRequests(
                    "Apple FM session '\(sessionID)' rejected overlapping requests"
                )
            }
            throw FMError.requestFailed(
                "Foundation Models session request failed after \(Date().timeIntervalSince(startTime))s: \(error.localizedDescription)"
            )
        } catch {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw FMError.requestFailed(
                "Foundation Models session request failed: \(error.localizedDescription)"
            )
        }
    }

    func respondStructured(
        sessionID: String,
        request: SessionStructuredResponseRequest
    ) async throws -> SessionStructuredResponseResponse {
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
        let options = try bridgeGenerationOptions(from: request.options)
        let schema = try generationSchema(from: request.schema)
        do {
            let response: LanguageModelSession.Response<GeneratedContent> = try await record.session
                .respond(
                    to: request.prompt,
                    schema: schema,
                    options: options
                )
            let payload = try generatedContentPayload(from: response.content)
            let outputJSON = try payload.contentJSONString()

            record.isResponding = false
            record.transcriptJSONSnapshot = try transcriptJSONString(for: record.session)
            sessions[sessionID] = record

            return SessionStructuredResponseResponse(
                session: try sessionState(sessionID: sessionID),
                model: record.model.id,
                content: payload,
                usage: estimatedUsage(prompt: request.prompt, output: outputJSON)
            )
        } catch let error as LanguageModelSession.GenerationError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            if case .concurrentRequests = error {
                throw FMError.concurrentRequests(
                    "Apple FM session '\(sessionID)' rejected overlapping requests"
                )
            }
            throw FMError.requestFailed(
                "Foundation Models structured request failed after \(Date().timeIntervalSince(startTime))s: \(error.localizedDescription)"
            )
        } catch let error as FMError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw error
        } catch {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw FMError.requestFailed(
                "Foundation Models structured request failed: \(error.localizedDescription)"
            )
        }
    }

    func streamResponse(
        sessionID: String,
        request: SessionRespondRequest
    ) throws -> AsyncThrowingStream<TextStreamEvent, Error> {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                "Apple FM session '\(sessionID)' already has an in-flight request"
            )
        }

        let options = try bridgeGenerationOptions(from: request.options)
        record.isResponding = true
        sessions[sessionID] = record

        let session = record.session
        let modelID = record.model.id
        let prompt = request.prompt

        return AsyncThrowingStream { continuation in
            let task = Task {
                var finalOutput = ""
                do {
                    let stream = session.streamResponse(to: prompt, options: options)
                    for try await snapshot in stream {
                        try Task.checkCancellation()
                        finalOutput = snapshot.content
                        continuation.yield(
                            TextStreamEvent(
                                kind: .snapshot,
                                model: modelID,
                                output: finalOutput,
                                session: nil,
                                usage: nil
                            )
                        )
                    }

                    let completedEvent = try self.finishSuccessfulStream(
                        sessionID: sessionID,
                        prompt: prompt,
                        finalOutput: finalOutput
                    )
                    continuation.yield(completedEvent)
                    continuation.finish()
                } catch is CancellationError {
                    self.cancelStream(sessionID: sessionID)
                    continuation.finish()
                } catch let error as LanguageModelSession.GenerationError {
                    self.cancelStream(sessionID: sessionID)
                    if case .concurrentRequests = error {
                        continuation.finish(
                            throwing: FMError.concurrentRequests(
                                "Apple FM session '\(sessionID)' rejected overlapping requests"
                            )
                        )
                    } else {
                        continuation.finish(
                            throwing: FMError.requestFailed(
                                "Foundation Models session stream failed: \(error.localizedDescription)"
                            )
                        )
                    }
                } catch {
                    self.cancelStream(sessionID: sessionID)
                    continuation.finish(
                        throwing: FMError.requestFailed(
                            "Foundation Models session stream failed: \(error.localizedDescription)"
                        )
                    )
                }
            }
            continuation.onTermination = { @Sendable _ in
                task.cancel()
            }
        }
    }

    func cancelStreamingSession(sessionID: String) {
        cancelStream(sessionID: sessionID)
    }

    func handleCompletion(request: ChatCompletionRequest) async throws -> ChatCompletionResponse {
        guard checkAvailability() else {
            let (_, _, message) = getAvailabilityStatus()
            throw FMError.modelUnavailable(message)
        }

        if request.stream ?? false {
            throw FMError.invalidRequest("Streaming is not supported by this bridge yet")
        }

        let model = try resolvedChatModelID(request.model)
        let options = try bridgeGenerationOptions(
            from: mergedGenerationOptions(
                options: request.options,
                legacyTemperature: request.temperature,
                legacyMaxTokens: request.maxTokens
            )
        )

        let prompt = buildPrompt(from: request.messages)
        let session = try makeSession(
            instructions: nil,
            model: defaultSessionModelConfiguration(),
            transcriptJSON: nil,
            transcript: nil
        )

        let startTime = Date()
        let response: LanguageModelSession.Response<String>
        do {
            response = try await session.respond(to: prompt, options: options)
        } catch {
            throw FMError.requestFailed(
                "Foundation Models request failed: \(error.localizedDescription)"
            )
        }

        let content = response.content

        return ChatCompletionResponse(
            id: "fm-\(UUID().uuidString.lowercased())",
            object: "chat.completion",
            created: Int(startTime.timeIntervalSince1970),
            model: model,
            choices: [
                Choice(
                    index: 0,
                    message: ResponseMessage(role: "assistant", content: content),
                    finishReason: "stop"
                )
            ],
            usage: estimatedUsage(prompt: prompt, output: content)
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
        transcriptJSON: String?,
        transcript: Transcript?
    ) throws -> LanguageModelSession {
        let foundationModel = SystemLanguageModel(
            useCase: model.useCase.foundationModelsValue,
            guardrails: model.guardrails.foundationModelsValue
        )
        let tools: [any Tool] = []
        if let transcript {
            return LanguageModelSession(
                model: foundationModel,
                tools: tools,
                transcript: transcript
            )
        }
        if let transcriptJSON {
            let transcript = try decodedTranscript(fromJSONString: transcriptJSON)
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
            transcriptJSON: record.transcriptJSONSnapshot
        )
    }

    private func transcriptJSONString(for session: LanguageModelSession) throws -> String? {
        let transcriptData = try JSONEncoder().encode(session.transcript)
        return String(data: transcriptData, encoding: .utf8)
    }

    private func generationSchema(from value: JSONValue) throws -> GenerationSchema {
        let schemaData = try JSONEncoder().encode(value)
        do {
            return try JSONDecoder().decode(GenerationSchema.self, from: schemaData)
        } catch {
            throw FMError.invalidRequest(
                "Invalid Apple FM generation schema: \(error.localizedDescription)"
            )
        }
    }

    private func generatedContentPayload(from content: GeneratedContent) throws -> GeneratedContentPayload {
        let jsonString = content.jsonString
        let jsonValue = try decodedJSONValue(fromJSONString: jsonString)
        return GeneratedContentPayload(
            generationID: "gen-\(UUID().uuidString.lowercased())",
            content: jsonValue,
            isComplete: content.isComplete
        )
    }

    private func decodedJSONValue(fromJSONString jsonString: String) throws -> JSONValue {
        do {
            return try JSONDecoder().decode(
                JSONValue.self,
                from: Data(jsonString.utf8)
            )
        } catch {
            throw FMError.serverError(
                "Failed to decode Apple FM structured content: \(error.localizedDescription)"
            )
        }
    }

    private func decodedTranscript(fromJSONString transcriptJSON: String) throws -> Transcript {
        do {
            return try JSONDecoder().decode(
                Transcript.self,
                from: Data(transcriptJSON.utf8)
            )
        } catch {
            throw FMError.invalidRequest(
                "Invalid Apple FM transcript: \(error.localizedDescription)"
            )
        }
    }

    private func restoredRecord(from record: SessionRecord) throws -> SessionRecord {
        let session = try makeSession(
            instructions: record.instructions,
            model: record.model,
            transcriptJSON: record.transcriptJSONSnapshot,
            transcript: nil
        )
        return SessionRecord(
            session: session,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            isResponding: false,
            transcriptJSONSnapshot: record.transcriptJSONSnapshot
        )
    }

    private func recoveryRecord(afterFailureOf record: SessionRecord) -> SessionRecord {
        if let restored = try? restoredRecord(from: record) {
            return restored
        }
        var fallback = record
        fallback.isResponding = false
        return fallback
    }

    private func cancelStream(sessionID: String) {
        guard let record = sessions[sessionID] else {
            return
        }
        sessions[sessionID] = recoveryRecord(afterFailureOf: record)
    }

    private func finishSuccessfulStream(
        sessionID: String,
        prompt: String,
        finalOutput: String
    ) throws -> TextStreamEvent {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest("Unknown Apple FM session '\(sessionID)'")
        }
        record.isResponding = false
        record.transcriptJSONSnapshot = try transcriptJSONString(for: record.session)
        sessions[sessionID] = record
        return TextStreamEvent(
            kind: .completed,
            model: record.model.id,
            output: finalOutput,
            session: try sessionState(sessionID: sessionID),
            usage: estimatedUsage(prompt: prompt, output: finalOutput)
        )
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

    private func resolvedChatModelID(_ requestedModel: String?) throws -> String {
        let defaultModel = defaultSessionModelConfiguration().id
        guard let requestedModel, !requestedModel.isEmpty else {
            return defaultModel
        }
        guard requestedModel == defaultModel else {
            throw FMError.invalidRequest(
                "Unsupported Apple FM model '\(requestedModel)'. Only '\(defaultModel)' is available."
            )
        }
        return defaultModel
    }

    private func mergedGenerationOptions(
        options: GenerationOptionsPayload?,
        legacyTemperature: Double?,
        legacyMaxTokens: Int?
    ) throws -> GenerationOptionsPayload? {
        let mergedTemperature: Double?
        if let payloadTemperature = options?.temperature, let legacyTemperature,
            payloadTemperature != legacyTemperature
        {
            throw FMError.invalidRequest(
                "Conflicting temperature values were provided in both 'options.temperature' and 'temperature'"
            )
        } else {
            mergedTemperature = options?.temperature ?? legacyTemperature
        }

        let mergedMaxTokens: Int?
        if let payloadMax = options?.maximumResponseTokens, let legacyMaxTokens,
            payloadMax != legacyMaxTokens
        {
            throw FMError.invalidRequest(
                "Conflicting maximum response token values were provided in both 'options.maximum_response_tokens' and 'max_tokens'"
            )
        } else {
            mergedMaxTokens = options?.maximumResponseTokens ?? legacyMaxTokens
        }

        if options == nil && mergedTemperature == nil && mergedMaxTokens == nil {
            return nil
        }

        return GenerationOptionsPayload(
            sampling: options?.sampling,
            temperature: mergedTemperature,
            maximumResponseTokens: mergedMaxTokens
        )
    }

    private func bridgeGenerationOptions(from payload: GenerationOptionsPayload?) throws -> GenerationOptions {
        var options = GenerationOptions()
        guard let payload else {
            return options
        }

        if let temperature = payload.temperature {
            guard temperature >= 0 else {
                throw FMError.invalidRequest("'temperature' must be non-negative")
            }
            options.temperature = temperature
        }

        if let maximumResponseTokens = payload.maximumResponseTokens {
            guard maximumResponseTokens > 0 else {
                throw FMError.invalidRequest("'maximum_response_tokens' must be positive")
            }
            options.maximumResponseTokens = maximumResponseTokens
        }

        if let sampling = payload.sampling {
            options.sampling = try foundationSampling(from: sampling)
        }

        return options
    }

    private func foundationSampling(
        from sampling: SamplingMode
    ) throws -> GenerationOptions.SamplingMode {
        switch sampling.mode {
        case .greedy:
            if sampling.topK != nil {
                throw FMError.invalidRequest("greedy sampling does not accept 'top'")
            }
            if sampling.topP != nil {
                throw FMError.invalidRequest(
                    "greedy sampling does not accept 'probability_threshold'"
                )
            }
            if sampling.seed != nil {
                throw FMError.invalidRequest("greedy sampling does not accept 'seed'")
            }
            return .greedy
        case .random:
            if let topK = sampling.topK, topK <= 0 {
                throw FMError.invalidRequest("'top' must be a positive integer")
            }
            if sampling.topK != nil && sampling.topP != nil {
                throw FMError.invalidRequest(
                    "Cannot specify both 'top' and 'probability_threshold'. Choose one sampling constraint."
                )
            }
            if let probabilityThreshold = sampling.topP,
                probabilityThreshold < 0 || probabilityThreshold > 1
            {
                throw FMError.invalidRequest(
                    "'probability_threshold' must be between 0.0 and 1.0"
                )
            }
            if let topK = sampling.topK {
                return .random(top: topK, seed: sampling.seed)
            }
            let probabilityThreshold = sampling.topP ?? 1.0
            return .random(probabilityThreshold: probabilityThreshold, seed: sampling.seed)
        }
    }

    private func estimatedUsage(prompt: String, output: String) -> Usage {
        let promptTokens = (prompt.count + 3) / 4
        let completionTokens = (output.count + 3) / 4
        let totalTokens = promptTokens + completionTokens
        return Usage(
            promptTokens: nil,
            completionTokens: nil,
            totalTokens: nil,
            promptTokensDetail: UsageMeasurement(value: promptTokens, truth: .estimated),
            completionTokensDetail: UsageMeasurement(
                value: completionTokens,
                truth: .estimated
            ),
            totalTokensDetail: UsageMeasurement(value: totalTokens, truth: .estimated)
        )
    }
}
