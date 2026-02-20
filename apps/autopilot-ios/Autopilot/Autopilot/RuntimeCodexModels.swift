import Foundation

enum RuntimeCodexApiErrorCode: String {
    case auth
    case forbidden
    case conflict
    case invalid
    case network
    case unknown
}

struct RuntimeCodexApiError: LocalizedError {
    let message: String
    let code: RuntimeCodexApiErrorCode
    let status: Int?

    var errorDescription: String? {
        message
    }
}

struct RuntimeCodexProjectionStatus: Decodable {
    let documentID: String
    let lastRuntimeSeq: Int
    let lagEvents: Int
    let status: String
    let projectionVersion: String
    let lastProjectedAt: String?

    enum CodingKeys: String, CodingKey {
        case documentID = "document_id"
        case lastRuntimeSeq = "last_runtime_seq"
        case lagEvents = "lag_events"
        case status
        case projectionVersion = "projection_version"
        case lastProjectedAt = "last_projected_at"
    }
}

struct RuntimeCodexWorkerSummary: Decodable, Identifiable {
    let workerID: String
    let status: String
    let latestSeq: Int
    let workspaceRef: String?
    let codexHomeRef: String?
    let adapter: String
    let metadata: [String: JSONValue]?
    let khalaProjection: RuntimeCodexProjectionStatus?

    var id: String { workerID }

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case status
        case latestSeq = "latest_seq"
        case workspaceRef = "workspace_ref"
        case codexHomeRef = "codex_home_ref"
        case adapter
        case metadata
        case khalaProjection = "khala_projection"
    }
}

struct RuntimeCodexWorkerSnapshot: Decodable {
    let workerID: String
    let status: String
    let latestSeq: Int
    let workspaceRef: String?
    let codexHomeRef: String?
    let adapter: String
    let metadata: [String: JSONValue]?

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case status
        case latestSeq = "latest_seq"
        case workspaceRef = "workspace_ref"
        case codexHomeRef = "codex_home_ref"
        case adapter
        case metadata
    }
}

struct RuntimeCodexStreamEvent {
    let id: Int?
    let event: String
    let payload: JSONValue
    let rawData: String

    var cursorHint: Int? {
        if let id {
            return id
        }

        guard let payloadObject = payload.objectValue else {
            return nil
        }

        if let seq = payloadObject["seq"]?.intValue {
            return seq
        }

        if let seq = payloadObject["latest_seq"]?.intValue {
            return seq
        }

        return nil
    }
}

struct RuntimeCodexStreamBatch {
    let events: [RuntimeCodexStreamEvent]
    let nextCursor: Int
}

struct RuntimeCodexAuthSession: Equatable {
    let userID: String?
    let email: String?
    let token: String
}

enum HandshakeState: Equatable {
    case idle
    case sending
    case waitingAck(handshakeID: String)
    case success(handshakeID: String)
    case timedOut(handshakeID: String)
    case failed(message: String)
}

enum StreamState: Equatable {
    case idle
    case connecting
    case live
    case reconnecting
}

enum AuthState: Equatable {
    case signedOut
    case sendingCode
    case codeSent(email: String)
    case verifying
    case authenticated(email: String?)
}

enum JSONValue: Codable, Equatable {
    case string(String)
    case int(Int)
    case double(Double)
    case bool(Bool)
    case object([String: JSONValue])
    case array([JSONValue])
    case null

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode(Int.self) {
            self = .int(value)
        } else if let value = try? container.decode(Double.self) {
            self = .double(value)
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode([String: JSONValue].self) {
            self = .object(value)
        } else if let value = try? container.decode([JSONValue].self) {
            self = .array(value)
        } else {
            throw DecodingError.typeMismatch(
                JSONValue.self,
                DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Unsupported JSON value")
            )
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        switch self {
        case .string(let value):
            try container.encode(value)
        case .int(let value):
            try container.encode(value)
        case .double(let value):
            try container.encode(value)
        case .bool(let value):
            try container.encode(value)
        case .object(let value):
            try container.encode(value)
        case .array(let value):
            try container.encode(value)
        case .null:
            try container.encodeNil()
        }
    }

    var stringValue: String? {
        switch self {
        case .string(let value):
            return value
        default:
            return nil
        }
    }

    var intValue: Int? {
        switch self {
        case .int(let value):
            return value
        case .double(let value):
            return Int(value)
        case .string(let value):
            return Int(value)
        default:
            return nil
        }
    }

    var objectValue: [String: JSONValue]? {
        switch self {
        case .object(let value):
            return value
        default:
            return nil
        }
    }
}

extension Dictionary where Key == String, Value == JSONValue {
    subscript(string key: String) -> String? {
        self[key]?.stringValue
    }

    subscript(int key: String) -> Int? {
        self[key]?.intValue
    }
}
