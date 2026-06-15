import Dispatch
import Foundation
import FoundationModels
import Network

private let bridgeVersion = "0.1.0"
private let defaultPort: UInt16 = 11435
private let modelId = "apple-foundation-model"
private let maxRequestBytes = 1_048_576

@main
enum FoundationBridge {
    static func main() async throws {
        let port = parsePort(CommandLine.arguments.dropFirst().first)
        let handler = AppleFmHandler()
        let server = try HttpServer(port: port, handler: handler)
        server.start()
        writeStderr("foundation-bridge listening on 127.0.0.1:\(port)\n")
        dispatchMain()
    }

    private static func parsePort(_ value: String?) -> UInt16 {
        guard let value, let port = UInt16(value) else {
            return defaultPort
        }
        return port
    }
}

actor AppleFmHandler {
    func health() -> HealthResponse {
        let model = SystemLanguageModel.default

        switch model.availability {
        case .available:
            return HealthResponse(
                ready: true,
                model: modelId,
                modelId: modelId,
                unavailableReason: nil,
                message: "Apple Foundation Models is available.",
                platform: "macOS",
                version: bridgeVersion
            )
        case .unavailable(let reason):
            let reasonText = String(describing: reason)
            return HealthResponse(
                ready: false,
                model: modelId,
                modelId: modelId,
                unavailableReason: mapUnavailableReason(reasonText),
                message: "Apple Foundation Models unavailable: \(reasonText)",
                platform: "macOS",
                version: bridgeVersion
            )
        @unknown default:
            return HealthResponse(
                ready: false,
                model: modelId,
                modelId: modelId,
                unavailableReason: "unknown",
                message: "Apple Foundation Models availability returned an unknown state.",
                platform: "macOS",
                version: bridgeVersion
            )
        }
    }

    func complete(_ request: ChatCompletionRequest) async throws -> ChatCompletionResponse {
        let availability = health()
        guard availability.ready else {
            throw BridgeError.notReady(availability)
        }

        let prompt = buildPrompt(from: request.messages)
        let session = LanguageModelSession()

        do {
            let response = try await session.respond(to: prompt)
            let content = response.content
            let usage = estimateUsage(prompt: prompt, completion: content)

            return ChatCompletionResponse(
                id: "apple-fm-\(UUID().uuidString.lowercased())",
                model: request.model ?? modelId,
                choices: [
                    ChatCompletionChoice(
                        index: 0,
                        message: ChatMessage(role: "assistant", content: content, name: nil, toolCallId: nil),
                        finishReason: "stop"
                    )
                ],
                usage: usage
            )
        } catch {
            throw BridgeError.generationFailed("Apple Foundation Models request failed: \(error.localizedDescription)")
        }
    }

    private func buildPrompt(from messages: [ChatMessage]) -> String {
        messages.map { message in
            switch message.role {
            case "system":
                return "System: \(message.content)"
            case "user":
                return "User: \(message.content)"
            case "assistant":
                return "Assistant: \(message.content)"
            case "tool":
                return "Tool Result: \(message.content)"
            default:
                return "\(message.role): \(message.content)"
            }
        }.joined(separator: "\n\n")
    }

    private func estimateUsage(prompt: String, completion: String) -> UsageMeasurement {
        let promptTokens = max(1, prompt.count / 4)
        let completionTokens = max(1, completion.count / 4)
        return UsageMeasurement(
            truth: "estimated",
            promptTokens: promptTokens,
            completionTokens: completionTokens,
            totalTokens: promptTokens + completionTokens
        )
    }

    private func mapUnavailableReason(_ rawReason: String) -> String {
        let reason = rawReason.lowercased()

        if reason.contains("unsupported") || reason.contains("ineligible") || reason.contains("device") {
            return "unsupported_hardware"
        }

        if reason.contains("appleintelligence") || reason.contains("intelligence") || reason.contains("disabled") {
            return "apple_intelligence_disabled"
        }

        if reason.contains("permission") || reason.contains("denied") {
            return "permission_denied"
        }

        if reason.contains("model") || reason.contains("asset") || reason.contains("download") {
            return "model_unavailable"
        }

        return "unknown"
    }
}

