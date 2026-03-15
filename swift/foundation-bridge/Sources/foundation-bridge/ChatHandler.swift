import Foundation
import FoundationModels

struct RemoteToolCallError: Error {
    let toolName: String
    let underlyingError: String
}

final class RemoteTool: Tool {
    let name: String
    let description: String
    let parameters: GenerationSchema

    private let callbackURL: URL
    private let sessionToken: String

    init(
        definition: ToolDefinition,
        callback: ToolCallbackConfiguration
    ) throws {
        self.name = definition.name
        self.description = definition.description ?? definition.name
        self.parameters = try ChatHandler.decodeGenerationSchema(from: definition.argumentsSchema)
        guard let callbackURL = URL(string: callback.url) else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Invalid Apple FM tool callback URL '\(callback.url)'")
            )
        }
        self.callbackURL = callbackURL
        self.sessionToken = callback.sessionToken
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let argumentsPayload = try ChatHandler.generatedContentPayload(from: arguments)
        let payload = ToolCallRequestPayload(
            sessionToken: sessionToken,
            toolName: name,
            arguments: argumentsPayload
        )
        let requestData = try JSONEncoder().encode(payload)

        var request = URLRequest(url: callbackURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = requestData

        let (data, response) = try await URLSession.shared.data(for: request)
        let statusCode = (response as? HTTPURLResponse)?.statusCode ?? 500
        if (200..<300).contains(statusCode) {
            let payload = try JSONDecoder().decode(ToolCallResponsePayload.self, from: data)
            return payload.output
        }

        if let toolError = try? JSONDecoder().decode(ToolCallErrorPayload.self, from: data) {
            throw RemoteToolCallError(
                toolName: toolError.toolName,
                underlyingError: toolError.underlyingError
            )
        }

        let errorMessage = String(data: data, encoding: .utf8) ?? "unknown tool callback failure"
        throw RemoteToolCallError(toolName: name, underlyingError: errorMessage)
    }
}

