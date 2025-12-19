import Foundation

// MARK: - AnyCodable Helper

/// Type-erased Codable for arbitrary JSON values
struct AnyCodable: Codable {
    let value: Any

    init(_ value: Any) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map { $0.value }
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Cannot decode value")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            throw EncodingError.invalidValue(value, EncodingError.Context(codingPath: [], debugDescription: "Cannot encode value"))
        }
    }
}

// MARK: - OpenAI-Compatible Request Types

struct ChatCompletionRequest: Codable {
    let model: String?
    let messages: [ChatMessage]
    let temperature: Double?
    let maxTokens: Int?
    let stream: Bool?
    let responseFormat: ResponseFormatRequest?
    let samplingMode: SamplingModeRequest?
    let useCase: UseCaseRequest?
    let guardrails: GuardrailsRequest?

    enum CodingKeys: String, CodingKey {
        case model
        case messages
        case temperature
        case maxTokens = "max_tokens"
        case stream
        case responseFormat = "response_format"
        case samplingMode = "sampling_mode"
        case useCase = "use_case"
        case guardrails
    }
}

/// Sampling mode configuration
struct SamplingModeRequest: Codable {
    let type: String
    let k: Int?
    let p: Double?

    /// Create greedy sampling mode
    static func greedy() -> Self {
        Self(type: "greedy", k: nil, p: nil)
    }

    /// Create top-k sampling mode
    static func topK(_ k: Int) -> Self {
        Self(type: "top_k", k: k, p: nil)
    }

    /// Create nucleus (top-p) sampling mode
    static func nucleus(_ p: Double) -> Self {
        Self(type: "nucleus", k: nil, p: p)
    }
}

/// Use case for generation
enum UseCaseRequest: String, Codable {
    case general = "general"
    case contentTagging = "content_tagging"
}

/// Guardrails configuration
enum GuardrailsRequest: String, Codable {
    case `default` = "default"
    case permissiveContentTransformations = "permissive_content_transformations"
}

/// Request for structured output format
struct ResponseFormatRequest: Codable {
    /// Type: "text", "json_object", or "json_schema"
    let type: String
    /// For pre-defined schemas like "test_generation"
    let schemaType: String?
    /// For custom JSON schemas
    let jsonSchema: JSONSchemaSpec?

    enum CodingKeys: String, CodingKey {
        case type
        case schemaType = "schema_type"
        case jsonSchema = "json_schema"
    }
}

/// JSON schema specification
struct JSONSchemaSpec: Codable {
    let name: String?
    let description: String?
    let schema: [String: AnyCodable]?
}

struct ChatMessage: Codable {
    let role: String
    let content: String
}

// MARK: - OpenAI-Compatible Response Types

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

// MARK: - Models Endpoint Types

struct ModelsResponse: Codable {
    let object: String
    let data: [ModelInfo]
}

struct ModelInfo: Codable {
    let id: String
    let object: String
    let created: Int
    let ownedBy: String

    enum CodingKeys: String, CodingKey {
        case id
        case object
        case created
        case ownedBy = "owned_by"
    }
}

// MARK: - Health Endpoint Types

struct HealthResponse: Codable {
    let status: String
    let modelAvailable: Bool
    let version: String
    let platform: String

    enum CodingKeys: String, CodingKey {
        case status
        case modelAvailable = "model_available"
        case version
        case platform
    }
}

// MARK: - Error Types

struct ErrorResponse: Codable {
    let error: ErrorDetail

    init(error: ErrorDetail) {
        self.error = error
    }

    init(error: String) {
        self.error = ErrorDetail(message: error, type: "error", code: nil)
    }
}

struct ErrorDetail: Codable {
    let message: String
    let type: String
    let code: String?
}

enum FMError: Error {
    case modelUnavailable(String)
    case requestFailed(String)
    case invalidRequest(String)
    case serverError(String)

    var errorResponse: ErrorResponse {
        switch self {
        case .modelUnavailable(let msg):
            return ErrorResponse(error: ErrorDetail(message: msg, type: "model_unavailable", code: "model_unavailable"))
        case .requestFailed(let msg):
            return ErrorResponse(error: ErrorDetail(message: msg, type: "request_failed", code: "request_failed"))
        case .invalidRequest(let msg):
            return ErrorResponse(error: ErrorDetail(message: msg, type: "invalid_request_error", code: "invalid_request"))
        case .serverError(let msg):
            return ErrorResponse(error: ErrorDetail(message: msg, type: "server_error", code: "server_error"))
        }
    }
}

enum CompletionError: Error {
    case modelUnavailable
    case invalidSchemaType
    case sessionNotFound
    case toolNotFound
}

// MARK: - HTTP Response Type

/// HTTP response for handler methods
struct HTTPResponse {
    let status: Int
    let headers: [String: String]
    let body: String

    init(status: Int, headers: [String: String], body: String) {
        self.status = status
        self.headers = headers
        self.body = body
    }
}