final class HttpServer: @unchecked Sendable {
    private let listener: NWListener
    private let handler: AppleFmHandler

    init(port: UInt16, handler: AppleFmHandler) throws {
        guard let nwPort = NWEndpoint.Port(rawValue: port) else {
            throw BridgeError.badRequest("Invalid port: \(port)")
        }

        self.listener = try NWListener(using: .tcp, on: nwPort)
        self.handler = handler
    }

    func start() {
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else {
                connection.cancel()
                return
            }

            connection.start(queue: .global(qos: .userInitiated))
            Task {
                await self.handle(connection)
            }
        }

        listener.stateUpdateHandler = { state in
            if case .failed(let error) = state {
                writeStderr("foundation-bridge listener failed: \(error)\n")
            }
        }

        listener.start(queue: .global(qos: .userInitiated))
    }

    private func handle(_ connection: NWConnection) async {
        do {
            let raw = try await receiveRequest(from: connection)
            let request = try HttpRequest.parse(raw)
            let response = await route(request)
            try await send(response, on: connection)
        } catch {
            let response = HttpResponse.json(
                status: 400,
                body: ErrorResponse(error: "bad_request", message: error.localizedDescription, unavailableReason: nil)
            )
            try? await send(response, on: connection)
        }

        connection.cancel()
    }

    private func route(_ request: HttpRequest) async -> HttpResponse {
        if request.method == "OPTIONS" {
            return HttpResponse(status: 204, contentType: "text/plain; charset=utf-8", body: Data())
        }

        if request.method == "GET" && request.path == "/health" {
            return await HttpResponse.json(body: handler.health())
        }

        if request.method == "GET" && (request.path == "/v1/models" || request.path == "/models") {
            return HttpResponse.json(
                body: ModelsResponse(
                    data: [
                        ModelDescriptor(id: modelId, ownedBy: "apple")
                    ]
                )
            )
        }

        if request.method == "POST" && (request.path == "/v1/chat/completions" || request.path == "/chat/completions") {
            return await completeChat(request)
        }

        if request.method == "POST" && request.path == "/v1/sessions" {
            return HttpResponse.json(
                body: SessionCreateResponse(session: SessionDescriptor(id: "apple_fm_session_\(UUID().uuidString.lowercased())"))
            )
        }

        if request.method == "POST" && request.path.hasPrefix("/v1/sessions/") && request.path.hasSuffix("/responses/stream") {
            return await streamSessionResponse(request)
        }

        return HttpResponse.json(
            status: 404,
            body: ErrorResponse(error: "not_found", message: "No route for \(request.method) \(request.path)", unavailableReason: nil)
        )
    }

    private func completeChat(_ request: HttpRequest) async -> HttpResponse {
        do {
            let decoded = try JSONDecoder().decode(ChatCompletionRequest.self, from: request.body)
            let completion = try await handler.complete(decoded)
            return HttpResponse.json(body: completion)
        } catch BridgeError.notReady(let health) {
            return HttpResponse.json(
                status: 503,
                body: ErrorResponse(
                    error: "apple_fm_not_ready",
                    message: health.message ?? "Apple Foundation Models is not ready.",
                    unavailableReason: health.unavailableReason
                )
            )
        } catch BridgeError.generationFailed(let message) {
            return HttpResponse.json(
                status: 500,
                body: ErrorResponse(error: "generation_failed", message: message, unavailableReason: "model_unavailable")
            )
        } catch {
            return HttpResponse.json(
                status: 400,
                body: ErrorResponse(error: "bad_request", message: error.localizedDescription, unavailableReason: nil)
            )
        }
    }

    private func streamSessionResponse(_ request: HttpRequest) async -> HttpResponse {
        do {
            let decoded = try JSONDecoder().decode(SessionStreamRequest.self, from: request.body)
            let completion = try await handler.complete(
                ChatCompletionRequest(model: modelId, messages: [ChatMessage(role: "user", content: decoded.prompt, name: nil, toolCallId: nil)])
            )
            let output = completion.choices.first?.message.content ?? ""
            let usage = completion.usage ?? UsageMeasurement(truth: "unknown", promptTokens: nil, completionTokens: nil, totalTokens: nil)
            let snapshot = StreamSnapshot(sequence: 0, content: output, output: output, finishReason: "stop")
            let completed = StreamCompleted(output: output, content: output, model: completion.model ?? modelId, usage: usage)
            let body = [
                encodeSse(event: "snapshot", value: snapshot),
                encodeSse(event: "completed", value: completed)
            ].joined(separator: "")
            return HttpResponse(status: 200, contentType: "text/event-stream; charset=utf-8", body: Data(body.utf8))
        } catch BridgeError.notReady(let health) {
            return HttpResponse.json(
                status: 503,
                body: ErrorResponse(
                    error: "apple_fm_not_ready",
                    message: health.message ?? "Apple Foundation Models is not ready.",
                    unavailableReason: health.unavailableReason
                )
            )
        } catch {
            return HttpResponse.json(
                status: 500,
                body: ErrorResponse(error: "tool_stream_failed", message: error.localizedDescription, unavailableReason: "model_unavailable")
            )
        }
    }
}

