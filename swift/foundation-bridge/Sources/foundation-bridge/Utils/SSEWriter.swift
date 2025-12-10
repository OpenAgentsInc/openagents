import Foundation

/// Server-Sent Events (SSE) writer for streaming responses
struct SSEWriter {
    /// Write an SSE event to a string builder
    /// - Parameters:
    ///   - data: JSON data to send
    ///   - id: Optional event ID
    /// - Returns: Formatted SSE string
    static func writeEvent(data: Data, id: String? = nil) -> String {
        var result = ""

        if let id = id {
            result += "id: \(id)\n"
        }

        if let jsonString = String(data: data, encoding: .utf8) {
            // SSE format: each line of data prefixed with "data: "
            let lines = jsonString.components(separatedBy: "\n")
            for line in lines {
                result += "data: \(line)\n"
            }
        }

        result += "\n"  // Empty line terminates event
        return result
    }

    /// Write the [DONE] message that signals end of stream
    static func writeDone() -> String {
        return "data: [DONE]\n\n"
    }

    /// Write an error event
    static func writeError(_ message: String) -> String {
        let errorData = ["error": ["message": message]]
        if let data = try? JSONEncoder().encode(errorData) {
            return writeEvent(data: data)
        }
        return "data: {\"error\":{\"message\":\"Unknown error\"}}\n\n"
    }
}

/// OpenAI-compatible streaming chunk
struct StreamChunk: Codable {
    let id: String
    let object: String = "chat.completion.chunk"
    let created: Int
    let model: String
    let choices: [StreamChoice]
}

struct StreamChoice: Codable {
    let index: Int
    let delta: Delta
    let finishReason: String?

    enum CodingKeys: String, CodingKey {
        case index
        case delta
        case finishReason = "finish_reason"
    }
}

struct Delta: Codable {
    let role: String?
    let content: String?
}
