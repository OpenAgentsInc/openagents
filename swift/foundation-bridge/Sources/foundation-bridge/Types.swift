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

struct ToolDefinition: Codable {
    let name: String
    let description: String?
    let argumentsSchema: JSONValue

    enum CodingKeys: String, CodingKey {
        case name
        case description
        case argumentsSchema = "arguments_schema"
    }
}

extension ToolDefinition {
    var metadata: SessionToolMetadata {
        SessionToolMetadata(name: name, description: description)
    }
}

struct ToolCallbackConfiguration: Codable {
    let url: String
    let sessionToken: String

    enum CodingKeys: String, CodingKey {
        case url
        case sessionToken = "session_token"
    }
}

struct SessionCreateRequest: Codable {
    let instructions: String?
    let model: SessionModelConfiguration?
    let tools: [ToolDefinition]
    let toolCallback: ToolCallbackConfiguration?
    let transcriptJSON: String?
    let transcript: Transcript?

    enum CodingKeys: String, CodingKey {
        case instructions
        case model
        case tools
        case toolCallback = "tool_callback"
        case transcriptJSON = "transcript_json"
        case transcript
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
    let options: GenerationOptionsPayload?
}

struct SessionRespondResponse: Codable {
    let session: SessionState
    let model: String
    let output: String
    let usage: Usage?
}

indirect enum JSONValue: Codable {
    case string(String)
    case integer(Int)
    case number(Double)
    case bool(Bool)
    case object([String: JSONValue])
    case array([JSONValue])
    case null

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let object = try? container.decode([String: JSONValue].self) {
            self = .object(object)
        } else if let array = try? container.decode([JSONValue].self) {
            self = .array(array)
        } else if let bool = try? container.decode(Bool.self) {
            self = .bool(bool)
        } else if let int = try? container.decode(Int.self) {
            self = .integer(int)
        } else if let double = try? container.decode(Double.self) {
            self = .number(double)
        } else if let string = try? container.decode(String.self) {
            self = .string(string)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "Unsupported JSON value"
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let value):
            try container.encode(value)
        case .integer(let value):
            try container.encode(value)
        case .number(let value):
            try container.encode(value)
        case .bool(let value):
            try container.encode(value)
        case .object(let value):
            try container.encode(value)
        case .array(let value):
            try container.encode(value)
        case .null:
            try container.encodeNil()
        }
    }
}

struct GeneratedContentPayload: Codable {
    let generationID: String
    let content: JSONValue
    let isComplete: Bool

    enum CodingKeys: String, CodingKey {
        case generationID = "generation_id"
        case content
        case isComplete = "is_complete"
    }

    func contentJSONString() throws -> String {
        let data = try JSONEncoder().encode(content)
        return String(data: data, encoding: .utf8) ?? "null"
    }
}

struct SessionStructuredResponseRequest: Codable {
    let prompt: String
    let schema: JSONValue
    let options: GenerationOptionsPayload?
}

struct SessionStructuredResponseResponse: Codable {
    let session: SessionState
    let model: String
    let content: GeneratedContentPayload
    let usage: Usage?
}

struct ToolCallRequestPayload: Codable {
    let sessionToken: String
    let toolName: String
    let arguments: GeneratedContentPayload

    enum CodingKeys: String, CodingKey {
        case sessionToken = "session_token"
        case toolName = "tool_name"
        case arguments
    }
}

struct ToolCallResponsePayload: Codable {
    let output: String
}

struct ToolCallErrorPayload: Codable {
    let toolName: String
    let underlyingError: String

    enum CodingKeys: String, CodingKey {
        case toolName = "tool_name"
        case underlyingError = "underlying_error"
    }
}

struct ChatCompletionRequest: Codable {
    let model: String?
    let messages: [ChatMessage]
    let temperature: Double?
    let maxTokens: Int?
    let options: GenerationOptionsPayload?
    let stream: Bool?

    enum CodingKeys: String, CodingKey {
        case model
        case messages
        case temperature
        case maxTokens = "max_tokens"
        case options
        case stream
    }
}

enum SamplingModeType: String, Codable {
    case greedy
    case random
}

struct SamplingMode: Codable {
    let mode: SamplingModeType
    let topK: Int?
    let topP: Double?
    let seed: UInt64?

    enum CodingKeys: String, CodingKey {
        case mode
        case topK = "top_k"
        case topP = "top_p"
        case seed
    }
}

struct GenerationOptionsPayload: Codable {
    let sampling: SamplingMode?
    let temperature: Double?
    let maximumResponseTokens: Int?

    enum CodingKeys: String, CodingKey {
        case sampling
        case temperature
        case maximumResponseTokens = "maximum_response_tokens"
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
    let promptTokensDetail: UsageMeasurement?
    let completionTokensDetail: UsageMeasurement?
    let totalTokensDetail: UsageMeasurement?

