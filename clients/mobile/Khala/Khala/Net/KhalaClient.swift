import Foundation

/// HTTP client for the public Khala API.
///
/// Contract (source of truth: https://openagents.com/AGENTS.md):
/// - Base URL:  https://openagents.com/api/v1
/// - Endpoint:  POST /chat/completions   (non-stream first; SSE is a later add)
/// - Model:     openagents/khala         (one public model; no variants)
/// - Auth:      Authorization: Bearer <oa_agent_… key>
/// - Free key:  POST https://openagents.com/api/keys/free  -> credential.token
enum KhalaClient {
    static let baseURL = URL(string: "https://openagents.com/api/v1")!
    static let freeKeyURL = URL(string: "https://openagents.com/api/keys/free")!
    static let model = "openagents/khala"

    enum KhalaError: Error, LocalizedError {
        case missingKey
        case quotaExceeded // HTTP 402
        case http(Int, String)
        case decoding
        case transport(Error)

        var errorDescription: String? {
            switch self {
            case .missingKey:
                return "No API key. Mint or paste a Khala key in Settings."
            case .quotaExceeded:
                return "Free quota reached. Add credits or wait for the UTC reset."
            case .http(let code, _):
                return "Khala API error (\(code)). Try again."
            case .decoding:
                return "Could not read the Khala response."
            case .transport(let err):
                return "Network error: \(err.localizedDescription)"
            }
        }
    }

    // MARK: - Free key minting

    /// Mint a free `oa_agent_…` key. Returns the token. The caller is
    /// responsible for showing the free-tier data-sharing disclosure to the
    /// user before/at first use (see the spec, Section 5).
    static func mintFreeKey() async throws -> String {
        var request = URLRequest(url: freeKeyURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            // { "credential": { "token": "oa_agent_…" }, "dataSharing": {…} }
            guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let credential = obj["credential"] as? [String: Any],
                  let token = credential["token"] as? String,
                  !token.isEmpty
            else {
                throw KhalaError.decoding
            }
            return token
        } catch let err as KhalaError {
            throw err
        } catch {
            throw KhalaError.transport(error)
        }
    }

    // MARK: - Chat completion (non-stream)

    /// Send a single user message to `openagents/khala` and return the
    /// assistant text. v1 is non-streaming.
    static func complete(prompt: String, apiKey: String) async throws -> String {
        let url = baseURL.appendingPathComponent("chat/completions")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")

        let body: [String: Any] = [
            "model": model,
            "messages": [["role": "user", "content": prompt]],
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            return try parseContent(from: data)
        } catch let err as KhalaError {
            throw err
        } catch {
            throw KhalaError.transport(error)
        }
    }

    /// Extract `choices[0].message.content` from an OpenAI-style response.
    private static func parseContent(from data: Data) throws -> String {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let choices = obj["choices"] as? [[String: Any]],
              let first = choices.first,
              let message = first["message"] as? [String: Any],
              let content = message["content"] as? String
        else {
            throw KhalaError.decoding
        }
        return content
    }
}
