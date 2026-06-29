import Foundation

/// Streaming, multi-turn chat path for the public Khala API.
///
/// This is the SSE companion to `KhalaClient.complete(...)`. It sends the full
/// conversation history (`messages`) to `POST /api/v1/chat/completions` with
/// `stream: true`, parses the OpenAI-style `data:` SSE frames, and surfaces
/// token deltas as an `AsyncThrowingStream<String, Error>`.
///
/// Contract (source of truth: https://openagents.com/AGENTS.md):
/// - Base URL:  https://openagents.com/api/v1
/// - Endpoint:  POST /chat/completions   with `stream: true`
/// - Model:     openagents/khala         (one public model; no variants)
/// - Auth:      Authorization: Bearer <oa_agent_… key>
/// - SSE:       lines prefixed `data:`; `[DONE]` terminates the stream.
///
/// Errors are mapped onto `KhalaClient.KhalaError` so callers handle both the
/// streaming and non-streaming paths with one error surface. The stream is
/// fully cancellation-aware: cancelling the consuming `Task` tears down the
/// underlying URL byte stream.
extension KhalaClient {
    /// One outgoing chat turn for the `messages` array. Decoupled from the
    /// SwiftData `Message` model so the Net layer never imports SwiftData and
    /// the mapping stays testable.
    struct OutgoingMessage: Sendable, Equatable {
        let role: String
        let content: String

        init(role: String, content: String) {
            self.role = role
            self.content = content
        }

        var json: [String: String] { ["role": role, "content": content] }
    }

    /// Final assembled result of a streamed completion: the concatenated token
    /// deltas plus the assistant role.
    struct StreamedCompletion: Sendable, Equatable {
        let role: String
        let content: String
    }

    /// Stream a multi-turn completion. Sends the full `messages` history and
    /// yields assistant token deltas as they arrive. The returned stream
    /// finishes when the server sends `[DONE]` (or closes the body); it
    /// throws a `KhalaError` on HTTP/transport/decoding failures.
    ///
    /// To assemble the final message, accumulate the yielded deltas, or use
    /// `streamAssembled(...)` which returns the full `StreamedCompletion`.
    ///
    /// Cancellation: cancel the consuming `Task` to abort the request; the
    /// underlying byte stream is torn down and the producer task is cancelled.
    static func streamCompletion(
        messages: [OutgoingMessage],
        apiKey: String,
        system: String? = nil,
        session: URLSession = .shared
    ) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    var wire: [OutgoingMessage] = []
                    if let system, !system.isEmpty {
                        wire.append(OutgoingMessage(role: "system", content: system))
                    }
                    wire.append(contentsOf: messages)

                    let request = try makeStreamRequest(messages: wire, apiKey: apiKey)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
                    if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
                    if http.statusCode == 402 { throw KhalaError.quotaExceeded }
                    guard (200..<300).contains(http.statusCode) else {
                        // Drain a bounded amount of the error body for context.
                        let body = await Self.collectErrorBody(from: bytes)
                        throw KhalaError.http(http.statusCode, body)
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

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    /// Convenience: stream a completion and return the fully assembled message.
    /// Forwards each delta to `onDelta` (on the calling task's executor) as it
    /// arrives so callers can render live, then returns the assembled result.
    static func streamAssembled(
        messages: [OutgoingMessage],
        apiKey: String,
        system: String? = nil,
        session: URLSession = .shared,
        onDelta: ((String) -> Void)? = nil
    ) async throws -> StreamedCompletion {
        var assembled = ""
        for try await delta in streamCompletion(
            messages: messages,
            apiKey: apiKey,
            system: system,
            session: session
        ) {
            assembled += delta
            onDelta?(delta)
        }
        return StreamedCompletion(role: "assistant", content: assembled)
    }

    // MARK: - Request construction

    private static func makeStreamRequest(
        messages: [OutgoingMessage],
        apiKey: String
    ) throws -> URLRequest {
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }

        let url = baseURL.appendingPathComponent("chat/completions")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("text/event-stream", forHTTPHeaderField: "Accept")
        request.setValue("Bearer \(trimmedKey)", forHTTPHeaderField: "Authorization")

        let body: [String: Any] = [
            "model": model,
            "stream": true,
            "messages": messages.map(\.json),
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        return request
    }

    // MARK: - SSE parsing

    /// Parsed view of one SSE `data:` line.
    struct SSEDelta: Equatable {
        let content: String?
        let isDone: Bool
    }

    /// Parse a single-line SSE event into a delta. Returns `nil` for blank
    /// lines, comments (`:`), and non-`data:` event fields. `[DONE]` maps to
    /// `isDone == true`.
    static func delta(fromSSELine line: String) -> SSEDelta? {
        delta(fromSSEFrameLines: [line])
    }

    /// Parse a complete SSE event frame. Multiple `data:` fields are joined
    /// with newlines per the SSE contract, which lets OpenAI-compatible JSON
    /// payloads arrive split across several data lines.
    static func delta(fromSSEFrameLines lines: [String]) -> SSEDelta? {
        let payload = sseDataPayload(fromFrameLines: lines)
        guard !payload.isEmpty else { return nil }
        if payload == "[DONE]" { return SSEDelta(content: nil, isDone: true) }

        guard let data = payload.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let choices = obj["choices"] as? [[String: Any]]
        else {
            // Unknown but well-formed-ish frame; skip rather than abort the stream.
            return nil
        }

        let content = choices.compactMap { choice -> String? in
            if let delta = choice["delta"] as? [String: Any],
               let content = delta["content"] as? String {
                return content
            }
            if let message = choice["message"] as? [String: Any],
               let content = message["content"] as? String {
                return content
            }
            return nil
        }
        .joined()

        return SSEDelta(content: content.isEmpty ? nil : content, isDone: false)
    }

    static func sseDataPayload(fromFrameLines lines: [String]) -> String {
        lines.compactMap { line -> String? in
            let trimmed = line.trimmingCharacters(in: CharacterSet(charactersIn: "\r"))
            guard trimmed.hasPrefix("data:") else { return nil }
            var payload = String(trimmed.dropFirst("data:".count))
            if payload.hasPrefix(" ") {
                payload.removeFirst()
            }
            return payload
        }
        .joined(separator: "\n")
        .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func flushSSEFrameLines(
        _ frameLines: inout [String],
        to continuation: AsyncThrowingStream<String, Error>.Continuation
    ) -> Bool {
        defer { frameLines.removeAll(keepingCapacity: true) }
        guard let delta = delta(fromSSEFrameLines: frameLines) else { return false }
        if delta.isDone { return true }
        if let content = delta.content, !content.isEmpty {
            continuation.yield(content)
        }
        return false
    }

    /// Read a bounded prefix of an error response body for diagnostic context.
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
