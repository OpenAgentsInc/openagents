import Foundation
import FoundationModels

actor ChatHandler {
    private var session: LanguageModelSession?

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

    func handleCompletion(request: ChatCompletionRequest) async throws -> ChatCompletionResponse {
        guard checkAvailability() else {
            let (_, _, message) = getAvailabilityStatus()
            throw FMError.modelUnavailable(message)
        }

        let prompt = buildPrompt(from: request.messages)

        if session == nil {
            session = LanguageModelSession()
        }

        guard let session else {
            throw FMError.serverError("Failed to create language model session")
        }

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
