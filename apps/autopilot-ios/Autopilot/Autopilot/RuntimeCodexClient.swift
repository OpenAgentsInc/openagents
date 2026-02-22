import Foundation

final class RuntimeCodexClient {
    private let baseURL: URL
    private let authToken: String?
    private let session: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder

    init(baseURL: URL, authToken: String? = nil, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.authToken = authToken?.trimmingCharacters(in: .whitespacesAndNewlines)
        self.session = session

        let decoder = JSONDecoder()
        self.decoder = decoder

        let encoder = JSONEncoder()
        self.encoder = encoder
    }

    static func clearSessionCookies(baseURL: URL) {
        guard let host = baseURL.host?.trimmingCharacters(in: CharacterSet(charactersIn: ".")).lowercased(),
              !host.isEmpty else {
            return
        }

        let storage = HTTPCookieStorage.shared
        for cookie in storage.cookies ?? [] {
            let domain = cookie.domain.trimmingCharacters(in: CharacterSet(charactersIn: ".")).lowercased()
            guard !domain.isEmpty else {
                continue
            }

            if host == domain || host.hasSuffix(".\(domain)") {
                storage.deleteCookie(cookie)
            }
        }

        URLCache.shared.removeAllCachedResponses()
    }

    func sendEmailCode(email: String) async throws -> RuntimeCodexAuthChallenge {
        let response: AuthSendCodeResponse = try await requestJSON(
            path: "/api/auth/email",
            method: "POST",
            body: AuthSendCodeRequest(email: email),
            headers: ["X-Client": "autopilot-ios"]
        )

        let challengeID = (response.challengeID ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !challengeID.isEmpty else {
            throw RuntimeCodexApiError(message: "auth_challenge_missing", code: .auth, status: 401)
        }

        let normalizedEmail = (response.email ?? email).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedEmail.isEmpty else {
            throw RuntimeCodexApiError(message: "auth_email_missing", code: .auth, status: 401)
        }

        return RuntimeCodexAuthChallenge(challengeID: challengeID, email: normalizedEmail)
    }

    func verifyEmailCode(code: String, challengeID: String?) async throws -> RuntimeCodexAuthSession {
        let response: AuthVerifyResponse = try await requestJSON(
            path: "/api/auth/verify",
            method: "POST",
            body: AuthVerifyRequest(code: code, challengeID: challengeID),
            headers: ["X-Client": "autopilot-ios"]
        )

        let normalizedToken = (response.token ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedToken.isEmpty else {
            throw RuntimeCodexApiError(message: "auth_token_missing", code: .auth, status: 401)
        }

        return RuntimeCodexAuthSession(
            userID: response.userID?.value ?? response.user?.id?.value,
            email: response.user?.email,
            tokenType: response.tokenType,
            token: normalizedToken,
            refreshToken: response.refreshToken?.trimmingCharacters(in: .whitespacesAndNewlines),
            sessionID: response.sessionID,
            accessExpiresAt: nil,
            refreshExpiresAt: nil
        )
    }

    func refreshSession(refreshToken: String) async throws -> RuntimeCodexAuthSession {
        let response: RefreshSessionResponse = try await requestJSON(
            path: "/api/auth/refresh",
            method: "POST",
            body: RefreshSessionRequest(refreshToken: refreshToken, rotateRefreshToken: true)
        )

        let normalizedToken = response.token.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedToken.isEmpty else {
            throw RuntimeCodexApiError(message: "auth_token_missing", code: .auth, status: 401)
        }

        let normalizedRefreshToken = response.refreshToken.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedRefreshToken.isEmpty else {
            throw RuntimeCodexApiError(message: "refresh_token_missing", code: .auth, status: 401)
        }

        return RuntimeCodexAuthSession(
            userID: nil,
            email: nil,
            tokenType: response.tokenType,
            token: normalizedToken,
            refreshToken: normalizedRefreshToken,
            sessionID: response.sessionID,
            accessExpiresAt: response.accessExpiresAt,
            refreshExpiresAt: response.refreshExpiresAt
        )
    }

    func currentSession() async throws -> RuntimeCodexSessionSnapshot {
        let response: SessionResponse = try await requestJSON(
            path: "/api/auth/session",
            method: "GET",
            body: Optional<EmptyRequest>.none
        )

        return RuntimeCodexSessionSnapshot(
            sessionID: response.data.session.sessionID,
            userID: response.data.session.userID,
            deviceID: response.data.session.deviceID,
            status: response.data.session.status,
            reauthRequired: response.data.session.reauthRequired,
            activeOrgID: response.data.session.activeOrgID
        )
    }

    func logoutSession() async throws {
        let _: AuthLogoutResponse = try await requestJSON(
            path: "/api/auth/logout",
            method: "POST",
            body: Optional<EmptyRequest>.none
        )
    }

    func listWorkers(status: String? = nil, limit: Int = 100) async throws -> [RuntimeCodexWorkerSummary] {
        let normalizedLimit = max(1, min(limit, 200))
        var queryItems = [URLQueryItem(name: "limit", value: String(normalizedLimit))]
        if let status, !status.isEmpty {
            queryItems.append(URLQueryItem(name: "status", value: status))
        }

        let response: DataResponse<[RuntimeCodexWorkerSummary]> = try await requestJSON(
            path: "/api/runtime/codex/workers",
            method: "GET",
            queryItems: queryItems,
            body: Optional<EmptyRequest>.none
        )

        return response.data ?? []
    }

    func workerSnapshot(workerID: String) async throws -> RuntimeCodexWorkerSnapshot {
        let response: DataResponse<RuntimeCodexWorkerSnapshot> = try await requestJSON(
            path: "/api/runtime/codex/workers/\(workerID.urlPathEncoded)",
            method: "GET",
            body: Optional<EmptyRequest>.none
        )

        guard let data = response.data else {
            throw RuntimeCodexApiError(message: "worker_snapshot_missing", code: .unknown, status: nil)
        }

        return data
    }

    func ingestWorkerEvent(workerID: String, eventType: String, payload: [String: JSONValue]) async throws {
        let requestBody = WorkerEventRequest(event: WorkerEventEnvelope(eventType: eventType, payload: payload))

        let _: DataResponse<JSONValue> = try await requestJSON(
            path: "/api/runtime/codex/workers/\(workerID.urlPathEncoded)/events",
            method: "POST",
            body: requestBody
        )
    }

    func requestWorkerAction(
        workerID: String,
        request: RuntimeCodexWorkerActionRequest
    ) async throws -> RuntimeCodexWorkerActionResult {
        let response: DataResponse<RuntimeCodexWorkerActionResult> = try await requestJSON(
            path: "/api/runtime/codex/workers/\(workerID.urlPathEncoded)/requests",
            method: "POST",
            body: WorkerActionRequestEnvelope(request: request)
        )

        guard let data = response.data else {
            throw RuntimeCodexApiError(message: "worker_request_response_missing", code: .unknown, status: nil)
        }

        return data
    }

    func stopWorker(workerID: String, reason: String? = nil) async throws -> RuntimeCodexWorkerStopResult {
        let normalizedReason = reason?.trimmingCharacters(in: .whitespacesAndNewlines)
        let response: DataResponse<RuntimeCodexWorkerStopResult> = try await requestJSON(
            path: "/api/runtime/codex/workers/\(workerID.urlPathEncoded)/stop",
            method: "POST",
            body: WorkerStopRequest(reason: normalizedReason?.isEmpty == true ? nil : normalizedReason)
        )

        guard let data = response.data else {
            throw RuntimeCodexApiError(message: "worker_stop_response_missing", code: .unknown, status: nil)
        }

        return data
    }

    func mintSyncToken(scopes: [String] = ["runtime.codex_worker_summaries"]) async throws -> RuntimeCodexSyncToken {
        let normalizedScopes = Array(
            Set(
                scopes
                    .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                    .filter { !$0.isEmpty }
            )
        ).sorted()

        let response: DataResponse<RuntimeCodexSyncToken> = try await requestJSON(
            path: "/api/sync/token",
            method: "POST",
            body: SyncTokenRequest(scopes: normalizedScopes),
            headers: ["X-Client": "autopilot-ios"]
        )

        guard let data = response.data else {
            throw RuntimeCodexApiError(message: "sync_token_missing", code: .unknown, status: nil)
        }

        let token = data.token.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !token.isEmpty else {
            throw RuntimeCodexApiError(message: "sync_token_missing", code: .unknown, status: nil)
        }

        return data
    }

    func syncWebSocketURL(token: String) throws -> URL {
        guard !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw RuntimeCodexApiError(message: "sync_token_missing", code: .invalid, status: nil)
        }

        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw RuntimeCodexApiError(message: "invalid_base_url", code: .invalid, status: nil)
        }

        if components.scheme == "https" {
            components.scheme = "wss"
        } else {
            components.scheme = "ws"
        }
        components.path = "/sync/socket/websocket"
        components.queryItems = [
            URLQueryItem(name: "token", value: token),
            URLQueryItem(name: "vsn", value: "2.0.0"),
        ]

        guard let url = components.url else {
            throw RuntimeCodexApiError(message: "invalid_sync_websocket_url", code: .invalid, status: nil)
        }

        return url
    }

