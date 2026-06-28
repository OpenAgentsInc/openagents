import Foundation

/// Attach-only client for the local Apple Foundation Models bridge.
///
/// The bridge is intentionally local and readiness-first. We always check
/// `GET /health` before inference, and unsupported machines are modeled as an
/// honest status rather than as a transport failure.
enum AppleFMClient {
    static let defaultBaseURL = URL(string: "http://127.0.0.1:11435")!
    static let model = "apple-foundation-model"

    enum Availability: String, Equatable, Sendable {
        case ready
        case unavailable
        case unsupported
    }

    enum UsageTruth: String, Equatable, Sendable {
        case exact
        case estimated
        case unknown
    }

    struct Status: Equatable, Sendable {
        let availability: Availability
        let baseURL: URL
        let model: String
        let message: String
        let blockerRefs: [String]
        let platform: String?

        var isReady: Bool { availability == .ready }

        static func unavailable(baseURL: URL, message: String, blockerRefs: [String]) -> Status {
            Status(
                availability: .unavailable,
                baseURL: baseURL,
                model: AppleFMClient.model,
                message: message,
                blockerRefs: blockerRefs,
                platform: nil
            )
        }
    }

    struct Snapshot: Equatable, Sendable {
        let content: String
        let usageTruth: UsageTruth
    }

    enum AppleFMError: Error, LocalizedError, Equatable {
        case notReady(Status)
        case http(Int, String)
        case decoding
        case transport(String)

        var errorDescription: String? {
            switch self {
            case .notReady(let status):
                return status.message
            case .http(let code, _):
                return "Apple FM bridge returned HTTP \(code)."
            case .decoding:
                return "Could not read the Apple FM bridge response."
            case .transport(let message):
                return message
            }
        }
    }

    static func resolvedBaseURL(environment: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        for key in ["PROBE_APPLE_FM_BASE_URL", "OPENAGENTS_APPLE_FM_BASE_URL"] {
            if let raw = environment[key]?.trimmingCharacters(in: .whitespacesAndNewlines),
               !raw.isEmpty,
               let url = URL(string: raw) {
                return url
            }
        }
        return defaultBaseURL
    }

    static func health(
        baseURL: URL = resolvedBaseURL(),
        session: URLSession = .shared
    ) async -> Status {
        let url = baseURL.appendingPathComponent("health")
        do {
            let (data, response) = try await session.data(for: URLRequest(url: url))
            guard let http = response as? HTTPURLResponse else {
                return .unavailable(
                    baseURL: baseURL,
                    message: "The Apple FM bridge did not return an HTTP response.",
                    blockerRefs: ["blocker.apple_fm.bridge_malformed_response"]
                )
            }
            guard (200..<300).contains(http.statusCode) else {
                return .unavailable(
                    baseURL: baseURL,
                    message: "The Apple FM bridge is not ready (HTTP \(http.statusCode)).",
                    blockerRefs: ["blocker.apple_fm.bridge_http_\(http.statusCode)"]
                )
            }
            return parseHealth(data, baseURL: baseURL)
        } catch {
            return .unavailable(
                baseURL: baseURL,
                message: "Start the local Apple FM bridge at \(baseURL.absoluteString), then try again.",
                blockerRefs: ["blocker.apple_fm.bridge_missing"]
            )
        }
    }

