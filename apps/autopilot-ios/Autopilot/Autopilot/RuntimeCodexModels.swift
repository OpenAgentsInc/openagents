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
        case convexProjection = "convex_projection"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        workerID = try container.decode(String.self, forKey: .workerID)
        status = try container.decode(String.self, forKey: .status)
        latestSeq = try container.decode(Int.self, forKey: .latestSeq)
        workspaceRef = try container.decodeIfPresent(String.self, forKey: .workspaceRef)
        codexHomeRef = try container.decodeIfPresent(String.self, forKey: .codexHomeRef)
        adapter = try container.decode(String.self, forKey: .adapter)
        metadata = container.decodeLenientMetadata(forKey: .metadata)
        khalaProjection =
            try container.decodeIfPresent(RuntimeCodexProjectionStatus.self, forKey: .khalaProjection)
            ?? container.decodeIfPresent(RuntimeCodexProjectionStatus.self, forKey: .convexProjection)
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

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        workerID = try container.decode(String.self, forKey: .workerID)
        status = try container.decode(String.self, forKey: .status)
        latestSeq = try container.decode(Int.self, forKey: .latestSeq)
        workspaceRef = try container.decodeIfPresent(String.self, forKey: .workspaceRef)
        codexHomeRef = try container.decodeIfPresent(String.self, forKey: .codexHomeRef)
        adapter = try container.decode(String.self, forKey: .adapter)
        metadata = container.decodeLenientMetadata(forKey: .metadata)
    }
}

private extension KeyedDecodingContainer {
    func decodeLenientMetadata(forKey key: Key) -> [String: JSONValue]? {
        if let object = try? decodeIfPresent([String: JSONValue].self, forKey: key) {
            return object
        }

        // Some workers still emit `metadata: []`; treat that as missing metadata.
        if (try? decodeIfPresent([JSONValue].self, forKey: key)) != nil {
            return nil
        }

        return nil
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

        if let envelope = RuntimeCodexProto.decodeWorkerEvent(from: payload),
           let seq = envelope.seq {
            return seq
        }

        return nil
    }
}

struct RuntimeCodexStreamBatch {
    let events: [RuntimeCodexStreamEvent]
    let nextCursor: Int
}

enum RuntimeCodexProto {
    static let workerEventType = "worker.event"
    static let iosHandshakeMethod = "ios/handshake"
    static let desktopHandshakeAckMethod = "desktop/handshake_ack"
    static let iosSource = "autopilot-ios"
    static let desktopSource = "autopilot-desktop"

    struct WorkerEventEnvelope {
        let seq: Int?
        let eventType: String
        let payload: WorkerPayload
    }

    struct WorkerPayload {
        let source: String?
        let method: String?
        let handshakeID: String?
        let deviceID: String?
        let desktopSessionID: String?
        let occurredAt: String?
    }

    enum HandshakeKind: Equatable {
        case iosHandshake
        case desktopHandshakeAck
    }

    struct HandshakeEnvelope: Equatable {
        let kind: HandshakeKind
        let handshakeID: String
    }

    static func decodeWorkerEvent(from payload: JSONValue) -> WorkerEventEnvelope? {
        guard let object = payload.objectValue else {
            return nil
        }

        let eventType = normalizedString(
            object["eventType"]?.stringValue ?? object["event_type"]?.stringValue
        )

        guard let eventType else {
            return nil
        }

        guard let workerPayload = object["payload"]?.objectValue else {
            return nil
        }

        let payloadEnvelope = WorkerPayload(
            source: normalizedString(workerPayload["source"]?.stringValue),
            method: normalizedString(workerPayload["method"]?.stringValue),
            handshakeID: normalizedString(
                workerPayload["handshake_id"]?.stringValue ?? workerPayload["handshakeId"]?.stringValue
            ),
            deviceID: normalizedString(workerPayload["device_id"]?.stringValue),
            desktopSessionID: normalizedString(workerPayload["desktop_session_id"]?.stringValue),
            occurredAt: normalizedString(workerPayload["occurred_at"]?.stringValue)
        )

        return WorkerEventEnvelope(
            seq: object["seq"]?.intValue ?? object["latest_seq"]?.intValue,
            eventType: eventType,
            payload: payloadEnvelope
        )
    }

    static func decodeHandshakeEnvelope(from payload: JSONValue) -> HandshakeEnvelope? {
        guard let event = decodeWorkerEvent(from: payload),
              event.eventType == workerEventType else {
            return nil
        }

        guard let source = event.payload.source,
              let method = event.payload.method,
              let handshakeID = event.payload.handshakeID else {
            return nil
        }

        if source == iosSource && method == iosHandshakeMethod {
            guard normalizedString(event.payload.deviceID) != nil,
                  normalizedString(event.payload.occurredAt) != nil else {
                return nil
            }

            return HandshakeEnvelope(kind: .iosHandshake, handshakeID: handshakeID)
        }

        if source == desktopSource && method == desktopHandshakeAckMethod {
            guard normalizedString(event.payload.desktopSessionID) != nil,
                  normalizedString(event.payload.occurredAt) != nil else {
                return nil
            }

            return HandshakeEnvelope(kind: .desktopHandshakeAck, handshakeID: handshakeID)
        }

        return nil
    }

    private static func normalizedString(_ raw: String?) -> String? {
        guard let raw else {
            return nil
        }

        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
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