    private func requestJSON<Response: Decodable, Body: Encodable>(
        path: String,
        method: String,
        queryItems: [URLQueryItem] = [],
        body: Body?,
        headers: [String: String] = [:]
    ) async throws -> Response {
        var request = try makeRequest(path: path, method: method, queryItems: queryItems, headers: headers)
        request.setValue("application/json", forHTTPHeaderField: "Accept")

        if let body {
            request.httpBody = try encoder.encode(body)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else {
                throw RuntimeCodexApiError(message: "non_http_response", code: .network, status: nil)
            }

            let bodyText = String(data: data, encoding: .utf8) ?? ""

            if !(200..<300).contains(http.statusCode) {
                throw mapResponseError(status: http.statusCode, body: bodyText)
            }

            if data.isEmpty {
                throw RuntimeCodexApiError(message: "empty_response", code: .unknown, status: http.statusCode)
            }

            let contentType = (http.value(forHTTPHeaderField: "content-type") ?? "").lowercased()
            if !contentType.contains("application/json") {
                let preview = bodyText
                    .replacingOccurrences(of: "\n", with: " ")
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                let limitedPreview = String(preview.prefix(160))
                let message = limitedPreview.isEmpty
                    ? "unexpected_non_json_response_\(http.statusCode)"
                    : "unexpected_non_json_response_\(http.statusCode): \(limitedPreview)"
                throw RuntimeCodexApiError(message: message, code: .unknown, status: http.statusCode)
            }

            do {
                return try decoder.decode(Response.self, from: data)
            } catch {
                throw RuntimeCodexApiError(
                    message: "decode_error: \(error.localizedDescription)",
                    code: .unknown,
                    status: http.statusCode
                )
            }
        } catch let error as RuntimeCodexApiError {
            throw error
        } catch {
            throw RuntimeCodexApiError(
                message: "network_error: \(error.localizedDescription)",
                code: .network,
                status: nil
            )
        }
    }

