import Foundation

/// One request line. `op` selects the operation; the other fields are the
/// arguments that operation reads.
struct Request: Decodable {
    let id: String
    let op: String
    var instructions: String?
    var prompt: String?
    var options: [String]?
    var band: [String]?
    var useCase: String?
    var guardrails: String?
    var sampling: Sampling?
    var maxTokens: Int?
    /// A `.fmadapter` package to attach for this call.
    var adapterPath: String?
    /// A name to ask the runtime about, for `adapter_compat`.
    var adapterName: String?
}

/// Sampling controls, mirroring the runtime's own validation rules: greedy
/// takes no top, no threshold, and no seed; random takes a top or a
/// threshold, never both.
struct Sampling: Decodable {
    var mode: String
    var seed: UInt64?
    var top: Int?
    var probabilityThreshold: Double?
    var temperature: Double?
}

struct AvailabilityBody: Encodable {
    var status: String
    var reason: String?
}

struct ErrorBody: Encodable {
    var code: String
    var message: String
}

struct Response: Encodable {
    var id: String
    var ok: Bool
    var availability: AvailabilityBody?
    var choice: String?
    var band: String?
    var text: String?
    var latencyMs: Double?
    var promptTokens: Int?
    var responseTokens: Int?
    /// Adapter identifiers the running base accepts for a given name.
    var compatibleAdapters: [String]?
    /// Producer metadata the loaded package carried.
    var adapterMetadata: [String: String]?
    var error: ErrorBody?

    static func failure(id: String, code: String, message: String) -> Response {
        Response(id: id, ok: false, error: ErrorBody(code: code, message: message))
    }
}
