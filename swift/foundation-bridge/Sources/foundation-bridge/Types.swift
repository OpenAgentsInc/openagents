import Foundation
import FoundationModels

enum SystemLanguageModelUseCase: String, Codable {
    case general
    case contentTagging = "content_tagging"
}

enum SystemLanguageModelGuardrails: String, Codable {
    case `default`
    case permissiveContentTransformations = "permissive_content_transformations"
}

enum SystemLanguageModelUnavailableReason: String, Codable {
    case appleIntelligenceNotEnabled = "apple_intelligence_not_enabled"
    case deviceNotEligible = "device_not_eligible"
    case modelNotReady = "model_not_ready"
    case unknown
}

extension SystemLanguageModelUseCase {
    var foundationModelsValue: FoundationModels.SystemLanguageModel.UseCase {
        switch self {
        case .general:
            return .general
        case .contentTagging:
            return .contentTagging
        }
    }
}

extension SystemLanguageModelGuardrails {
    var foundationModelsValue: FoundationModels.SystemLanguageModel.Guardrails {
        switch self {
        case .default:
            return .default
        case .permissiveContentTransformations:
            return .permissiveContentTransformations
        }
    }
}

struct SessionModelConfiguration: Codable {
    let id: String
    let useCase: SystemLanguageModelUseCase
    let guardrails: SystemLanguageModelGuardrails

    enum CodingKeys: String, CodingKey {
        case id
        case useCase = "use_case"
        case guardrails
    }
}

struct SessionToolMetadata: Codable {
    let name: String
    let description: String?
}

struct SessionCreateRequest: Codable {
    let instructions: String?
    let model: SessionModelConfiguration?
    let tools: [SessionToolMetadata]
    let transcriptJSON: String?

    enum CodingKeys: String, CodingKey {
        case instructions
        case model
        case tools
        case transcriptJSON = "transcript_json"
    }
}

struct SessionState: Codable {
    let id: String
    let instructions: String?
    let model: SessionModelConfiguration
    let tools: [SessionToolMetadata]
    let isResponding: Bool
    let transcriptJSON: String?

    enum CodingKeys: String, CodingKey {
        case id
        case instructions
        case model
        case tools
        case isResponding = "is_responding"
        case transcriptJSON = "transcript_json"
    }
}

struct SessionCreateResponse: Codable {
    let session: SessionState
}

struct SessionRespondRequest: Codable {
    let prompt: String
}

struct SessionRespondResponse: Codable {
    let session: SessionState
    let model: String
    let output: String
    let usage: Usage?
}

struct ChatCompletionRequest: Codable {
    let model: String?
    let messages: [ChatMessage]
    let temperature: Double?
    let maxTokens: Int?
    let stream: Bool?

    enum CodingKeys: String, CodingKey {
        case model
        case messages
        case temperature
        case maxTokens = "max_tokens"
        case stream
    }
}

struct ChatMessage: Codable {
    let role: String
    let content: String
}

struct ChatCompletionResponse: Codable {
    let id: String
    let object: String
    let created: Int
    let model: String
    let choices: [Choice]
    let usage: Usage?
}

struct Choice: Codable {
    let index: Int
    let message: ResponseMessage
    let finishReason: String?

    enum CodingKeys: String, CodingKey {
        case index
        case message
        case finishReason = "finish_reason"
    }
}

struct ResponseMessage: Codable {
    let role: String
    let content: String?
}

struct Usage: Codable {
    let promptTokens: Int?
    let completionTokens: Int?
    let totalTokens: Int?

    enum CodingKeys: String, CodingKey {
        case promptTokens = "prompt_tokens"
        case completionTokens = "completion_tokens"
        case totalTokens = "total_tokens"
    }
}

struct ModelsResponse: Codable {
    let object: String
    let data: [ModelInfo]
}

struct ModelInfo: Codable {
    let id: String
    let object: String
    let created: Int
    let ownedBy: String
    let defaultUseCase: SystemLanguageModelUseCase
    let defaultGuardrails: SystemLanguageModelGuardrails
    let supportedUseCases: [SystemLanguageModelUseCase]
    let supportedGuardrails: [SystemLanguageModelGuardrails]
    let available: Bool?
    let unavailableReason: SystemLanguageModelUnavailableReason?
    let availabilityMessage: String?

    enum CodingKeys: String, CodingKey {
        case id
        case object
        case created
        case ownedBy = "owned_by"
        case defaultUseCase = "default_use_case"
        case defaultGuardrails = "default_guardrails"
        case supportedUseCases = "supported_use_cases"
        case supportedGuardrails = "supported_guardrails"
        case available
        case unavailableReason = "unavailable_reason"
        case availabilityMessage = "availability_message"
    }
}

struct HealthResponse: Codable {
    let status: String
    let modelAvailable: Bool
    let version: String
    let platform: String
    let availabilityMessage: String
    let unavailableReason: SystemLanguageModelUnavailableReason?
    let defaultUseCase: SystemLanguageModelUseCase
    let defaultGuardrails: SystemLanguageModelGuardrails
    let supportedUseCases: [SystemLanguageModelUseCase]
    let supportedGuardrails: [SystemLanguageModelGuardrails]
    let appleSiliconRequired: Bool
    let appleIntelligenceRequired: Bool

    enum CodingKeys: String, CodingKey {
        case status
        case modelAvailable = "model_available"
        case version
        case platform
        case availabilityMessage = "availability_message"
        case unavailableReason = "unavailable_reason"
        case defaultUseCase = "default_use_case"
        case defaultGuardrails = "default_guardrails"
        case supportedUseCases = "supported_use_cases"
        case supportedGuardrails = "supported_guardrails"
        case appleSiliconRequired = "apple_silicon_required"
        case appleIntelligenceRequired = "apple_intelligence_required"
    }
}

struct ErrorResponse: Codable {
    let error: ErrorDetail
}

struct ErrorDetail: Codable {
    let message: String
    let type: String
    let code: String?
}

enum FMError: Error {
    case modelUnavailable(String)
    case concurrentRequests(String)
    case requestFailed(String)
    case invalidRequest(String)
    case serverError(String)

    var errorResponse: ErrorResponse {
        switch self {
        case .modelUnavailable(let msg):
            return ErrorResponse(
                error: ErrorDetail(
                    message: msg,
                    type: "model_unavailable",
                    code: "model_unavailable"
                )
            )
        case .concurrentRequests(let msg):
            return ErrorResponse(
                error: ErrorDetail(
                    message: msg,
                    type: "concurrent_requests",
                    code: "concurrent_requests"
                )
            )
        case .requestFailed(let msg):
            return ErrorResponse(
                error: ErrorDetail(
                    message: msg,
                    type: "request_failed",
                    code: "request_failed"
                )
            )
        case .invalidRequest(let msg):
            return ErrorResponse(
                error: ErrorDetail(
                    message: msg,
                    type: "invalid_request_error",
                    code: "invalid_request"
                )
            )
        case .serverError(let msg):
            return ErrorResponse(
                error: ErrorDetail(
                    message: msg,
                    type: "server_error",
                    code: "server_error"
                )
            )
        }
    }
}