    enum CodingKeys: String, CodingKey {
        case promptTokens = "prompt_tokens"
        case completionTokens = "completion_tokens"
        case totalTokens = "total_tokens"
        case promptTokensDetail = "prompt_tokens_detail"
        case completionTokensDetail = "completion_tokens_detail"
        case totalTokensDetail = "total_tokens_detail"
    }
}

enum UsageTruth: String, Codable {
    case exact
    case estimated
}

struct UsageMeasurement: Codable {
    let value: Int
    let truth: UsageTruth
}

enum TextStreamEventKind: String, Codable {
    case snapshot
    case completed
}

struct TextStreamEvent: Codable {
    let kind: TextStreamEventKind
    let model: String
    let output: String
    let session: SessionState?
    let usage: Usage?
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
    let toolName: String?
    let underlyingError: String?
    let failureReason: String?
    let recoverySuggestion: String?
    let debugDescription: String?
    let refusalExplanation: String?

    enum CodingKeys: String, CodingKey {
        case message
        case type
        case code
        case toolName = "tool_name"
        case underlyingError = "underlying_error"
        case failureReason = "failure_reason"
        case recoverySuggestion = "recovery_suggestion"
        case debugDescription = "debug_description"
        case refusalExplanation = "refusal_explanation"
    }
}

struct FMErrorPayload {
    let message: String
    let failureReason: String?
    let recoverySuggestion: String?
    let debugDescription: String?
    let refusalExplanation: String?
    let toolName: String?
    let underlyingError: String?

    init(
        message: String,
        failureReason: String? = nil,
        recoverySuggestion: String? = nil,
        debugDescription: String? = nil,
        refusalExplanation: String? = nil,
        toolName: String? = nil,
        underlyingError: String? = nil
    ) {
        self.message = message
        self.failureReason = failureReason
        self.recoverySuggestion = recoverySuggestion
        self.debugDescription = debugDescription
        self.refusalExplanation = refusalExplanation
        self.toolName = toolName
        self.underlyingError = underlyingError
    }
}

enum FMError: Error {
    case exceededContextWindowSize(FMErrorPayload)
    case assetsUnavailable(FMErrorPayload)
    case guardrailViolation(FMErrorPayload)
    case unsupportedGuide(FMErrorPayload)
    case unsupportedLanguageOrLocale(FMErrorPayload)
    case decodingFailure(FMErrorPayload)
    case rateLimited(FMErrorPayload)
    case concurrentRequests(FMErrorPayload)
    case refusal(FMErrorPayload)
    case invalidGenerationSchema(FMErrorPayload)
    case toolCallFailed(FMErrorPayload)
    case invalidRequest(FMErrorPayload)
    case serverError(FMErrorPayload)

    private func response(
        type: String,
        code: String,
        payload: FMErrorPayload
    ) -> ErrorResponse {
        ErrorResponse(
            error: ErrorDetail(
                message: payload.message,
                type: type,
                code: code,
                toolName: payload.toolName,
                underlyingError: payload.underlyingError,
                failureReason: payload.failureReason,
                recoverySuggestion: payload.recoverySuggestion,
                debugDescription: payload.debugDescription,
                refusalExplanation: payload.refusalExplanation
            )
        )
    }

    var errorResponse: ErrorResponse {
        switch self {
        case .exceededContextWindowSize(let payload):
            return response(
                type: "exceeded_context_window_size",
                code: "exceeded_context_window_size",
                payload: payload
            )
        case .assetsUnavailable(let payload):
            return response(
                type: "assets_unavailable",
                code: "assets_unavailable",
                payload: payload
            )
        case .guardrailViolation(let payload):
            return response(
                type: "guardrail_violation",
                code: "guardrail_violation",
                payload: payload
            )
        case .unsupportedGuide(let payload):
            return response(
                type: "unsupported_guide",
                code: "unsupported_guide",
                payload: payload
            )
        case .unsupportedLanguageOrLocale(let payload):
            return response(
                type: "unsupported_language_or_locale",
                code: "unsupported_language_or_locale",
                payload: payload
            )
        case .decodingFailure(let payload):
            return response(
                type: "decoding_failure",
                code: "decoding_failure",
                payload: payload
            )
        case .rateLimited(let payload):
            return response(
                type: "rate_limited",
                code: "rate_limited",
                payload: payload
            )
        case .concurrentRequests(let payload):
            return response(
                type: "concurrent_requests",
                code: "concurrent_requests",
                payload: payload
            )
        case .refusal(let payload):
            return response(type: "refusal", code: "refusal", payload: payload)
        case .invalidGenerationSchema(let payload):
            return response(
                type: "invalid_generation_schema",
                code: "invalid_generation_schema",
                payload: payload
            )
        case .toolCallFailed(let payload):
            return response(
                type: "tool_call_failed",
                code: "tool_call_failed",
                payload: payload
            )
        case .invalidRequest(let payload):
            return response(type: "invalid_request", code: "invalid_request", payload: payload)
        case .serverError(let payload):
            return response(type: "server_error", code: "server_error", payload: payload)
        }
    }
}
