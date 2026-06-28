import Foundation

extension KhalaClient {
    struct OutgoingMessage: Sendable, Equatable {
        let role: String
        let content: String

        var json: [String: String] {
            ["role": role, "content": content]
        }
    }

    struct SSEDelta: Equatable {
        let content: String?
        let isDone: Bool
    }

    static func streamCompletion(
        messages: [OutgoingMessage],
        apiKey: String,
        session: URLSession = .shared
    ) -> AsyncThrowingStream<String, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let request = try makeStreamRequest(messages: messages, apiKey: apiKey)
                    let (bytes, response) = try await session.bytes(for: request)
                    guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
                    if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
                    if http.statusCode == 402 { throw KhalaError.quotaExceeded }
                    guard (200..<300).contains(http.statusCode) else {
                        let body = await collectErrorBody(from: bytes)
                        throw KhalaError.http(http.statusCode, body)
                    }

                    for try await line in bytes.lines {
                        try Task.checkCancellation()
                        guard let delta = delta(fromSSELine: line) else { continue }
                        if delta.isDone { break }
                        if let content = delta.content, !content.isEmpty {
                            continuation.yield(content)
                        }
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish(throwing: CancellationError())
                } catch let error as KhalaError {
                    continuation.finish(throwing: error)
                } catch let urlError as URLError where urlError.code == .cancelled {
                    continuation.finish(throwing: CancellationError())
                } catch {
                    continuation.finish(throwing: KhalaError.transport(error.localizedDescription))
                }
            }

            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    private static func makeStreamRequest(messages: [OutgoingMessage], apiKey: String) throws -> URLRequest {
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

    static func delta(fromSSELine line: String) -> SSEDelta? {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("data:") else { return nil }

        let payload = String(trimmed.dropFirst("data:".count)).trimmingCharacters(in: .whitespaces)
        guard !payload.isEmpty else { return nil }
        if payload == "[DONE]" { return SSEDelta(content: nil, isDone: true) }

        guard let data = payload.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let choices = object["choices"] as? [[String: Any]]
        else {
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
        }.joined()

        return SSEDelta(content: content.isEmpty ? nil : content, isDone: false)
    }

    private static func collectErrorBody(from bytes: URLSession.AsyncBytes, limit: Int = 2_048) async -> String {
        var data = Data()
        do {
            for try await byte in bytes {
                data.append(byte)
                if data.count >= limit { break }
            }
        } catch {
        }
        return String(data: data, encoding: .utf8) ?? ""
    }
}
