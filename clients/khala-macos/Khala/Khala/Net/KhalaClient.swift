import Foundation

enum KhalaClient {
    static let baseURL = URL(string: "https://openagents.com/api/v1")!
    static let model = "openagents/khala"

    enum KhalaError: Error, LocalizedError, Equatable {
        case missingKey
        case unauthorized
        case quotaExceeded
        case http(Int, String)
        case decoding
        case transport(String)

        var errorDescription: String? {
            switch self {
            case .missingKey: return "Add a Khala API key in Settings before sending."
            case .unauthorized: return "Khala rejected this API key."
            case .quotaExceeded: return "Free quota reached. Add credits or wait for the reset."
            case .http(let code, _): return "Khala API error (\(code))."
            case .decoding: return "Could not read the Khala response."
            case .transport(let message): return "Network error: \(message)"
            }
        }
    }

    static func complete(prompt: String, apiKey: String, session: URLSession = .shared) async throws -> String {
        let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        guard !apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw KhalaError.missingKey }
        var request = URLRequest(url: baseURL.appendingPathComponent("chat/completions"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["model": model, "messages": [["role": "user", "content": trimmed]]])
        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else { throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "") }
            return try parseContent(from: data)
        } catch let error as KhalaError {
            throw error
        } catch {
            throw KhalaError.transport(error.localizedDescription)
        }
    }

    static func parseContent(from data: Data) throws -> String {
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any], let choices = object["choices"] as? [[String: Any]], let first = choices.first, let message = first["message"] as? [String: Any], let content = message["content"] as? String else { throw KhalaError.decoding }
        return content
    }
}