struct HttpRequest {
    let method: String
    let path: String
    let headers: [String: String]
    let body: Data

    static func parse(_ data: Data) throws -> HttpRequest {
        guard let split = data.range(of: Data("\r\n\r\n".utf8)) else {
            throw BridgeError.badRequest("HTTP headers were incomplete.")
        }

        guard let headerText = String(data: data[..<split.lowerBound], encoding: .utf8) else {
            throw BridgeError.badRequest("HTTP headers were not UTF-8.")
        }

        let lines = headerText.components(separatedBy: "\r\n")
        guard let requestLine = lines.first else {
            throw BridgeError.badRequest("Missing HTTP request line.")
        }

        let requestParts = requestLine.split(separator: " ", maxSplits: 2).map(String.init)
        guard requestParts.count >= 2 else {
            throw BridgeError.badRequest("Malformed HTTP request line.")
        }

        var headers: [String: String] = [:]
        for line in lines.dropFirst() {
            let parts = line.split(separator: ":", maxSplits: 1).map(String.init)
            guard parts.count == 2 else {
                continue
            }
            headers[parts[0].lowercased()] = parts[1].trimmingCharacters(in: .whitespaces)
        }

        let bodyStart = split.upperBound
        let contentLength = Int(headers["content-length"] ?? "0") ?? 0
        let bodyEnd = min(data.count, bodyStart + contentLength)
        let path = requestParts[1].split(separator: "?", maxSplits: 1).first.map(String.init) ?? requestParts[1]

        return HttpRequest(
            method: requestParts[0].uppercased(),
            path: path,
            headers: headers,
            body: data.subdata(in: bodyStart..<bodyEnd)
        )
    }
}

struct HttpResponse {
    let status: Int
    let contentType: String
    let body: Data

    static func json<T: Encodable>(status: Int = 200, body: T) -> HttpResponse {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = (try? encoder.encode(body)) ?? Data("{}".utf8)
        return HttpResponse(status: status, contentType: "application/json; charset=utf-8", body: data)
    }
}

struct HealthResponse: Codable {
    let ready: Bool
    let model: String?
    let modelId: String?
    let unavailableReason: String?
    let message: String?
    let platform: String?
    let version: String?
}

struct ChatCompletionRequest: Codable {
    let model: String?
    let messages: [ChatMessage]
    let temperature: Double?
    let maxTokens: Int?

    init(model: String?, messages: [ChatMessage], temperature: Double? = nil, maxTokens: Int? = nil) {
        self.model = model
        self.messages = messages
        self.temperature = temperature
        self.maxTokens = maxTokens
    }
}

struct ChatMessage: Codable {
    let role: String
    let content: String
    let name: String?
    let toolCallId: String?
}

struct ChatCompletionResponse: Codable {
    let id: String?
    let model: String?
    let choices: [ChatCompletionChoice]
    let usage: UsageMeasurement?
}

struct ChatCompletionChoice: Codable {
    let index: Int?
    let message: ChatMessage
    let finishReason: String?
}

