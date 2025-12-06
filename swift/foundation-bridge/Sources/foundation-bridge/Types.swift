import Foundation

// MARK: - OpenAI-Compatible Request Types

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
