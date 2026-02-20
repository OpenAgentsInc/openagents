import Foundation

enum DaemonClientError: LocalizedError {
    case invalidURL
    case missingSession
    case httpStatus(Int, String)
    case decode(String)

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            return "Daemon URL is invalid."
        case .missingSession:
            return "Daemon session is missing."
        case .httpStatus(let status, let body):
            return "Daemon request failed with status \(status): \(body)"
        case .decode(let body):
            return "Failed to decode daemon response: \(body)"
        }
    }
}

@MainActor
final class DaemonAPIClient {
    private let baseURL: URL
    private let session = URLSession.shared
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder

    private let keychainService = "com.inboxautopilot.daemon"
    private let keychainAccount = "session-token"

    private var sessionToken: String?

    init(baseURL: URL = URL(string: "http://127.0.0.1:8787")!) {
        self.baseURL = baseURL

        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        self.decoder = decoder

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        self.encoder = encoder
    }

    func bootstrapSession() async throws {
        if sessionToken == nil {
            sessionToken = try? KeychainStore.load(service: keychainService, account: keychainAccount)
        }

        if sessionToken != nil {
            return
        }

        let payload = SessionCreateRequest(clientName: "Inbox Autopilot macOS")
        let response: SessionCreateResponse = try await perform(
            path: "/session",
            method: "POST",
            body: payload,
            requiresAuth: false
        )

        sessionToken = response.sessionToken
        try KeychainStore.save(response.sessionToken, service: keychainService, account: keychainAccount)
    }

    func health() async throws -> HealthResponse {
        try await perform(path: "/health", requiresAuth: false)
    }

    func gmailStatus() async throws -> AuthStatusResponse {
        try await perform(path: "/auth/gmail/status")
    }

    func chatGPTStatus() async throws -> AuthStatusResponse {
        try await perform(path: "/auth/chatgpt/status")
    }

    func gmailAuthURL(redirectURI: String, codeChallenge: String? = nil) async throws -> GmailAuthURLResponse {
        var query = [URLQueryItem(name: "redirect_uri", value: redirectURI)]
        if let codeChallenge {
            query.append(URLQueryItem(name: "code_challenge", value: codeChallenge))
        }
        return try await perform(path: "/auth/gmail/url", queryItems: query)
    }

    func exchangeGmailCode(_ request: GmailAuthRequest) async throws {
        let _: EmptyResponse = try await perform(path: "/auth/gmail", method: "POST", body: request)
    }

    func connectChatGPT(apiKey: String) async throws {
        let req = ChatGPTAuthRequest(apiKey: apiKey)
        let _: EmptyResponse = try await perform(path: "/auth/chatgpt", method: "POST", body: req)
    }

    func startBackfill(days: Int?) async throws -> BackfillResponse {
        let req = BackfillRequest(days: days)
        return try await perform(path: "/sync/backfill", method: "POST", body: req)
    }

    func syncNow() async throws -> BackfillResponse {
        try await perform(path: "/sync/now", method: "POST", body: EmptyRequest())
    }

    func listThreads(search: String?, limit: Int = 100) async throws -> [ThreadSummary] {
        var query = [URLQueryItem(name: "limit", value: String(limit))]
        if let search, !search.isEmpty {
            query.append(URLQueryItem(name: "search", value: search))
        }
        let response: ThreadListResponse = try await perform(path: "/threads", queryItems: query)
        return response.threads
    }

    func threadDetail(id: String) async throws -> ThreadDetailResponse {
        try await perform(path: "/threads/\(id)")
    }

    func generateDraft(threadID: String) async throws -> GenerateDraftResponse {
        try await perform(path: "/threads/\(threadID)/generate-draft", method: "POST", body: EmptyRequest())
    }

    func approveAndSend(threadID: String) async throws -> ApproveSendResponse {
        try await perform(path: "/threads/\(threadID)/approve-send", method: "POST", body: EmptyRequest())
    }

    func markDraftNeedsHuman(draftID: String) async throws {
        let _: EmptyResponse = try await perform(path: "/drafts/\(draftID)/needs-human", method: "POST", body: EmptyRequest())
    }

    func pendingDrafts(limit: Int = 200) async throws -> [DraftRecord] {
        let response: DraftListResponse = try await perform(
            path: "/drafts",
            queryItems: [
                URLQueryItem(name: "status", value: "pending"),
                URLQueryItem(name: "limit", value: String(limit))
            ]
        )
        return response.drafts
    }

    func mineTemplates(limit: Int = 12) async throws -> [TemplateSuggestion] {
        let response: TemplateMineResponse = try await perform(
            path: "/templates/mine",
            queryItems: [URLQueryItem(name: "limit", value: String(limit))]
        )
        return response.suggestions
    }

    func draftEditRate(limitPerCategory: Int = 200, threshold: Double = 0.35) async throws -> DraftQualityReport {
        try await perform(
            path: "/quality/draft-edit-rate",
            queryItems: [
                URLQueryItem(name: "limit_per_category", value: String(limitPerCategory)),
                URLQueryItem(name: "threshold", value: String(threshold))
            ]
        )
    }