    private func mapResponseError(status: Int, body: String) -> RuntimeCodexApiError {
        let message = extractErrorMessage(from: body) ?? "request_failed_\(status)"

        switch status {
        case 401:
            return RuntimeCodexApiError(message: message, code: .auth, status: status)
        case 403:
            return RuntimeCodexApiError(message: message, code: .forbidden, status: status)
        case 409:
            return RuntimeCodexApiError(message: message, code: .conflict, status: status)
        case 400, 422:
            return RuntimeCodexApiError(message: message, code: .invalid, status: status)
        default:
            return RuntimeCodexApiError(message: message, code: .unknown, status: status)
        }
    }

    private func extractErrorMessage(from body: String) -> String? {
        guard let data = body.data(using: .utf8),
              let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }

        if let error = value["error"] as? [String: Any],
           let message = error["message"] as? String,
           !message.isEmpty {
            return message
        }

        if let message = value["message"] as? String,
           !message.isEmpty {
            return message
        }

        if let errors = value["errors"] as? [String: Any] {
            for raw in errors.values {
                if let messages = raw as? [String],
                   let first = messages.first,
                   !first.isEmpty {
                    return first
                }

                if let single = raw as? String,
                   !single.isEmpty {
                    return single
                }
            }
        }

        return nil
    }

    private func makeRequest(
        path: String,
        method: String,
        queryItems: [URLQueryItem],
        headers: [String: String]
    ) throws -> URLRequest {
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw RuntimeCodexApiError(message: "invalid_base_url", code: .invalid, status: nil)
        }

        let normalizedPath = path.hasPrefix("/") ? path : "/\(path)"
        components.path = normalizedPath
        components.queryItems = queryItems.isEmpty ? nil : queryItems

        guard let url = components.url else {
            throw RuntimeCodexApiError(message: "invalid_request_url", code: .invalid, status: nil)
        }

        var request = URLRequest(url: url)
        request.httpMethod = method
        if let authToken, !authToken.isEmpty {
            request.setValue("Bearer \(authToken)", forHTTPHeaderField: "Authorization")
        }
        request.setValue(Self.makeRequestID(), forHTTPHeaderField: "x-request-id")

        for (key, value) in headers {
            request.setValue(value, forHTTPHeaderField: key)
        }

        return request
    }

    private static func makeRequestID() -> String {
        "iosreq-\(UUID().uuidString.lowercased())"
    }
}

