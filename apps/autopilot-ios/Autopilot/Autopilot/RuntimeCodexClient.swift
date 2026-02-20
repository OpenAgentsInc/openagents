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

    func sendEmailCode(email: String) async throws {
        let _: AuthSendCodeResponse = try await requestJSON(
            path: "/api/auth/email",
            method: "POST",
            body: AuthSendCodeRequest(email: email),
            headers: ["X-Client": "autopilot-ios"]
        )
    }

    func verifyEmailCode(code: String) async throws -> RuntimeCodexAuthSession {
        let response: AuthVerifyResponse = try await requestJSON(
            path: "/api/auth/verify",
            method: "POST",
            body: AuthVerifyRequest(code: code),
            headers: ["X-Client": "autopilot-ios"]
        )

        let normalizedToken = (response.token ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedToken.isEmpty else {
            throw RuntimeCodexApiError(message: "auth_token_missing", code: .auth, status: 401)
        }

        return RuntimeCodexAuthSession(
            userID: response.userID?.value ?? response.user?.id?.value,
            email: response.user?.email,
            token: normalizedToken
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

    func streamWorker(workerID: String, cursor: Int, tailMS: Int = 8_000) async throws -> RuntimeCodexStreamBatch {
        let normalizedCursor = max(0, cursor)
        let normalizedTailMS = max(1_000, tailMS)
        let body = try await requestText(
            path: "/api/runtime/codex/workers/\(workerID.urlPathEncoded)/stream",
            queryItems: [
                URLQueryItem(name: "cursor", value: String(normalizedCursor)),
                URLQueryItem(name: "tail_ms", value: String(normalizedTailMS)),
            ]
        )

        let events = Self.parseSSEEvents(raw: body)
        let nextCursor = events.reduce(normalizedCursor) { partial, event in
            max(partial, event.cursorHint ?? partial)
        }

        return RuntimeCodexStreamBatch(events: events, nextCursor: nextCursor)
    }

    static func parseSSEEvents(raw: String) -> [RuntimeCodexStreamEvent] {
        var events: [RuntimeCodexStreamEvent] = []
        let jsonDecoder = JSONDecoder()
        let normalized = raw.replacingOccurrences(of: "\r\n", with: "\n")
        var currentID: Int?
        var currentEvent = "message"
        var dataLines: [String] = []
        var sawEventFields = false

        func flushCurrentEvent() {
            defer {
                currentID = nil
                currentEvent = "message"
                dataLines = []
                sawEventFields = false
            }

            guard !dataLines.isEmpty else {
                return
            }

            let dataRaw = dataLines.joined(separator: "\n")
            let payload: JSONValue

            if let data = dataRaw.data(using: .utf8),
               let decoded = try? jsonDecoder.decode(JSONValue.self, from: data) {
                payload = decoded
            } else {
                payload = .string(dataRaw)
            }

            events.append(
                RuntimeCodexStreamEvent(
                    id: currentID,
                    event: currentEvent,
                    payload: payload,
                    rawData: dataRaw
                )
            )
        }

        for rawLine in normalized.split(separator: "\n", omittingEmptySubsequences: false) {
            let line = String(rawLine)

            if line.isEmpty {
                flushCurrentEvent()
                continue
            }

            if line.hasPrefix("id:") {
                // Some upstream streams omit blank separators; treat a new `id:` as a frame boundary.
                if !dataLines.isEmpty {
                    flushCurrentEvent()
                }

                let rawID = line.dropFirst(3).trimmingCharacters(in: .whitespaces)
                currentID = Int(rawID)
                sawEventFields = true
                continue
            }

            if line.hasPrefix("event:") {
                let rawEvent = line.dropFirst(6).trimmingCharacters(in: .whitespaces)
                currentEvent = rawEvent.isEmpty ? "message" : rawEvent
                sawEventFields = true
                continue
            }

            if line.hasPrefix("data:") {
                let rawData = line.dropFirst(5).trimmingCharacters(in: .whitespaces)
                dataLines.append(rawData)
                sawEventFields = true
                continue
            }

            // If upstream folds data lines without repeating `data:`, append to previous payload line.
            if sawEventFields {
                if let last = dataLines.indices.last {
                    dataLines[last].append("\n" + line)
                } else {
                    dataLines.append(line)
                }
            }
        }

        flushCurrentEvent()
        return events
    }

    private func requestText(path: String, queryItems: [URLQueryItem], headers: [String: String] = [:]) async throws -> String {
        var request = try makeRequest(path: path, method: "GET", queryItems: queryItems, headers: headers)
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else {
                throw RuntimeCodexApiError(message: "non_http_response", code: .network, status: nil)
            }

            let text = String(data: data, encoding: .utf8) ?? ""

            if !(200..<300).contains(http.statusCode) {
                throw mapResponseError(status: http.statusCode, body: text)
            }

            return text
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
}

private struct AuthVerifyRequest: Encodable {
    let code: String
}

private struct AuthVerifyResponse: Decodable {
    let ok: Bool?
    let userID: LossyString?
    let token: String?
    let user: AuthVerifyUser?

    enum CodingKeys: String, CodingKey {
        case ok
        case userID = "userId"
        case token
        case user
    }
}

private struct AuthVerifyUser: Decodable {
    let id: LossyString?
    let email: String?
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
