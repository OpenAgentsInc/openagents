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
/// - Response:  OpenAI-style SSE frames when `Accept: text/event-stream`
///
/// Unlike the public Khala paths, a non-owner caller is expected to receive
/// 401/403 here. The app surfaces that as the existing `.unauthorized` error so
/// the channel degrades gracefully (no public-Khala fallback, no roleplay).
extension KhalaClient {
    /// The Artanis operator endpoint. It lives at the site root, NOT under the
    /// public `/api/v1` OpenAI-compatible base.
    static let artanisChatURL = URL(string: "https://openagents.com/api/operator/artanis/chat")!

    /// Stream the owner-only Artanis operator channel. The endpoint returns
    /// OpenAI-style SSE frames, so this parser reuses the public Khala delta
    /// contract while keeping the owner-only URL/auth boundary separate.
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
                        let body = await Self.collectArtanisErrorBody(from: bytes)
                        throw KhalaError.http(http.statusCode, body)
                    }

                    let contentType = http.value(forHTTPHeaderField: "Content-Type")?.lowercased() ?? ""
                    if !contentType.contains("text/event-stream") {
                        let body = await Self.collectArtanisBody(from: bytes)
                        guard let reply = Self.artanisReply(from: body) else {
                            throw KhalaError.decoding
                        }
                        if !reply.isEmpty {
                            continuation.yield(reply)
                        }
                        continuation.finish()
                        return
                    }

                    var frameLines: [String] = []
                    var streamIsDone = false
                    for try await line in bytes.lines {
                        try Task.checkCancellation()
                        if line.trimmingCharacters(in: .newlines).isEmpty {
                            streamIsDone = Self.flushSSEFrameLines(&frameLines, to: continuation)
                            if streamIsDone { break }
                        } else {
                            frameLines.append(line)
                            if let delta = Self.delta(fromSSEFrameLines: frameLines) {
                                frameLines.removeAll(keepingCapacity: true)
                                if delta.isDone {
                                    streamIsDone = true
                                    break
                                }
                                if let content = delta.content, !content.isEmpty {
                                    continuation.yield(content)
                                }
                            }
                        }
                    }
                    if !streamIsDone {
                        _ = Self.flushSSEFrameLines(&frameLines, to: continuation)
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish(throwing: CancellationError())
                } catch let err as KhalaError {
                    continuation.finish(throwing: err)
                } catch let urlError as URLError where urlError.code == .cancelled {
                    continuation.finish(throwing: CancellationError())
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

    private static func collectArtanisErrorBody(
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

    private static func collectArtanisBody(from bytes: URLSession.AsyncBytes) async -> Data {
        var collected = Data()
        do {
            for try await byte in bytes {
                collected.append(byte)
            }
        } catch {
            // Best-effort; parser will fail closed if the response is incomplete.
        }
        return collected
    }

    private static func artanisReply(from data: Data) -> String? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let reply = obj["reply"] as? String
        else { return nil }
        return reply
    }
}