    func events(limit: Int = 300, threadID: String? = nil) async throws -> [EventRecord] {
        var query = [URLQueryItem(name: "limit", value: String(limit))]
        if let threadID {
            query.append(URLQueryItem(name: "thread_id", value: threadID))
        }
        let response: EventListResponse = try await perform(path: "/events", queryItems: query)
        return response.events
    }

    func audit(threadID: String) async throws -> AuditResponse {
        try await perform(path: "/threads/\(threadID)/audit")
    }

    func exportAudit(threadID: String) async throws -> ExportAuditResponse {
        try await perform(path: "/threads/\(threadID)/export-audit", method: "POST", body: EmptyRequest())
    }

    func settings() async throws -> SettingsResponse {
        try await perform(path: "/settings")
    }

    func updateSettings(_ request: UpdateSettingsRequest) async throws -> SettingsResponse {
        try await perform(path: "/settings", method: "PUT", body: request)
    }

    func deleteLocalCorpus() async throws {
        let _: EmptyResponse = try await perform(path: "/settings/delete-corpus", method: "POST", body: EmptyRequest())
    }

    func factoryReset() async throws {
        let _: EmptyResponse = try await perform(path: "/settings/factory-reset", method: "POST", body: EmptyRequest())
    }

    func eventStream() async throws -> AsyncThrowingStream<EventRecord, Error> {
        try await bootstrapSession()
        guard let token = sessionToken else {
            throw DaemonClientError.missingSession
        }

        var request = URLRequest(url: makeURL(path: "/events/stream", queryItems: []) ?? baseURL)
        request.httpMethod = "GET"
        request.setValue(token, forHTTPHeaderField: "x-session-token")
        request.setValue(UUID().uuidString.lowercased(), forHTTPHeaderField: "x-nonce")

        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw DaemonClientError.httpStatus(-1, "non-http response")
        }
        guard 200..<300 ~= http.statusCode else {
            throw DaemonClientError.httpStatus(http.statusCode, "")
        }

        return AsyncThrowingStream { continuation in
            Task {
                do {
                    for try await line in bytes.lines {
                        guard line.hasPrefix("data: ") else {
                            continue
                        }
                        let payload = String(line.dropFirst(6))
                        guard let data = payload.data(using: .utf8) else {
                            continue
                        }
                        let event = try decoder.decode(EventRecord.self, from: data)
                        continuation.yield(event)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    private func refreshSession() async throws {
        KeychainStore.delete(service: keychainService, account: keychainAccount)
        sessionToken = nil
        try await bootstrapSession()
    }

    private func makeURL(path: String, queryItems: [URLQueryItem]) -> URL? {
        var components = URLComponents(url: baseURL.appendingPathComponent(path), resolvingAgainstBaseURL: false)
        components?.queryItems = queryItems.isEmpty ? nil : queryItems
        return components?.url
    }

    private func perform<T: Decodable>(
        path: String,
        method: String = "GET",
        queryItems: [URLQueryItem] = [],
        requiresAuth: Bool = true
    ) async throws -> T {
        try await perform(path: path, method: method, queryItems: queryItems, body: Optional<EmptyRequest>.none, requiresAuth: requiresAuth)
    }

    private func perform<T: Decodable, B: Encodable>(
        path: String,
        method: String = "GET",
        queryItems: [URLQueryItem] = [],
        body: B?,
        requiresAuth: Bool = true
    ) async throws -> T {
        if requiresAuth {
            try await bootstrapSession()
        }

        guard let url = makeURL(path: path, queryItems: queryItems) else {
            throw DaemonClientError.invalidURL
        }

        var request = URLRequest(url: url)
        request.httpMethod = method

        if requiresAuth {
            guard let token = sessionToken else {
                throw DaemonClientError.missingSession
            }
            request.setValue(token, forHTTPHeaderField: "x-session-token")
            request.setValue(UUID().uuidString.lowercased(), forHTTPHeaderField: "x-nonce")
        }

        if let body {
            request.httpBody = try encoder.encode(body)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }

        do {
            let (data, response) = try await session.data(for: request)
            return try decodeResponse(data: data, response: response)
        } catch {
            if requiresAuth,
               case DaemonClientError.httpStatus(let status, _) = error,
               status == 401
            {
                try await refreshSession()
                return try await perform(path: path, method: method, queryItems: queryItems, body: body, requiresAuth: true)
            }
            throw error
        }
    }

    private func decodeResponse<T: Decodable>(data: Data, response: URLResponse) throws -> T {
        guard let http = response as? HTTPURLResponse else {
            throw DaemonClientError.httpStatus(-1, "non-http response")
        }

        guard 200..<300 ~= http.statusCode else {
            let body = String(data: data, encoding: .utf8) ?? ""
            throw DaemonClientError.httpStatus(http.statusCode, body)
        }

        if T.self == EmptyResponse.self {
            return EmptyResponse() as! T
        }

        do {
            return try decoder.decode(T.self, from: data)
        } catch {
            let body = String(data: data, encoding: .utf8) ?? ""
            throw DaemonClientError.decode(body)
        }
    }
}

private struct EmptyRequest: Encodable {}
private struct EmptyResponse: Decodable {}
