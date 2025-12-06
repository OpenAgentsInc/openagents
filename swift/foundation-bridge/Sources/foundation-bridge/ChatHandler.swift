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
            switch reason {
            case .deviceNotSupported:
                return (false, "Device not supported - requires Apple Silicon Mac")
            case .appleIntelligenceNotEnabled:
                return (false, "Apple Intelligence is not enabled in System Settings")
            case .modelNotReady:
                return (false, "Model is not ready - please wait for download to complete")
            @unknown default:
                return (false, "Foundation Models unavailable: \(reason)")
            }
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

        // Create or reuse session
        if session == nil {
            session = LanguageModelSession()
        }

        guard let session = session else {
            throw FMError.serverError("Failed to create language model session")
        }

        // Generate response
        let startTime = Date()
        let response: LanguageModelSession.Response
        do {
            response = try await session.respond(to: prompt)
        } catch {
            throw FMError.requestFailed("Foundation Models request failed: \(error.localizedDescription)")
        }
        let endTime = Date()

        // Extract content
        let content = response.content

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
