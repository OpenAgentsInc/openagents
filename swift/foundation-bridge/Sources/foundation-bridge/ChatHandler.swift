import Foundation
import FoundationModels

/// Handles chat completion requests using Apple Foundation Models
actor ChatHandler {
    private var session: LanguageModelSession?

    init() {}

    /// Check if Foundation Models is available on this device
    func checkAvailability() -> Bool {
        let model = SystemLanguageModel.default
        return model.availability == .available
    }

    /// Get availability status message
    func getAvailabilityStatus() -> (available: Bool, message: String) {
        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            return (true, "Foundation Models is available")
        case .unavailable(let reason):
            return (false, "Foundation Models unavailable: \(reason)")
        @unknown default:
            return (false, "Unknown availability status")
        }
    }

    /// Process a chat completion request
    func handleCompletion(request: ChatCompletionRequest) async throws -> ChatCompletionResponse {
        // Check availability
        guard checkAvailability() else {
            let (_, message) = getAvailabilityStatus()
            throw FMError.modelUnavailable(message)
        }

        // Build prompt from messages
        let prompt = buildPrompt(from: request.messages)

        // ALWAYS create a fresh session to avoid context accumulation
        // The LanguageModelSession accumulates context across respond() calls,
        // which causes "Exceeded model context window size" errors after a few calls.
        let session = LanguageModelSession()

        // Generate response - use guided generation if schema is specified
        let startTime = Date()
        let content: String

        do {
            if let responseFormat = request.responseFormat {
                content = try await handleGuidedGeneration(
                    session: session,
                    prompt: prompt,
                    responseFormat: responseFormat
                )
            } else {
                // Standard unguided generation
                let response = try await session.respond(to: prompt)
                content = response.content
            }
        } catch {
            throw FMError.requestFailed("Foundation Models request failed: \(error.localizedDescription)")
        }
        let endTime = Date()
        _ = endTime // Silence unused variable warning

        // Estimate token counts (rough approximation: ~4 chars per token)
        let promptTokens = prompt.count / 4
        let completionTokens = content.count / 4

        return ChatCompletionResponse(
            id: "fm-\(UUID().uuidString.lowercased())",
            object: "chat.completion",
            created: Int(startTime.timeIntervalSince1970),
            model: "apple-foundation-model",
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

    /// Handle guided generation with constrained output
    private func handleGuidedGeneration(
        session: LanguageModelSession,
        prompt: String,
        responseFormat: ResponseFormatRequest
    ) async throws -> String {
        // Check if using a pre-defined schema type
        if let schemaType = responseFormat.schemaType {
            switch schemaType {
            case "test_generation":
                // Use the Generable TestGenerationResult type
                let response = try await session.respond(
                    to: prompt,
                    generating: TestGenerationResult.self
                )
                // Extract the generated content from the response
                return encodeToJSON(response.content)

            case "environment_aware_test_generation":
                // Use the Generable EnvironmentAwareTestResult type
                // Prompt should include environment context for anti-cheat and parameter discovery
                let response = try await session.respond(
                    to: prompt,
                    generating: EnvironmentAwareTestResult.self
                )
                return encodeToJSON(response.content)

            default:
                throw FMError.invalidRequest("Unknown schema type: \(schemaType)")
            }
        }

        // For json_object type without schema, just request JSON output
        if responseFormat.type == "json_object" {
            // Add JSON instruction to prompt and use standard generation
            let jsonPrompt = prompt + "\n\nRespond with valid JSON only."
            let response = try await session.respond(to: jsonPrompt)
            return response.content
        }

        // For json_schema with custom schema, we'd need to build a DynamicGenerationSchema
        // This is more complex - for now, fall back to standard generation with JSON hint
        if responseFormat.type == "json_schema", let _ = responseFormat.jsonSchema {
            // TODO: Implement dynamic schema support
            // For now, add the schema description to the prompt
            let jsonPrompt = prompt + "\n\nRespond with valid JSON matching the requested schema."
            let response = try await session.respond(to: jsonPrompt)
            return response.content
        }

        // Default: standard generation
        let response = try await session.respond(to: prompt)
        return response.content
    }

    /// Encode a Codable value to JSON string
    private func encodeToJSON<T: Encodable>(_ value: T) -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        guard let data = try? encoder.encode(value),
              let json = String(data: data, encoding: .utf8) else {
            return "{\"error\": \"Failed to encode response\"}"
        }
        return json
    }

    /// Build a prompt string from chat messages
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

        // Add prompt for assistant response
        parts.append("Assistant:")

        return parts.joined(separator: "\n\n")
    }

    /// Reset the session (useful for clearing context)
    func resetSession() {
        session = nil
    }
}
