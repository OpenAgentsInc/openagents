import Foundation

/// Owner-authenticated Artanis operator channel (#6363, epic #6359).
///
/// This is a THIN client of the shared `POST /api/operator/artanis/chat`
/// endpoint owned by the core lane. That Worker route is the single home of the
/// Artanis operator logic, persona, situational awareness, and persistent
/// owner-interaction memory; the app only posts the conversation with the
/// owner's bearer key and renders the reply. The Khala CLI talks to the exact
/// same endpoint, so both surfaces run identical operator logic.
///
/// Contract (shared with the CLI + the core Worker route):
/// - Endpoint:  POST https://openagents.com/api/operator/artanis/chat
/// - Auth:      Authorization: Bearer <owner oa_agent_… key / session>
/// - Body:      { "messages": [ { "role", "content" }, … ] }
/// - Response:  JSON by default, or SSE frames when `Accept:
///              text/event-stream` is sent. SSE frames carry the Artanis
///              operator signature/provenance ref and OpenAI-style deltas.
///
/// Unlike the public Khala paths, a non-owner caller is expected to receive
/// 401/403 here. The app surfaces that as the existing `.unauthorized` error so
/// the channel degrades gracefully (no public-Khala fallback, no roleplay).
extension KhalaClient {
    /// The Artanis operator endpoint. It lives at the site root, NOT under the
    /// public `/api/v1` OpenAI-compatible base.
    static let artanisChatURL = URL(string: "https://openagents.com/api/operator/artanis/chat")!

    /// Send the full conversation to the owner-only Artanis operator channel and
    /// return the operator's reply. Non-streaming, per the shared contract.
    static func artanisChat(
        messages: [OutgoingMessage],
        apiKey: String,
        session: URLSession = .shared
    ) async throws -> String {
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }

        var request = URLRequest(url: artanisChatURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(trimmedKey)", forHTTPHeaderField: "Authorization")

        let body: [String: Any] = ["messages": messages.map(\.json)]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let reply = obj["reply"] as? String
            else {
                throw KhalaError.decoding
            }
            return reply
        } catch let err as KhalaError {
            throw err
        } catch {
            throw KhalaError.transport(error)
        }
    }

    /// Stream the owner-only Artanis operator channel. This is the real two-way
    /// operator path: the app posts the full conversation to the authenticated
    /// Worker route, receives SSE deltas signed/provenanced as the Artanis
    /// interaction signature, and renders them into the same transcript UI as
    /// public Khala.
    static func streamArtanisCompletion(
        messages: [OutgoingMessage],
        apiKey: String,
        session: URLSession = .shared
    ) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let request = try makeArtanisStreamRequest(messages: messages, apiKey: apiKey)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
                    if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
                    if http.statusCode == 402 { throw KhalaError.quotaExceeded }
                    guard (200..<300).contains(http.statusCode) else {
                        let body = await Self.collectErrorBody(from: bytes)
                        throw KhalaError.http(http.statusCode, body)
                    }

                    for try await line in bytes.lines {
                        try Task.checkCancellation()
                        guard let delta = Self.delta(fromSSELine: line) else { continue }
                        if delta.isDone { break }
                        if let content = delta.content, !content.isEmpty {
                            continuation.yield(content)
                        }
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish(throwing: CancellationError())
                } catch let err as KhalaError {
                    continuation.finish(throwing: err)
                } catch {
                    continuation.finish(throwing: KhalaError.transport(error))
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    private static func makeArtanisStreamRequest(
        messages: [OutgoingMessage],
        apiKey: String
    ) throws -> URLRequest {
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }

        var request = URLRequest(url: artanisChatURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(trimmedKey)", forHTTPHeaderField: "Authorization")

        let body: [String: Any] = ["messages": messages.map(\.json)]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        return request
    }

    private static func collectErrorBody(
        from bytes: URLSession.AsyncBytes,
        limit: Int = 2_048
    ) async -> String {
        var collected = Data()
        do {
            for try await byte in bytes {
                collected.append(byte)
                if collected.count >= limit { break }
            }
        } catch {
            // Best-effort; return whatever was read.
        }
        return String(data: collected, encoding: .utf8) ?? ""
    }
}
