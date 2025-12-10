import Foundation
import FoundationModels

/// Handler for streaming chat completions using Server-Sent Events (SSE)
struct StreamHandler {
    static func handleStreamingCompletion(request: ChatCompletionRequest) async throws -> AsyncStream<String> {
        // Check model availability
        let model = SystemLanguageModel.default
        guard model.isAvailable else {
            throw CompletionError.modelUnavailable
        }

        // Create session
        let session = LanguageModelSession(model: model)

        // Build prompt from messages
        let prompt = buildPrompt(from: request.messages)

        // Generate unique ID for this completion
        let completionId = "fm-\(UUID().uuidString)"
        let created = Int(Date().timeIntervalSince1970)

        // Extract values from request before closure (Sendable compliance)
        let modelName = request.model ?? "apple-foundation-model"
        let responseFormat = request.responseFormat
        let schemaType = responseFormat?.schemaType
        let isGuidedGeneration = responseFormat?.type == "json_schema" && schemaType != nil

        // Return async stream of SSE-formatted strings
        return AsyncStream { continuation in
            Task {
                do {
                    // Check if this is guided generation
                    if isGuidedGeneration, let schemaType = schemaType {
                        // Streaming with guided generation
                        try await streamGuidedGeneration(
                            session: session,
                            prompt: prompt,
                            schemaType: schemaType,
                            completionId: completionId,
                            created: created,
                            model: modelName,
                            continuation: continuation
                        )
                    } else {
                        // Regular text streaming
                        try await streamTextGeneration(
                            session: session,
                            prompt: prompt,
                            completionId: completionId,
                            created: created,
                            model: modelName,
                            continuation: continuation
                        )
                    }

                    // Send [DONE] message
                    continuation.yield(SSEWriter.writeDone())
                    continuation.finish()
                } catch {
                    let errorMessage = SSEWriter.writeError(error.localizedDescription)
                    continuation.yield(errorMessage)
                    continuation.finish()
                }
            }
        }
    }

    private static func streamTextGeneration(
        session: LanguageModelSession,
        prompt: String,
        completionId: String,
        created: Int,
        model: String,
        continuation: AsyncStream<String>.Continuation
    ) async throws {
        // Send initial chunk with role
        let initialChunk = StreamChunk(
            id: completionId,
            created: created,
            model: model,
            choices: [StreamChoice(
                index: 0,
                delta: Delta(role: "assistant", content: ""),
                finishReason: nil
            )]
        )
        if let data = try? JSONEncoder().encode(initialChunk) {
            continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
        }

        // Stream response - note: snapshots contain ACCUMULATED content, not deltas
        let stream = session.streamResponse(to: prompt)
        var previousContent = ""

        for try await snapshot in stream {
            // snapshot.content is the full accumulated text so far
            let currentContent = snapshot.content

            // Calculate the delta (new content since last snapshot)
            let delta: String
            if currentContent.hasPrefix(previousContent) {
                delta = String(currentContent.dropFirst(previousContent.count))
            } else {
                // In case of any edge case, just use current content
                delta = currentContent
            }

            previousContent = currentContent

            // Only send non-empty deltas
            if !delta.isEmpty {
                let chunk = StreamChunk(
                    id: completionId,
                    created: created,
                    model: model,
                    choices: [StreamChoice(
                        index: 0,
                        delta: Delta(role: nil, content: delta),
                        finishReason: nil
                    )]
                )

                if let data = try? JSONEncoder().encode(chunk) {
                    continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
                }
            }
        }

        // Send final chunk with finish_reason
        let finalChunk = StreamChunk(
            id: completionId,
            created: created,
            model: model,
            choices: [StreamChoice(
                index: 0,
                delta: Delta(role: nil, content: ""),
                finishReason: "stop"
            )]
        )
        if let data = try? JSONEncoder().encode(finalChunk) {
            continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
        }
    }

    private static func streamGuidedGeneration(
        session: LanguageModelSession,
        prompt: String,
        schemaType: String,
        completionId: String,
        created: Int,
        model: String,
        continuation: AsyncStream<String>.Continuation
    ) async throws {
        // Get schema for type
        guard let schema = getSchema(for: schemaType) else {
            throw CompletionError.invalidSchemaType
        }

        // Send initial chunk
        let initialChunk = StreamChunk(
            id: completionId,
            created: created,
            model: model,
            choices: [StreamChoice(
                index: 0,
                delta: Delta(role: "assistant", content: ""),
                finishReason: nil
            )]
        )
        if let data = try? JSONEncoder().encode(initialChunk) {
            continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
        }

        // Stream guided response
        let stream = session.streamResponse(to: prompt, schema: schema)

        var previousJson = ""
        for try await snapshot in stream {
            // Use GeneratedContent's built-in jsonString property
            let currentJson = snapshot.rawContent.jsonString

            // Calculate delta (new content since last snapshot)
            let delta: String
            if currentJson.hasPrefix(previousJson) {
                delta = String(currentJson.dropFirst(previousJson.count))
            } else {
                delta = currentJson
            }

            previousJson = currentJson

            if !delta.isEmpty {
                let chunk = StreamChunk(
                    id: completionId,
                    created: created,
                    model: model,
                    choices: [StreamChoice(
                        index: 0,
                        delta: Delta(role: nil, content: delta),
                        finishReason: nil
                    )]
                )

                if let data = try? JSONEncoder().encode(chunk) {
                    continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
                }
            }
        }

        // Send final chunk
        let finalChunk = StreamChunk(
            id: completionId,
            created: created,
            model: model,
            choices: [StreamChoice(
                index: 0,
                delta: Delta(role: nil, content: ""),
                finishReason: "stop"
            )]
        )
        if let data = try? JSONEncoder().encode(finalChunk) {
            continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
        }
    }

    private static func buildPrompt(from messages: [ChatMessage]) -> String {
        messages.map { message in
            if message.role == "system" {
                return "System: \(message.content)"
            } else if message.role == "user" {
                return "User: \(message.content)"
            } else {
                return "Assistant: \(message.content)"
            }
        }.joined(separator: "\n\n")
    }

    private static func getSchema(for type: String) -> GenerationSchema? {
        switch type {
        case "test_generation":
            return TestGenerationResult.generationSchema
        case "environment_aware_test_generation":
            return EnvironmentAwareTestResult.generationSchema
        case "tool_call":
            return ToolCallRequest.generationSchema
        default:
            return nil
        }
    }
}
