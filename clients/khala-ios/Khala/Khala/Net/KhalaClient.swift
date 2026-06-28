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
    static let operatorFleetStatusURL = URL(string: "https://openagents.com/api/operator/fleet/status")!
    static let model = "openagents/khala"

    enum KhalaError: Error, LocalizedError {
        case missingKey
        case unauthorized // HTTP 401 / 403 — key invalid, revoked, or unrecognized
        case quotaExceeded // HTTP 402
        case invalidCodingRequest(String)
        case http(Int, String)
        case decoding
        case transport(Error)

        var errorDescription: String? {
            switch self {
            case .missingKey:
                return "No API key. Mint or paste a Khala key in Settings."
            case .unauthorized:
                return "Key rejected. Mint or paste a valid Khala key in Settings."
            case .quotaExceeded:
                return "Free quota reached. Add credits or wait for the UTC reset."
            case .invalidCodingRequest(let reason):
                return reason
            case .http(let code, _):
                return "Khala API error (\(code)). Try again."
            case .decoding:
                return "Could not read the Khala response."
            case .transport(let err):
                return "Network error: \(err.localizedDescription)"
            }
        }

        var recoveryTitle: String {
            switch self {
            case .quotaExceeded:
                return "Free quota reached"
            case .http(let code, _) where code >= 500:
                return "Temporary Khala error"
            case .transport:
                return "Connection interrupted"
            case .missingKey:
                return "Missing API key"
            case .unauthorized:
                return "Key rejected"
            case .invalidCodingRequest:
                return "Check the request"
            case .http:
                return "Khala API error"
            case .decoding:
                return "Unexpected response"
            }
        }

        var recoveryMessage: String {
            switch self {
            case .quotaExceeded:
                return "Your free quota is out for now. Add credits or wait for the UTC reset before sending again."
            case .http(let code, _) where code >= 500:
                return "The server returned \(code). This is usually temporary, so the same message can be retried."
            case .transport(let err):
                return "The request did not reach Khala cleanly: \(err.localizedDescription). Check the connection and retry."
            case .missingKey:
                return "Mint or paste a Khala key in Settings, then send the message again."
            case .unauthorized:
                return "Khala did not accept this API key. Open Settings to mint a free key or paste a valid one, then send again."
            case .invalidCodingRequest(let reason):
                return reason
            case .http(let code, _):
                return "Khala returned HTTP \(code). Update the request or try again after checking Settings."
            case .decoding:
                return "Khala responded, but the app could not read the response body."
            }
        }

        var isRetryable: Bool {
            switch self {
            case .http(let code, _) where code >= 500:
                return true
            case .transport:
                return true
            default:
                // .unauthorized is NOT retryable: the same key will fail again;
                // the user must fix it in Settings first.
                return false
            }
        }

        /// True when the right next step is to open Settings and fix the key.
        var requiresKeyAttention: Bool {
            switch self {
            case .missingKey, .unauthorized: return true
            default: return false
            }
        }
    }

    struct CodingDelegationResult {
        let assignmentRef: String?
        let durableRequestId: String?
        let durableStreamURL: String?
        let nextOffset: String?
        let streamClosed: Bool
        let streamUpToDate: Bool
        let text: String

        var displayText: String {
            var lines: [String] = []
            if let assignmentRef, !assignmentRef.isEmpty {
                lines.append("Assignment: \(assignmentRef)")
            } else {
                lines.append("Assignment: pending")
            }
            if let durableRequestId, !durableRequestId.isEmpty {
                lines.append("Durable request: \(durableRequestId)")
            }
            if let durableStreamURL, !durableStreamURL.isEmpty {
                lines.append("Stream: \(durableStreamURL)")
            }
            if let nextOffset, !nextOffset.isEmpty {
                lines.append("Offset: \(nextOffset)")
            }
            lines.append(streamClosed ? "Stream closed: yes" : "Stream closed: no")
            if streamUpToDate {
                lines.append("Status: up to date")
            }
            if !text.isEmpty {
                lines.append("")
                lines.append(text)
            }
            return lines.joined(separator: "\n")
        }
    }

    // MARK: - Free key minting

    /// Mint a free `oa_agent_…` key. Returns the token. The caller is
    /// responsible for showing the free-tier data-sharing disclosure to the
    /// user before/at first use (see the spec, Section 5).
    static func mintFreeKey(session: URLSession = .shared) async throws -> String {
        var request = URLRequest(url: freeKeyURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        do {
            let (data, response) = try await session.data(for: request)
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

    // MARK: - Fleet inspector

    static func fetchFleetInspectorStatus(
        apiKey: String,
        session: URLSession = .shared
    ) async throws -> FleetInspectorStatus {
        var request = URLRequest(url: operatorFleetStatusURL)
        request.httpMethod = "GET"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            return try FleetInspectorStatus.decode(from: data)
        } catch let err as KhalaError {
            throw err
        } catch {
            throw KhalaError.transport(error)
        }
    }

    // MARK: - Chat completion (non-stream)

    /// Send a single user message to `openagents/khala` and return the
    /// assistant text. v1 is non-streaming.
    static func complete(
        prompt: String,
        apiKey: String,
        session: URLSession = .shared
    ) async throws -> String {
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }

        let url = baseURL.appendingPathComponent("chat/completions")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(trimmedKey)", forHTTPHeaderField: "Authorization")

        let body: [String: Any] = [
            "model": model,
            "messages": [["role": "user", "content": prompt]],
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
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

    // MARK: - Codex delegation request

    /// Issue a typed `codex_agent_task` Khala request against an explicit,
    /// caller-owned Pylon ref. The server enforces ownership; the app keeps the
    /// local form bounded and public-safe before sending it.
    static func requestCodexTask(
        prompt: String,
        pylonRef: String,
        apiKey: String,
        session: URLSession = .shared
    ) async throws -> CodingDelegationResult {
        let trimmedPrompt = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedPylonRef = pylonRef.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }
        try validateCodingPrompt(trimmedPrompt)
        try validatePylonRef(trimmedPylonRef)

        let url = baseURL.appendingPathComponent("chat/completions")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(trimmedKey)", forHTTPHeaderField: "Authorization")

        let body: [String: Any] = [
            "model": model,
            "messages": [["role": "user", "content": trimmedPrompt]],
            "workflowClass": "codex_agent_task",
            "targetPylonRef": trimmedPylonRef,
            "openagents": [
                "workflowClass": "codex_agent_task",
                "coding": [
                    "targetPylonRef": trimmedPylonRef,
                ],
            ],
            "stream": true,
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }

            let durableStreamURL = http.value(forHTTPHeaderField: "openagents-durable-stream-url")
            return CodingDelegationResult(
                assignmentRef: http.value(forHTTPHeaderField: "openagents-coding-assignment-ref"),
                durableRequestId: durableRequestId(from: durableStreamURL),
                durableStreamURL: durableStreamURL,
                nextOffset: http.value(forHTTPHeaderField: "stream-next-offset"),
                streamClosed: http.value(forHTTPHeaderField: "stream-closed") == "true",
                streamUpToDate: http.value(forHTTPHeaderField: "stream-up-to-date") == "true",
                text: parseStreamText(from: data)
            )
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

    private static func parseStreamText(from data: Data) -> String {
        guard let raw = String(data: data, encoding: .utf8) else { return "" }
        return raw
            .components(separatedBy: "\n\n")
            .compactMap { frame -> String? in
                guard let delta = delta(fromSSEFrameLines: frame.components(separatedBy: .newlines)),
                      !delta.isDone
                else { return nil }
                return delta.content
            }
            .joined()
    }

    private static func durableRequestId(from value: String?) -> String? {
        guard let value, !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return nil
        }
        guard let url = URL(string: value, relativeTo: URL(string: "https://openagents.invalid")) else {
            return nil
        }
        let prefix = "/v1/chat/completions/durable/"
        guard url.path.hasPrefix(prefix) else { return nil }
        let encoded = String(url.path.dropFirst(prefix.count))
        guard !encoded.isEmpty, !encoded.contains("/") else { return nil }
        return encoded.removingPercentEncoding ?? encoded
    }

    private static func validateCodingPrompt(_ value: String) throws {
        guard value.count >= 3, value.count <= 8_000 else {
            throw KhalaError.invalidCodingRequest("Codex task prompt must be 3-8000 characters.")
        }

        let lowercased = value.lowercased()
        let blockedTerms = [
            "api key",
            "apikey",
            "authorization:",
            "bearer ",
            "mnemonic",
            "password",
            "private key",
            "secret",
            "spark1",
            "~/.codex",
        ]
        if blockedTerms.contains(where: { lowercased.contains($0) }) {
            throw KhalaError.invalidCodingRequest("Keep Codex task prompts public-safe: use public issue numbers, public paths, and public verification commands only.")
        }
    }

    private static func validatePylonRef(_ value: String) throws {
        guard value.range(
            of: #"^[a-z0-9][a-z0-9_.:-]{2,119}$"#,
            options: .regularExpression
        ) != nil else {
            throw KhalaError.invalidCodingRequest("Enter a public-safe Pylon ref.")
        }
    }
}
