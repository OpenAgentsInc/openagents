import Foundation
import FoundationModels

// A line-delimited JSON bridge to Apple's on-device foundation model: one
// request object a line on stdin, one response object a line on stdout,
// diagnostics on stderr. Restored from the Lev helper (swift/lev-bridge,
// removed 2026-10), without the adapter operations.
//
// Requests: {"id":"1","op":"availability"}
//           {"id":"2","op":"generate","instructions":"…","prompt":"…","maxTokens":40}
//           {"id":"3","op":"decide","prompt":"…","options":["a","b"]}
// Every request gets its own session, so no request sees another's text.

setvbuf(stdout, nil, _IOLBF, 0)

struct Request: Decodable {
    let id: String
    let op: String
    var instructions: String?
    var prompt: String?
    var options: [String]?
    var maxTokens: Int?
    var temperature: Double?
}

struct ErrorBody: Encodable { var code: String; var message: String }

struct Response: Encodable {
    var id: String
    var ok: Bool
    var status: String?
    var reason: String?
    var text: String?
    var choice: String?
    var latencyMs: Int?
    var error: ErrorBody?
    static func failure(_ id: String, _ code: String, _ message: String) -> Response {
        Response(id: id, ok: false, error: ErrorBody(code: code, message: message))
    }
}

let encoder = JSONEncoder()
encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]

func emit(_ r: Response) {
    guard let data = try? encoder.encode(r), let line = String(data: data, encoding: .utf8) else {
        FileHandle.standardError.write(Data("apple-fm-bridge: encode failed\n".utf8)); return
    }
    print(line)
}

func availability(_ q: Request) -> Response {
    var r = Response(id: q.id, ok: true)
    switch SystemLanguageModel.default.availability {
    case .available: r.status = "available"
    case .unavailable(let why):
        r.status = "unavailable"
        switch why {
        case .appleIntelligenceNotEnabled: r.reason = "apple_intelligence_not_enabled"
        case .deviceNotEligible: r.reason = "device_not_eligible"
        case .modelNotReady: r.reason = "model_not_ready"
        @unknown default: r.reason = "unknown"
        }
    @unknown default: r.status = "unknown"
    }
    return r
}

func options(_ q: Request) -> GenerationOptions {
    GenerationOptions(sampling: .greedy, temperature: q.temperature, maximumResponseTokens: q.maxTokens)
}

func failure(_ id: String, _ error: Error) -> Response {
    if let g = error as? LanguageModelSession.GenerationError {
        let code: String
        switch g {
        case .exceededContextWindowSize: code = "exceeded_context_window_size"
        case .assetsUnavailable: code = "assets_unavailable"
        case .guardrailViolation: code = "guardrail_violation"
        case .unsupportedGuide: code = "unsupported_guide"
        case .unsupportedLanguageOrLocale: code = "unsupported_language_or_locale"
        case .decodingFailure: code = "decoding_failure"
        case .rateLimited: code = "rate_limited"
        case .concurrentRequests: code = "concurrent_requests"
        case .refusal: code = "refusal"
        @unknown default: code = "unknown"
        }
        return Response.failure(id, code, g.localizedDescription)
    }
    return Response.failure(id, "server_error", "\(error)")
}

func generate(_ q: Request) async -> Response {
    let start = Date()
    do {
        let s = LanguageModelSession(instructions: q.instructions ?? "")
        let out = try await s.respond(to: q.prompt ?? "", options: options(q))
        var r = Response(id: q.id, ok: true)
        r.text = out.content
        r.latencyMs = Int(Date().timeIntervalSince(start) * 1000)
        return r
    } catch { return failure(q.id, error) }
}

// One choice from the admitted options, constrained by a generation schema,
// so caller text cannot add an option.
func decide(_ q: Request) async -> Response {
    guard let admitted = q.options, !admitted.isEmpty else {
        return Response.failure(q.id, "invalid_request", "decide needs a non-empty options array")
    }
    let start = Date()
    do {
        let choice = DynamicGenerationSchema.Property(
            name: "choice", description: "The single option that answers the question.",
            schema: DynamicGenerationSchema(name: "Choice", description: nil, anyOf: admitted))
        let schema = try GenerationSchema(
            root: DynamicGenerationSchema(name: "Decision", description: "One typed judgment.", properties: [choice]),
            dependencies: [])
        let s = LanguageModelSession(instructions: q.instructions ?? "")
        let out = try await s.respond(to: q.prompt ?? "", schema: schema, options: options(q))
        var r = Response(id: q.id, ok: true)
        r.choice = try out.content.value(String.self, forProperty: "choice")
        r.latencyMs = Int(Date().timeIntervalSince(start) * 1000)
        return r
    } catch { return failure(q.id, error) }
}

let decoder = JSONDecoder()
while let line = readLine(strippingNewline: true) {
    if line.isEmpty { continue }
    guard let data = line.data(using: .utf8), let q = try? decoder.decode(Request.self, from: data) else {
        emit(Response.failure("", "invalid_request", "request did not parse")); continue
    }
    switch q.op {
    case "availability": emit(availability(q))
    case "generate": emit(await generate(q))
    case "decide": emit(await decide(q))
    default: emit(Response.failure(q.id, "invalid_request", "unknown op '\(q.op)'"))
    }
}
