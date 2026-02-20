import Foundation

final class RuntimeCodexClient {
    private let baseURL: URL
    private let authToken: String
    private let session: URLSession
    private let decoder: JSONDecoder
    private let encoder: JSONEncoder

    init(baseURL: URL, authToken: String, session: URLSession = .shared) {
        self.baseURL = baseURL
        self.authToken = authToken
        self.session = session

        let decoder = JSONDecoder()
        self.decoder = decoder

        let encoder = JSONEncoder()
        self.encoder = encoder
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
        let chunks = raw
            .components(separatedBy: "\n\n")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        var events: [RuntimeCodexStreamEvent] = []
        let jsonDecoder = JSONDecoder()

        for chunk in chunks {
            let lines = chunk.split(separator: "\n", omittingEmptySubsequences: false)
            var id: Int?
            var event = "message"
            var dataLines: [String] = []

            for line in lines {
                if line.hasPrefix("id:") {
                    let rawID = line.dropFirst(3).trimmingCharacters(in: .whitespaces)
                    id = Int(rawID)
                } else if line.hasPrefix("event:") {
                    let rawEvent = line.dropFirst(6).trimmingCharacters(in: .whitespaces)
                    event = rawEvent.isEmpty ? "message" : rawEvent
                } else if line.hasPrefix("data:") {
                    let rawData = line.dropFirst(5).trimmingCharacters(in: .whitespaces)
                    dataLines.append(rawData)
                }
            }

            guard !dataLines.isEmpty else {
                continue
            }

            let dataRaw = dataLines.joined(separator: "\n")
            let payload: JSONValue

            if let data = dataRaw.data(using: .utf8),
               let decoded = try? jsonDecoder.decode(JSONValue.self, from: data) {
                payload = decoded
            } else {
                payload = .string(dataRaw)
            }

            events.append(RuntimeCodexStreamEvent(id: id, event: event, payload: payload, rawData: dataRaw))
        }

        return events
    }

    private func requestText(path: String, queryItems: [URLQueryItem]) async throws -> String {
        var request = try makeRequest(path: path, method: "GET", queryItems: queryItems)
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
        body: Body?
    ) async throws -> Response {
        var request = try makeRequest(path: path, method: method, queryItems: queryItems)
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
              let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let error = value["error"] as? [String: Any],
              let message = error["message"] as? String,
              !message.isEmpty else {
            return nil
        }

        return message
    }

    private func makeRequest(path: String, method: String, queryItems: [URLQueryItem]) throws -> URLRequest {
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
        request.setValue("Bearer \(authToken)", forHTTPHeaderField: "Authorization")
        request.setValue(Self.makeRequestID(), forHTTPHeaderField: "x-request-id")

        return request
    }

    private static func makeRequestID() -> String {
        "iosreq-\(UUID().uuidString.lowercased())"
    }
}

private struct DataResponse<T: Decodable>: Decodable {
    let data: T?
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