    static func streamSnapshotCompletion(
        messages: [KhalaClient.OutgoingMessage],
        baseURL: URL = resolvedBaseURL(),
        session: URLSession = .shared
    ) -> AsyncThrowingStream<Snapshot, Error> {
        AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let status = await health(baseURL: baseURL, session: session)
                    guard status.isReady else { throw AppleFMError.notReady(status) }

                    let snapshot = try await completePlainText(
                        messages: messages,
                        baseURL: baseURL,
                        session: session
                    )
                    continuation.yield(snapshot)
                    continuation.finish()
                } catch let error as AppleFMError {
                    continuation.finish(throwing: error)
                } catch let urlError as URLError where urlError.code == .cancelled {
                    continuation.finish(throwing: CancellationError())
                } catch {
                    continuation.finish(throwing: AppleFMError.transport(error.localizedDescription))
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

    private static func completePlainText(
        messages: [KhalaClient.OutgoingMessage],
        baseURL: URL,
        session: URLSession
    ) async throws -> Snapshot {
        var request = URLRequest(url: baseURL.appendingPathComponent("v1/chat/completions"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        let body: [String: Any] = [
            "model": model,
            "stream": false,
            "messages": messages.map(\.json),
        ]
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else { throw AppleFMError.decoding }
        guard (200..<300).contains(http.statusCode) else {
            throw AppleFMError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
        }
        return try parseCompletion(data)
    }

    private static func parseHealth(_ data: Data, baseURL: URL) -> Status {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return .unavailable(
                baseURL: baseURL,
                message: "The Apple FM bridge health response was not JSON.",
                blockerRefs: ["blocker.apple_fm.malformed_health"]
            )
        }

        let rawState = string(in: obj, keys: ["availability", "state", "status"])?.lowercased()
        let ready = bool(in: obj, keys: ["ready", "isReady", "available"]) == true
            || rawState == "ready"
            || rawState == "ok"
            || rawState == "healthy"
        let blockers = stringArray(in: obj, keys: ["blockerRefs", "blockers", "reasons", "unavailableReasons"])
        let lowerBlockers = blockers.map { $0.lowercased() }
        let unsupported = rawState == "unsupported"
            || bool(in: obj, keys: ["unsupported"]) == true
            || lowerBlockers.contains(where: { $0.contains("unsupported") || $0.contains("not_admitted") })
        let availability: Availability = ready ? .ready : (unsupported ? .unsupported : .unavailable)
        let fallbackMessage: String
        switch availability {
        case .ready:
            fallbackMessage = "Apple FM local backend ready."
        case .unsupported:
            fallbackMessage = "This Mac is not admitted for Apple FM yet."
        case .unavailable:
            fallbackMessage = "Apple FM bridge is reachable but not ready."
        }

        return Status(
            availability: availability,
            baseURL: baseURL,
            model: string(in: obj, keys: ["model", "modelId"]) ?? model,
            message: string(in: obj, keys: ["message", "detail", "reason"]) ?? fallbackMessage,
            blockerRefs: blockers,
            platform: string(in: obj, keys: ["platform", "system", "version"])
        )
    }

    private static func parseCompletion(_ data: Data) throws -> Snapshot {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw AppleFMError.decoding
        }

        let content: String?
        if let choices = obj["choices"] as? [[String: Any]],
           let first = choices.first {
            if let message = first["message"] as? [String: Any] {
                content = message["content"] as? String
            } else {
                content = first["text"] as? String
            }
        } else {
            content = obj["content"] as? String ?? obj["text"] as? String
        }
        guard let content else { throw AppleFMError.decoding }

        let usage = obj["usage"] as? [String: Any]
        let rawTruth = (
            string(in: usage ?? [:], keys: ["truth", "usageTruth", "measurement"])
                ?? string(in: obj, keys: ["usageTruth"])
                ?? ""
        ).lowercased()
        let usageTruth = UsageTruth(rawValue: rawTruth) ?? .unknown
        return Snapshot(content: content, usageTruth: usageTruth)
    }

    private static func string(in obj: [String: Any], keys: [String]) -> String? {
        for key in keys {
            if let value = obj[key] as? String, !value.isEmpty {
                return value
            }
        }
        return nil
    }

    private static func bool(in obj: [String: Any], keys: [String]) -> Bool? {
        for key in keys {
            if let value = obj[key] as? Bool {
                return value
            }
        }
        return nil
    }

    private static func stringArray(in obj: [String: Any], keys: [String]) -> [String] {
        for key in keys {
            if let values = obj[key] as? [String] {
                return values
            }
            if let values = obj[key] as? [Any] {
                return values.compactMap { $0 as? String }
            }
        }
        return []
    }
}