struct UsageMeasurement: Codable {
    let truth: String
    let promptTokens: Int?
    let completionTokens: Int?
    let totalTokens: Int?
}

struct SessionStreamRequest: Codable {
    let prompt: String
}

struct SessionCreateResponse: Codable {
    let session: SessionDescriptor
}

struct SessionDescriptor: Codable {
    let id: String
}

struct ModelsResponse: Codable {
    let data: [ModelDescriptor]
}

struct ModelDescriptor: Codable {
    let id: String
    let ownedBy: String
}

struct StreamSnapshot: Codable {
    let sequence: Int
    let content: String
    let output: String
    let finishReason: String
}

struct StreamCompleted: Codable {
    let output: String
    let content: String
    let model: String
    let usage: UsageMeasurement
}

struct ErrorResponse: Codable {
    let error: String
    let message: String
    let unavailableReason: String?
}

enum BridgeError: Error {
    case badRequest(String)
    case notReady(HealthResponse)
    case generationFailed(String)
}

private func receiveRequest(from connection: NWConnection) async throws -> Data {
    var buffer = Data()

    while buffer.count < maxRequestBytes {
        let chunk = try await receiveChunk(from: connection)
        buffer.append(chunk.data)

        if requestIsComplete(buffer) || chunk.isComplete {
            return buffer
        }
    }

    throw BridgeError.badRequest("HTTP request exceeded \(maxRequestBytes) bytes.")
}

private func receiveChunk(from connection: NWConnection) async throws -> (data: Data, isComplete: Bool) {
    try await withCheckedThrowingContinuation { continuation in
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { data, _, isComplete, error in
            if let error {
                continuation.resume(throwing: error)
                return
            }

            continuation.resume(returning: (data ?? Data(), isComplete))
        }
    }
}

private func requestIsComplete(_ data: Data) -> Bool {
    guard let split = data.range(of: Data("\r\n\r\n".utf8)) else {
        return false
    }

    guard let headerText = String(data: data[..<split.lowerBound], encoding: .utf8) else {
        return true
    }

    let contentLength = headerText
        .components(separatedBy: "\r\n")
        .compactMap { line -> Int? in
            let parts = line.split(separator: ":", maxSplits: 1).map(String.init)
            guard parts.count == 2, parts[0].lowercased() == "content-length" else {
                return nil
            }
            return Int(parts[1].trimmingCharacters(in: .whitespaces))
        }
        .first ?? 0

    return data.count >= split.upperBound + contentLength
}

private func send(_ response: HttpResponse, on connection: NWConnection) async throws {
    let reason = statusReason(response.status)
    let headers = """
    HTTP/1.1 \(response.status) \(reason)\r
    Content-Type: \(response.contentType)\r
    Content-Length: \(response.body.count)\r
    Cache-Control: no-store\r
    Access-Control-Allow-Origin: *\r
    Access-Control-Allow-Headers: Content-Type, Authorization\r
    Access-Control-Allow-Methods: GET, POST, OPTIONS\r
    Connection: close\r
    \r

    """
    var payload = Data(headers.utf8)
    payload.append(response.body)

    try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
        connection.send(content: payload, completion: .contentProcessed { error in
            if let error {
                continuation.resume(throwing: error)
            } else {
                continuation.resume(returning: ())
            }
        })
    }
}

private func encodeSse<T: Encodable>(event: String, value: T) -> String {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys]
    let data = (try? encoder.encode(value)) ?? Data("{}".utf8)
    let json = String(data: data, encoding: .utf8) ?? "{}"
    return "event: \(event)\ndata: \(json)\n\n"
}

private func statusReason(_ status: Int) -> String {
    switch status {
    case 200:
        return "OK"
    case 204:
        return "No Content"
    case 400:
        return "Bad Request"
    case 404:
        return "Not Found"
    case 500:
        return "Internal Server Error"
    case 503:
        return "Service Unavailable"
    default:
        return "OK"
    }
}

private func writeStderr(_ value: String) {
    FileHandle.standardError.write(Data(value.utf8))
}