private struct DataResponse<T: Decodable>: Decodable {
    let data: T?
}

private struct AuthSendCodeRequest: Encodable {
    let email: String
}

private struct AuthSendCodeResponse: Decodable {
    let ok: Bool?
    let status: String?
    let email: String?
    let challengeID: String?

    enum CodingKeys: String, CodingKey {
        case ok
        case status
        case email
        case challengeID = "challengeId"
    }
}

private struct AuthVerifyRequest: Encodable {
    let code: String
    let challengeID: String?

    enum CodingKeys: String, CodingKey {
        case code
        case challengeID = "challenge_id"
    }
}

private struct AuthVerifyResponse: Decodable {
    let ok: Bool?
    let userID: LossyString?
    let tokenType: String?
    let token: String?
    let refreshToken: String?
    let sessionID: String?
    let user: AuthVerifyUser?

    enum CodingKeys: String, CodingKey {
        case ok
        case userID = "userId"
        case tokenType = "tokenType"
        case token
        case refreshToken = "refreshToken"
        case sessionID = "sessionId"
        case user
    }
}

private struct AuthVerifyUser: Decodable {
    let id: LossyString?
    let email: String?
}

private struct RefreshSessionRequest: Encodable {
    let refreshToken: String
    let rotateRefreshToken: Bool

    enum CodingKeys: String, CodingKey {
        case refreshToken = "refresh_token"
        case rotateRefreshToken = "rotate_refresh_token"
    }
}

private struct RefreshSessionResponse: Decodable {
    let tokenType: String?
    let token: String
    let refreshToken: String
    let sessionID: String?
    let accessExpiresAt: String?
    let refreshExpiresAt: String?

    enum CodingKeys: String, CodingKey {
        case tokenType = "tokenType"
        case token
        case refreshToken = "refreshToken"
        case sessionID = "sessionId"
        case accessExpiresAt = "accessExpiresAt"
        case refreshExpiresAt = "refreshExpiresAt"
    }
}

private struct SessionResponse: Decodable {
    let data: SessionPayload
}

private struct SessionPayload: Decodable {
    let session: SessionData
}

private struct SessionData: Decodable {
    let sessionID: String
    let userID: String
    let deviceID: String
    let status: String
    let reauthRequired: Bool
    let activeOrgID: String?

    enum CodingKeys: String, CodingKey {
        case sessionID = "sessionId"
        case userID = "userId"
        case deviceID = "deviceId"
        case status
        case reauthRequired = "reauthRequired"
        case activeOrgID = "activeOrgId"
    }
}

private struct AuthLogoutResponse: Decodable {
    let ok: Bool?
    let status: String?
}

private struct SyncTokenRequest: Encodable {
    let scopes: [String]
}

private struct LossyString: Decodable {
    let value: String

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let stringValue = try? container.decode(String.self) {
            value = stringValue
            return
        }

        if let intValue = try? container.decode(Int.self) {
            value = String(intValue)
            return
        }

        if let doubleValue = try? container.decode(Double.self) {
            value = String(Int(doubleValue))
            return
        }

        throw DecodingError.typeMismatch(
            String.self,
            DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Value is not string-like")
        )
    }
}

private struct WorkerEventRequest: Encodable {
    let event: WorkerEventEnvelope
}

private struct WorkerActionRequestEnvelope: Encodable {
    let request: RuntimeCodexWorkerActionRequest
}

private struct WorkerStopRequest: Encodable {
    let reason: String?
}

private struct WorkerEventEnvelope: Encodable {
    let eventType: String
    let payload: [String: JSONValue]

    enum CodingKeys: String, CodingKey {
        case eventType = "event_type"
        case payload
    }
}

private struct EmptyRequest: Encodable {}

private extension String {
    var urlPathEncoded: String {
        addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? self
    }
}
