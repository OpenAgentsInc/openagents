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

        // Return async stream of SSE-formatted strings
        return AsyncStream { continuation in
            Task {
                do {
                    // Check if this is guided generation
                    if let responseFormat = request.responseFormat,
                       responseFormat.type == "json_schema",
                       let schemaType = responseFormat.schemaType {
                        // Streaming with guided generation
                        try await streamGuidedGeneration(
                            session: session,
                            prompt: prompt,
                            schemaType: schemaType,
                            completionId: completionId,
                            created: created,
                            model: request.model,
                            continuation: continuation
                        )
                    } else {
                        // Regular text streaming
                        try await streamTextGeneration(
                            session: session,
                            prompt: prompt,
                            completionId: completionId,
                            created: created,
                            model: request.model,
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

        // Stream response
        let stream = session.streamResponse(to: prompt)

        for try await snapshot in stream {
            // Extract text from snapshot
            let content = snapshot.content as? String ?? ""

            let chunk = StreamChunk(
                id: completionId,
                created: created,
                model: model,
                choices: [StreamChoice(
                    index: 0,
                    delta: Delta(role: nil, content: content),
                    finishReason: nil
                )]
            )

            if let data = try? JSONEncoder().encode(chunk) {
                continuation.yield(SSEWriter.writeEvent(data: data, id: completionId))
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

        var accumulatedContent = ""
        for try await snapshot in stream {
            // Convert GeneratedContent to JSON string
            if let jsonData = try? JSONEncoder().encode(snapshot.rawContent),
               let jsonString = String(data: jsonData, encoding: .utf8) {
                accumulatedContent = jsonString

                let chunk = StreamChunk(
                    id: completionId,
                    created: created,
                    model: model,
                    choices: [StreamChoice(
                        index: 0,
                        delta: Delta(role: nil, content: jsonString),
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