actor ChatHandler {
    private struct AdapterPackageMetadata {
        let adapterIdentifier: String
        let baseModelSignature: String?
        let packageFormatVersion: String?
        let draftModelPresent: Bool
    }

    private struct LoadedAdapterRecord {
        let runtimeAdapter: SystemLanguageModel.Adapter
        let packageURL: URL
        let selection: AdapterSelection
        let baseModelSignature: String?
        let packageFormatVersion: String?
        let draftModelPresent: Bool
        var attachedSessionIDs: Set<String>

        func inventoryEntry(
            reasonCode: String? = nil,
            message: String? = "Adapter compiled and ready"
        ) -> AdapterInventoryEntry {
            AdapterInventoryEntry(
                adapter: selection,
                baseModelSignature: baseModelSignature,
                packageFormatVersion: packageFormatVersion,
                draftModelPresent: draftModelPresent,
                compatibility: AdapterCompatibility(
                    compatible: true,
                    reasonCode: reasonCode,
                    message: message
                ),
                attachedSessionIDs: attachedSessionIDs.sorted()
            )
        }
    }

    private struct SessionRecord {
        let session: LanguageModelSession
        let instructions: String?
        let model: SessionModelConfiguration
        let tools: [SessionToolMetadata]
        let toolDefinitions: [ToolDefinition]
        let adapterSelection: AdapterSelection?
        let toolCallback: ToolCallbackConfiguration?
        var isResponding: Bool
        var transcriptJSONSnapshot: String?
    }

    private var sessions: [String: SessionRecord] = [:]
    private var loadedAdapters: [String: LoadedAdapterRecord] = [:]

    init() {}

    private let defaultUseCase: SystemLanguageModelUseCase = .general
    private let defaultGuardrails: SystemLanguageModelGuardrails = .default
    private let supportedPackageFormatVersions: Set<String> = [
        "openagents.apple-fmadapter.v1"
    ]

    func supportedUseCases() -> [SystemLanguageModelUseCase] {
        [.general, .contentTagging]
    }

    func supportedGuardrails() -> [SystemLanguageModelGuardrails] {
        [.default, .permissiveContentTransformations]
    }

    func listAdapters() -> AdaptersResponse {
        AdaptersResponse(
            adapters: adapterInventory(),
            attachSupported: true
        )
    }

    func loadAdapter(request: AdapterLoadRequest) async throws -> AdapterLoadResponse {
        let packageURL = URL(fileURLWithPath: request.packagePath)
        let metadata = try validateAdapterPackage(at: packageURL)
        let adapterID = request.requestedAdapterID ?? metadata.adapterIdentifier

        if let existing = loadedAdapters[adapterID], !existing.attachedSessionIDs.isEmpty {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter '\(adapterID)' is already loaded and attached to active sessions",
                    failureReason: "adapter_in_use"
                )
            )
        }

        do {
            let runtimeAdapter = try SystemLanguageModel.Adapter(fileURL: packageURL)
            try await runtimeAdapter.compile()
            let record = LoadedAdapterRecord(
                runtimeAdapter: runtimeAdapter,
                packageURL: packageURL,
                selection: AdapterSelection(
                    adapterID: adapterID,
                    packageDigest: nil
                ),
                baseModelSignature: metadata.baseModelSignature,
                packageFormatVersion: metadata.packageFormatVersion,
                draftModelPresent: metadata.draftModelPresent,
                attachedSessionIDs: []
            )
            loadedAdapters[adapterID] = record
            return AdapterLoadResponse(adapter: record.inventoryEntry())
        } catch {
            throw adapterLoadFailure(
                adapterID: adapterID,
                packageURL: packageURL,
                error: error
            )
        }
    }

    func unloadAdapter(adapterID: String) throws {
        guard let record = loadedAdapters[adapterID] else {
            throw FMError.adapterNotFound(
                FMErrorPayload(message: "Unknown Apple FM adapter '\(adapterID)'")
            )
        }
        if !record.attachedSessionIDs.isEmpty {
            throw FMError.invalidRequest(
                FMErrorPayload(
                    message: "Adapter '\(adapterID)' cannot be unloaded while attached to active sessions",
                    failureReason: "adapter_in_use"
                )
            )
        }
        _ = loadedAdapters.removeValue(forKey: adapterID)
    }

    func attachSessionAdapter(
        sessionID: String,
        request: AdapterAttachRequest
    ) throws -> SessionState {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' cannot change adapters while a request is active"
                )
            )
        }
        if record.adapterSelection == request.adapter {
            return try sessionState(sessionID: sessionID)
        }
        if record.adapterSelection != nil {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' already has an attached adapter; detach it before attaching a different one",
                    failureReason: "attach_conflict"
                )
            )
        }

        let transcriptJSON = record.transcriptJSONSnapshot
        let session = try makeSession(
            instructions: record.instructions,
            model: record.model,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: request.adapter,
            toolCallback: record.toolCallback,
            transcriptJSON: transcriptJSON,
            transcript: nil
        )
        record = SessionRecord(
            session: session,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: request.adapter,
            toolCallback: record.toolCallback,
            isResponding: false,
            transcriptJSONSnapshot: transcriptJSON
        )
        sessions[sessionID] = record
        updateAttachmentIndex(sessionID: sessionID, from: nil, to: request.adapter)
        return try sessionState(sessionID: sessionID)
    }

    func detachSessionAdapter(sessionID: String) throws -> SessionState {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' cannot change adapters while a request is active"
                )
            )
        }
        guard let previousAdapter = record.adapterSelection else {
            return try sessionState(sessionID: sessionID)
        }

        let transcriptJSON = record.transcriptJSONSnapshot
        let session = try makeSession(
            instructions: record.instructions,
            model: record.model,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: nil,
            toolCallback: record.toolCallback,
            transcriptJSON: transcriptJSON,
            transcript: nil
        )
        record = SessionRecord(
            session: session,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: nil,
            toolCallback: record.toolCallback,
            isResponding: false,
            transcriptJSONSnapshot: transcriptJSON
        )
        sessions[sessionID] = record
        updateAttachmentIndex(sessionID: sessionID, from: previousAdapter, to: nil)
        return try sessionState(sessionID: sessionID)
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
                FMErrorPayload(
                    message: "Provide only one of 'transcript_json' or 'transcript' when restoring an Apple FM session"
                )
            )
        }
        let sessionID = "sess-\(UUID().uuidString.lowercased())"
        let model = request.model ?? defaultSessionModelConfiguration()
        let session = try makeSession(
            instructions: request.instructions,
            model: model,
            toolDefinitions: request.tools,
            adapterSelection: request.adapter,
            toolCallback: request.toolCallback,
            transcriptJSON: request.transcriptJSON,
            transcript: request.transcript
        )
        let transcriptJSONSnapshot = try transcriptJSONString(for: session)
        sessions[sessionID] = SessionRecord(
            session: session,
            instructions: request.instructions,
            model: model,
            tools: request.tools.map(\.metadata),
            toolDefinitions: request.tools,
            adapterSelection: request.adapter,
            toolCallback: request.toolCallback,
            isResponding: false,
            transcriptJSONSnapshot: transcriptJSONSnapshot
        )
        updateAttachmentIndex(sessionID: sessionID, from: nil, to: request.adapter)
        return SessionCreateResponse(session: try sessionState(sessionID: sessionID))
    }

    func sessionTranscript(sessionID: String) throws -> Transcript {
        guard let record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
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
        guard let removed = sessions.removeValue(forKey: sessionID) else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        updateAttachmentIndex(sessionID: sessionID, from: removed.adapterSelection, to: nil)
    }

    func resetSession(sessionID: String) throws -> SessionState {
        guard let record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' cannot reset while a request is active"
                )
            )
        }
        sessions[sessionID] = try restoredRecord(from: record)
        return try sessionState(sessionID: sessionID)
    }

    func respond(sessionID: String, request: SessionRespondRequest) async throws -> SessionRespondResponse {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' already has an in-flight request"
                )
            )
        }

        record.isResponding = true
        sessions[sessionID] = record

        let options = try bridgeGenerationOptions(from: request.options)
        let executionSession = try sessionForExecution(record: record, requestAdapter: request.adapter)
        let usesTemporaryOverride = request.adapter != nil && request.adapter != record.adapterSelection
        do {
            let response: LanguageModelSession.Response<String> = try await executionSession.respond(
                to: request.prompt,
                options: options
            )
            let content = response.content

            record = try successfulRecord(
                from: record,
                executedSession: executionSession,
                temporaryOverride: usesTemporaryOverride
            )
            sessions[sessionID] = record

            return SessionRespondResponse(
                session: try sessionState(sessionID: sessionID),
                model: record.model.id,
                output: content,
                usage: estimatedUsage(prompt: request.prompt, output: content)
            )
        } catch let error as LanguageModelSession.ToolCallError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw toolCallFailure(from: error)
        } catch let error as LanguageModelSession.GenerationError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw await generationFailure(
                from: error,
                fallbackPrefix: "Foundation Models session request failed"
            )
        } catch {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw FMError.serverError(
                FMErrorPayload(
                    message: "Foundation Models session request failed: \(error.localizedDescription)",
                    debugDescription: String(reflecting: error)
                )
            )
        }
    }

    func respondStructured(
        sessionID: String,
        request: SessionStructuredResponseRequest
    ) async throws -> SessionStructuredResponseResponse {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' already has an in-flight request"
                )
            )
        }

        record.isResponding = true
        sessions[sessionID] = record

        let options = try bridgeGenerationOptions(from: request.options)
        let schema = try Self.decodeGenerationSchema(from: request.schema)
        let executionSession = try sessionForExecution(record: record, requestAdapter: request.adapter)
        let usesTemporaryOverride = request.adapter != nil && request.adapter != record.adapterSelection
        do {
            let response: LanguageModelSession.Response<GeneratedContent> = try await executionSession
                .respond(
                    to: request.prompt,
                    schema: schema,
                    options: options
                )
            let payload = try Self.generatedContentPayload(from: response.content)
            let outputJSON = try payload.contentJSONString()

            record = try successfulRecord(
                from: record,
                executedSession: executionSession,
                temporaryOverride: usesTemporaryOverride
            )
            sessions[sessionID] = record

            return SessionStructuredResponseResponse(
                session: try sessionState(sessionID: sessionID),
                model: record.model.id,
                content: payload,
                usage: estimatedUsage(prompt: request.prompt, output: outputJSON)
            )
        } catch let error as LanguageModelSession.ToolCallError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw toolCallFailure(from: error)
        } catch let error as LanguageModelSession.GenerationError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw await generationFailure(
                from: error,
                fallbackPrefix: "Foundation Models structured request failed"
            )
        } catch let error as FMError {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw error
        } catch {
            sessions[sessionID] = recoveryRecord(afterFailureOf: record)
            throw FMError.serverError(
                FMErrorPayload(
                    message: "Foundation Models structured request failed: \(error.localizedDescription)",
                    debugDescription: String(reflecting: error)
                )
            )
        }
    }

    func streamResponse(
        sessionID: String,
        request: SessionRespondRequest
    ) throws -> AsyncThrowingStream<TextStreamEvent, Error> {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        if record.isResponding {
            throw FMError.concurrentRequests(
                FMErrorPayload(
                    message: "Apple FM session '\(sessionID)' already has an in-flight request"
                )
            )
        }

        let options = try bridgeGenerationOptions(from: request.options)
        record.isResponding = true
        sessions[sessionID] = record

        let session = try sessionForExecution(record: record, requestAdapter: request.adapter)
        let usesTemporaryOverride = request.adapter != nil && request.adapter != record.adapterSelection
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
                        finalOutput: finalOutput,
                        executedSession: session,
                        temporaryOverride: usesTemporaryOverride
                    )
                    continuation.yield(completedEvent)
                    continuation.finish()
                } catch is CancellationError {
                    self.cancelStream(sessionID: sessionID)
                    continuation.finish()
                } catch let error as LanguageModelSession.GenerationError {
                    self.cancelStream(sessionID: sessionID)
                    let mapped = await self.generationFailure(
                        from: error,
                        fallbackPrefix: "Foundation Models session stream failed"
                    )
                    continuation.finish(throwing: mapped)
                } catch {
                    self.cancelStream(sessionID: sessionID)
                    continuation.finish(
                        throwing: FMError.serverError(
                            FMErrorPayload(
                                message: "Foundation Models session stream failed: \(error.localizedDescription)",
                                debugDescription: String(reflecting: error)
                            )
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
            throw FMError.assetsUnavailable(FMErrorPayload(message: message))
        }

        if request.stream ?? false {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Streaming is not supported by this bridge yet")
            )
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
            toolDefinitions: [],
            adapterSelection: request.adapter,
            toolCallback: nil,
            transcriptJSON: nil,
            transcript: nil
        )

        let response: LanguageModelSession.Response<String>
        do {
            response = try await session.respond(to: prompt, options: options)
        } catch let error as LanguageModelSession.ToolCallError {
            throw toolCallFailure(from: error)
        } catch let error as LanguageModelSession.GenerationError {
            throw await generationFailure(
                from: error,
                fallbackPrefix: "Foundation Models request failed"
            )
        } catch {
            throw FMError.serverError(
                FMErrorPayload(
                    message: "Foundation Models request failed: \(error.localizedDescription)",
                    debugDescription: String(reflecting: error)
                )
            )
        }

        let content = response.content

        return ChatCompletionResponse(
            id: "fm-\(UUID().uuidString.lowercased())",
            object: "chat.completion",
            created: Int(Date().timeIntervalSince1970),
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
        toolDefinitions: [ToolDefinition],
        adapterSelection: AdapterSelection?,
        toolCallback: ToolCallbackConfiguration?,
        transcriptJSON: String?,
        transcript: Transcript?
    ) throws -> LanguageModelSession {
        let foundationModel = try resolvedFoundationModel(
            model: model,
            adapterSelection: adapterSelection
        )
        let tools = try makeTools(
            definitions: toolDefinitions,
            callback: toolCallback
        )
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
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        return SessionState(
            id: sessionID,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            adapter: record.adapterSelection,
            isResponding: record.isResponding,
            transcriptJSON: record.transcriptJSONSnapshot
        )
    }

    private func transcriptJSONString(for session: LanguageModelSession) throws -> String? {
        let transcriptData = try JSONEncoder().encode(session.transcript)
        return String(data: transcriptData, encoding: .utf8)
    }

    fileprivate static func decodeGenerationSchema(from value: JSONValue) throws -> GenerationSchema {
        let schemaData = try JSONEncoder().encode(value)
        do {
            return try JSONDecoder().decode(GenerationSchema.self, from: schemaData)
        } catch {
            throw FMError.invalidGenerationSchema(
                FMErrorPayload(
                    message: "Invalid Apple FM generation schema: \(error.localizedDescription)",
                    debugDescription: String(reflecting: error)
                )
            )
        }
    }

    fileprivate static func generatedContentPayload(from content: GeneratedContent) throws -> GeneratedContentPayload {
        let jsonString = content.jsonString
        let jsonValue = try decodeJSONValue(fromJSONString: jsonString)
        return GeneratedContentPayload(
            generationID: "gen-\(UUID().uuidString.lowercased())",
            content: jsonValue,
            isComplete: content.isComplete
        )
    }

    fileprivate static func decodeJSONValue(fromJSONString jsonString: String) throws -> JSONValue {
        do {
            return try JSONDecoder().decode(
                JSONValue.self,
                from: Data(jsonString.utf8)
            )
        } catch {
            throw FMError.serverError(
                FMErrorPayload(
                    message: "Failed to decode Apple FM structured content: \(error.localizedDescription)",
                    debugDescription: String(reflecting: error)
                )
            )
        }
    }

    private func makeTools(
        definitions: [ToolDefinition],
        callback: ToolCallbackConfiguration?
    ) throws -> [any Tool] {
        guard let callback else {
            return []
        }
        return try definitions.map { definition in
            try RemoteTool(definition: definition, callback: callback)
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
                FMErrorPayload(
                    message: "Invalid Apple FM transcript: \(error.localizedDescription)",
                    debugDescription: String(reflecting: error)
                )
            )
        }
    }

    private func restoredRecord(from record: SessionRecord) throws -> SessionRecord {
        try restoredRecord(
            from: record,
            transcriptJSONSnapshot: record.transcriptJSONSnapshot,
            adapterSelection: record.adapterSelection
        )
    }

    private func restoredRecord(
        from record: SessionRecord,
        transcriptJSONSnapshot: String?,
        adapterSelection: AdapterSelection?
    ) throws -> SessionRecord {
        let session = try makeSession(
            instructions: record.instructions,
            model: record.model,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: adapterSelection,
            toolCallback: record.toolCallback,
            transcriptJSON: transcriptJSONSnapshot,
            transcript: nil
        )
        return SessionRecord(
            session: session,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: adapterSelection,
            toolCallback: record.toolCallback,
            isResponding: false,
            transcriptJSONSnapshot: transcriptJSONSnapshot
        )
    }

    private func successfulRecord(
        from record: SessionRecord,
        executedSession: LanguageModelSession,
        temporaryOverride: Bool
    ) throws -> SessionRecord {
        let transcriptJSONSnapshot = try transcriptJSONString(for: executedSession)
        if temporaryOverride {
            return try restoredRecord(
                from: record,
                transcriptJSONSnapshot: transcriptJSONSnapshot,
                adapterSelection: record.adapterSelection
            )
        }
        return SessionRecord(
            session: record.session,
            instructions: record.instructions,
            model: record.model,
            tools: record.tools,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: record.adapterSelection,
            toolCallback: record.toolCallback,
            isResponding: false,
            transcriptJSONSnapshot: transcriptJSONSnapshot
        )
    }

    private func sessionForExecution(
        record: SessionRecord,
        requestAdapter: AdapterSelection?
    ) throws -> LanguageModelSession {
        guard let requestAdapter, requestAdapter != record.adapterSelection else {
            return record.session
        }
        return try makeSession(
            instructions: record.instructions,
            model: record.model,
            toolDefinitions: record.toolDefinitions,
            adapterSelection: requestAdapter,
            toolCallback: record.toolCallback,
            transcriptJSON: record.transcriptJSONSnapshot,
            transcript: nil
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
        finalOutput: String,
        executedSession: LanguageModelSession,
        temporaryOverride: Bool
    ) throws -> TextStreamEvent {
        guard var record = sessions[sessionID] else {
            throw FMError.invalidRequest(
                FMErrorPayload(message: "Unknown Apple FM session '\(sessionID)'")
            )
        }
        record = try successfulRecord(
            from: record,
            executedSession: executedSession,
            temporaryOverride: temporaryOverride
        )
        sessions[sessionID] = record
        return TextStreamEvent(
            kind: .completed,
            model: record.model.id,
            output: finalOutput,
            session: try sessionState(sessionID: sessionID),
            usage: estimatedUsage(prompt: prompt, output: finalOutput)
        )
    }

    private func adapterInventory() -> [AdapterInventoryEntry] {
        loadedAdapters
            .values
            .map { $0.inventoryEntry() }
            .sorted { left, right in
                left.adapter.adapterID.localizedCaseInsensitiveCompare(right.adapter.adapterID)
                    == .orderedAscending
            }
    }

    private func updateAttachmentIndex(
        sessionID: String,
        from previous: AdapterSelection?,
        to next: AdapterSelection?
    ) {
        if let previous {
            if var record = loadedAdapters[previous.adapterID] {
                record.attachedSessionIDs.remove(sessionID)
                loadedAdapters[previous.adapterID] = record
            }
        }
        if let next {
            if var record = loadedAdapters[next.adapterID] {
                record.attachedSessionIDs.insert(sessionID)
                loadedAdapters[next.adapterID] = record
            }
        }
    }

    private func resolvedFoundationModel(
        model: SessionModelConfiguration,
        adapterSelection: AdapterSelection?
    ) throws -> SystemLanguageModel {
        if let adapterSelection {
            let adapterRecord = try resolvedLoadedAdapter(selection: adapterSelection)
            return SystemLanguageModel(
                adapter: adapterRecord.runtimeAdapter,
                guardrails: model.guardrails.foundationModelsValue
            )
        }
        return SystemLanguageModel(
            useCase: model.useCase.foundationModelsValue,
            guardrails: model.guardrails.foundationModelsValue
        )
    }

    private func resolvedLoadedAdapter(selection: AdapterSelection) throws -> LoadedAdapterRecord {
        guard let record = loadedAdapters[selection.adapterID] else {
            throw FMError.adapterNotFound(
                FMErrorPayload(
                    message: "Unknown Apple FM adapter '\(selection.adapterID)'"
                )
            )
        }
        if let requestedDigest = selection.packageDigest,
            let loadedDigest = record.selection.packageDigest,
            requestedDigest != loadedDigest
        {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter '\(selection.adapterID)' is loaded with a different package digest",
                    failureReason: "package_digest_mismatch"
                )
            )
        }
        return record
    }

    private func validateAdapterPackage(at packageURL: URL) throws -> AdapterPackageMetadata {
        var isDirectory: ObjCBool = false
        guard FileManager.default.fileExists(
            atPath: packageURL.path,
            isDirectory: &isDirectory
        ), isDirectory.boolValue else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter package path '\(packageURL.path)' does not exist or is not a directory",
                    failureReason: "missing_package_directory"
                )
            )
        }
        guard packageURL.pathExtension == "fmadapter" else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter package '\(packageURL.lastPathComponent)' must end with .fmadapter",
                    failureReason: "invalid_package_extension"
                )
            )
        }

        let metadataURL = packageURL.appendingPathComponent("metadata.json")
        let weightsURL = packageURL.appendingPathComponent("adapter_weights.bin")
        guard FileManager.default.fileExists(atPath: metadataURL.path) else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter package '\(packageURL.lastPathComponent)' is missing metadata.json",
                    failureReason: "missing_metadata"
                )
            )
        }
        guard FileManager.default.fileExists(atPath: weightsURL.path) else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter package '\(packageURL.lastPathComponent)' is missing adapter_weights.bin",
                    failureReason: "missing_weights"
                )
            )
        }

        let draftMilURL = packageURL.appendingPathComponent("draft.mil")
        let draftWeightsURL = packageURL.appendingPathComponent("draft_weights.bin")
        let hasDraftMil = FileManager.default.fileExists(atPath: draftMilURL.path)
        let hasDraftWeights = FileManager.default.fileExists(atPath: draftWeightsURL.path)
        guard hasDraftMil == hasDraftWeights else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter package '\(packageURL.lastPathComponent)' carries only part of the draft-model payload",
                    failureReason: "incomplete_draft_payload"
                )
            )
        }

        let metadataData: Data
        do {
            metadataData = try Data(contentsOf: metadataURL)
        } catch {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Failed to read adapter metadata for '\(packageURL.lastPathComponent)'",
                    failureReason: "metadata_read_failed",
                    debugDescription: String(reflecting: error)
                )
            )
        }

        let rawMetadata: [String: Any]
        do {
            rawMetadata = try decodedJSONObject(data: metadataData)
        } catch let error as FMError {
            throw error
        } catch {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter metadata for '\(packageURL.lastPathComponent)' is not valid JSON",
                    failureReason: "invalid_metadata_json",
                    debugDescription: String(reflecting: error)
                )
            )
        }

        guard let adapterIdentifier = rawMetadata["adapterIdentifier"] as? String,
            !adapterIdentifier.isEmpty
        else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter metadata is missing a non-empty adapterIdentifier",
                    failureReason: "missing_adapter_identifier"
                )
            )
        }

        let baseModelSignature = rawMetadata["baseModelSignature"] as? String
        if let baseModelSignature,
            baseModelSignature.range(of: "^[0-9a-f]{40}$", options: .regularExpression) == nil
        {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter '\(adapterIdentifier)' has an invalid baseModelSignature",
                    failureReason: "invalid_base_model_signature"
                )
            )
        }

        if let loraRank = rawMetadata["loraRank"] as? Int, loraRank <= 0 {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter '\(adapterIdentifier)' must carry a positive loraRank",
                    failureReason: "invalid_lora_rank"
                )
            )
        }

        let creatorDefined = rawMetadata["creatorDefined"] as? [String: Any]
        let packageFormatVersion = creatorDefined?["packageFormatVersion"] as? String
        if let packageFormatVersion,
            !supportedPackageFormatVersions.contains(packageFormatVersion)
        {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter '\(adapterIdentifier)' uses unsupported package format '\(packageFormatVersion)'",
                    failureReason: "unsupported_package_version"
                )
            )
        }

        let draftModelPresent =
            (creatorDefined?["draftModelPresent"] as? Bool)
            ?? (hasDraftMil && hasDraftWeights)

        return AdapterPackageMetadata(
            adapterIdentifier: adapterIdentifier,
            baseModelSignature: baseModelSignature,
            packageFormatVersion: packageFormatVersion,
            draftModelPresent: draftModelPresent
        )
    }

    private func decodedJSONObject(data: Data) throws -> [String: Any] {
        let value = try JSONSerialization.jsonObject(with: data)
        guard let object = value as? [String: Any] else {
            throw FMError.adapterIncompatible(
                FMErrorPayload(
                    message: "Adapter metadata must decode to a JSON object",
                    failureReason: "invalid_metadata_shape"
                )
            )
        }
        return object
    }

    private func adapterLoadFailure(
        adapterID: String,
        packageURL: URL,
        error: Error
    ) -> FMError {
        if let error = error as? FMError {
            return error
        }
        if let assetError = error as? SystemLanguageModel.Adapter.AssetError {
            switch assetError {
            case .invalidAsset(let context):
                return .adapterIncompatible(
                    FMErrorPayload(
                        message: "Adapter '\(adapterID)' is not a valid Foundation Models asset",
                        failureReason: "invalid_asset",
                        debugDescription: context.debugDescription
                    )
                )
            case .invalidAdapterName(let context):
                return .adapterIncompatible(
                    FMErrorPayload(
                        message: "Adapter '\(adapterID)' is rejected by Foundation Models because the adapter name is invalid",
                        failureReason: "invalid_adapter_name",
                        debugDescription: context.debugDescription
                    )
                )
            case .compatibleAdapterNotFound(let context):
                return .adapterIncompatible(
                    FMErrorPayload(
                        message: "Adapter '\(adapterID)' is not compatible with the active Apple Foundation Models runtime",
                        failureReason: "compatible_adapter_not_found",
                        debugDescription: context.debugDescription
                    )
                )
            @unknown default:
                return .adapterIncompatible(
                    FMErrorPayload(
                        message: "Adapter '\(adapterID)' hit an unknown Foundation Models asset error",
                        failureReason: "unknown_asset_error",
                        debugDescription: String(reflecting: assetError)
                    )
                )
            }
        }
        return .adapterIncompatible(
            FMErrorPayload(
                message: "Failed to load adapter '\(adapterID)' from '\(packageURL.path)'",
                failureReason: "adapter_compile_failed",
                debugDescription: String(reflecting: error)
            )
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
                FMErrorPayload(
                    message: "Unsupported Apple FM model '\(requestedModel)'. Only '\(defaultModel)' is available."
                )
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
                FMErrorPayload(
                    message: "Conflicting temperature values were provided in both 'options.temperature' and 'temperature'"
                )
            )
        } else {
            mergedTemperature = options?.temperature ?? legacyTemperature
        }

        let mergedMaxTokens: Int?
        if let payloadMax = options?.maximumResponseTokens, let legacyMaxTokens,
            payloadMax != legacyMaxTokens
        {
            throw FMError.invalidRequest(
                FMErrorPayload(
                    message: "Conflicting maximum response token values were provided in both 'options.maximum_response_tokens' and 'max_tokens'"
                )
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
                throw FMError.invalidRequest(
                    FMErrorPayload(message: "'temperature' must be non-negative")
                )
            }
            options.temperature = temperature
        }

        if let maximumResponseTokens = payload.maximumResponseTokens {
            guard maximumResponseTokens > 0 else {
                throw FMError.invalidRequest(
                    FMErrorPayload(message: "'maximum_response_tokens' must be positive")
                )
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
                throw FMError.invalidRequest(
                    FMErrorPayload(message: "greedy sampling does not accept 'top'")
                )
            }
            if sampling.topP != nil {
                throw FMError.invalidRequest(
                    FMErrorPayload(
                        message: "greedy sampling does not accept 'probability_threshold'"
                    )
                )
            }
            if sampling.seed != nil {
                throw FMError.invalidRequest(
                    FMErrorPayload(message: "greedy sampling does not accept 'seed'")
                )
            }
            return .greedy
        case .random:
            if let topK = sampling.topK, topK <= 0 {
                throw FMError.invalidRequest(
                    FMErrorPayload(message: "'top' must be a positive integer")
                )
            }
            if sampling.topK != nil && sampling.topP != nil {
                throw FMError.invalidRequest(
                    FMErrorPayload(
                        message: "Cannot specify both 'top' and 'probability_threshold'. Choose one sampling constraint."
                    )
                )
            }
            if let probabilityThreshold = sampling.topP,
                probabilityThreshold < 0 || probabilityThreshold > 1
            {
                throw FMError.invalidRequest(
                    FMErrorPayload(
                        message: "'probability_threshold' must be between 0.0 and 1.0"
                    )
                )
            }
            if let topK = sampling.topK {
                return .random(top: topK, seed: sampling.seed)
            }
            let probabilityThreshold = sampling.topP ?? 1.0
            return .random(probabilityThreshold: probabilityThreshold, seed: sampling.seed)
        }
    }

    nonisolated private func toolCallFailure(from error: LanguageModelSession.ToolCallError) -> FMError {
        if let remoteError = error.underlyingError as? RemoteToolCallError {
            return FMError.toolCallFailed(
                FMErrorPayload(
                    message: "Tool '\(remoteError.toolName)' failed: \(remoteError.underlyingError)",
                    debugDescription: String(reflecting: error),
                    toolName: remoteError.toolName,
                    underlyingError: remoteError.underlyingError
                )
            )
        }

        return FMError.toolCallFailed(
            FMErrorPayload(
                message: "Tool call failed: \(error.localizedDescription)",
                debugDescription: String(reflecting: error),
                toolName: "unknown_tool",
                underlyingError: error.underlyingError.localizedDescription
            )
        )
    }

    nonisolated private func generationFailure(
        from error: LanguageModelSession.GenerationError,
        fallbackPrefix: String
    ) async -> FMError {
        let payload = FMErrorPayload(
            message: error.errorDescription ?? "\(fallbackPrefix): \(error.localizedDescription)",
            failureReason: error.failureReason,
            recoverySuggestion: error.recoverySuggestion,
            debugDescription: String(reflecting: error)
        )

        switch error {
        case .exceededContextWindowSize:
            return .exceededContextWindowSize(payload)
        case .assetsUnavailable:
            return .assetsUnavailable(payload)
        case .guardrailViolation:
            return .guardrailViolation(payload)
        case .unsupportedGuide:
            return .unsupportedGuide(payload)
        case .unsupportedLanguageOrLocale:
            return .unsupportedLanguageOrLocale(payload)
        case .decodingFailure:
            return .decodingFailure(payload)
        case .rateLimited:
            return .rateLimited(payload)
        case .concurrentRequests:
            return .concurrentRequests(payload)
        case .refusal(let refusal, _):
            let explanationText = try? await refusal.explanation.content
            return .refusal(
                FMErrorPayload(
                    message: payload.message,
                    failureReason: payload.failureReason,
                    recoverySuggestion: payload.recoverySuggestion,
                    debugDescription: payload.debugDescription,
                    refusalExplanation: explanationText
                )
            )
        @unknown default:
            return .serverError(payload)
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
