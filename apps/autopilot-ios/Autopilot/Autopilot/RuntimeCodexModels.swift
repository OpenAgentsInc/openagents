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
    let heartbeatState: String?
    let lastHeartbeatAt: String?
    let startedAt: String?
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
        case heartbeatState = "heartbeat_state"
        case lastHeartbeatAt = "last_heartbeat_at"
        case startedAt = "started_at"
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
        heartbeatState = try container.decodeIfPresent(String.self, forKey: .heartbeatState)
        lastHeartbeatAt = try container.decodeIfPresent(String.self, forKey: .lastHeartbeatAt)
        startedAt = try container.decodeIfPresent(String.self, forKey: .startedAt)
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

enum RuntimeCodexControlMethod: String, Codable, Equatable {
    case threadStart = "thread/start"
    case threadResume = "thread/resume"
    case turnStart = "turn/start"
    case turnInterrupt = "turn/interrupt"
    case threadList = "thread/list"
    case threadRead = "thread/read"
}

struct RuntimeCodexWorkerActionRequest: Codable, Equatable {
    let requestID: String
    let method: RuntimeCodexControlMethod
    let params: [String: JSONValue]
    let requestVersion: String?
    let sentAt: String?
    let source: String?
    let sessionID: String?
    let threadID: String?

    init(
        requestID: String,
        method: RuntimeCodexControlMethod,
        params: [String: JSONValue] = [:],
        requestVersion: String? = "v1",
        sentAt: String? = nil,
        source: String? = "autopilot-ios",
        sessionID: String? = nil,
        threadID: String? = nil
    ) {
        self.requestID = requestID
        self.method = method
        self.params = params
        self.requestVersion = requestVersion
        self.sentAt = sentAt
        self.source = source
        self.sessionID = sessionID
        self.threadID = threadID
    }

    enum CodingKeys: String, CodingKey {
        case requestID = "request_id"
        case method
        case params
        case requestVersion = "request_version"
        case sentAt = "sent_at"
        case source
        case sessionID = "session_id"
        case threadID = "thread_id"
    }
}

struct RuntimeCodexWorkerActionResult: Decodable {
    let workerID: String?
    let requestID: String?
    let ok: Bool?
    let method: String?
    let response: JSONValue?
    let status: String?
    let projection: RuntimeCodexProjectionStatus?

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case requestID = "request_id"
        case ok
        case method
        case response
        case status
        case projection
    }
}

struct RuntimeCodexWorkerStopResult: Decodable {
    let workerID: String?
    let status: String?
    let idempotentReplay: Bool?
    let projection: RuntimeCodexProjectionStatus?

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case status
        case idempotentReplay = "idempotent_replay"
        case projection
    }
}

enum RuntimeCodexControlRequestState: String, Codable, Equatable {
    case queued
    case running
    case success
    case error
}

struct RuntimeCodexControlReceipt: Codable, Equatable {
    enum Outcome: Codable, Equatable {
        case success(response: JSONValue?)
        case error(code: String, message: String, retryable: Bool, details: JSONValue?)

        private enum CodingKeys: String, CodingKey {
            case kind
            case response
            case code
            case message
            case retryable
            case details
        }

        private enum Kind: String, Codable {
            case success
            case error
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            let kind = try container.decode(Kind.self, forKey: .kind)
            switch kind {
            case .success:
                let response = try container.decodeIfPresent(JSONValue.self, forKey: .response)
                self = .success(response: response)
            case .error:
                self = .error(
                    code: try container.decode(String.self, forKey: .code),
                    message: try container.decode(String.self, forKey: .message),
                    retryable: try container.decode(Bool.self, forKey: .retryable),
                    details: try container.decodeIfPresent(JSONValue.self, forKey: .details)
                )
            }
        }

        func encode(to encoder: Encoder) throws {
            var container = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .success(let response):
                try container.encode(Kind.success, forKey: .kind)
                try container.encodeIfPresent(response, forKey: .response)
            case .error(let code, let message, let retryable, let details):
                try container.encode(Kind.error, forKey: .kind)
                try container.encode(code, forKey: .code)
                try container.encode(message, forKey: .message)
                try container.encode(retryable, forKey: .retryable)
                try container.encodeIfPresent(details, forKey: .details)
            }
        }
    }

    let requestID: String
    let method: String
    let occurredAt: String?
    let outcome: Outcome

    enum CodingKeys: String, CodingKey {
        case requestID = "request_id"
        case method
        case occurredAt = "occurred_at"
        case outcome
    }
}

struct RuntimeCodexControlRequestTracker: Codable, Identifiable, Equatable {
    let workerID: String
    let request: RuntimeCodexWorkerActionRequest
    let createdAt: String
    var lastUpdatedAt: String
    var state: RuntimeCodexControlRequestState
    var sentAt: String?
    var receiptAt: String?
    var errorCode: String?
    var errorMessage: String?
    var retryable: Bool
    var response: JSONValue?

    var id: String { request.requestID }
    var requestID: String { request.requestID }

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case request
        case createdAt = "created_at"
        case lastUpdatedAt = "last_updated_at"
        case state
        case sentAt = "sent_at"
        case receiptAt = "receipt_at"
        case errorCode = "error_code"
        case errorMessage = "error_message"
        case retryable
        case response
    }
}

final class RuntimeCodexControlCoordinator {
    private struct Command: Encodable {
        let op: String
        let workerID: String?
        let request: RuntimeCodexWorkerActionRequest?
        let requestID: String?
        let message: String?
        let code: String?
        let retryable: Bool?
        let occurredAt: String?
        let receipt: RuntimeCodexControlReceipt?

        enum CodingKeys: String, CodingKey {
            case op
            case workerID = "worker_id"
            case request
            case requestID = "request_id"
            case message
            case code
            case retryable
            case occurredAt = "occurred_at"
            case receipt
        }
    }

    private struct ApplyResult: Decodable {
        let tracker: RuntimeCodexControlRequestTracker?
        let snapshots: [RuntimeCodexControlRequestTracker]
        let queued: [RuntimeCodexControlRequestTracker]
    }

    private let rawCoordinator: UnsafeMutableRawPointer?
    private var snapshotsCache: [RuntimeCodexControlRequestTracker] = []
    private var queuedCache: [RuntimeCodexControlRequestTracker] = []

    init() {
        rawCoordinator = RustClientCoreBridge.createControlCoordinator()
    }

    deinit {
        RustClientCoreBridge.freeControlCoordinator(rawCoordinator)
    }

    var snapshots: [RuntimeCodexControlRequestTracker] {
        snapshotsCache
    }

    var queuedRequests: [RuntimeCodexControlRequestTracker] {
        queuedCache.filter { $0.state == .queued }
    }

    @discardableResult
    func enqueue(
        workerID: String,
        request: RuntimeCodexWorkerActionRequest,
        occurredAt: String
    ) -> RuntimeCodexControlRequestTracker {
        let command = Command(
            op: "enqueue",
            workerID: workerID,
            request: request,
            requestID: nil,
            message: nil,
            code: nil,
            retryable: nil,
            occurredAt: occurredAt,
            receipt: nil
        )
        if let result = apply(command), let tracker = result.tracker {
            return tracker
        }

        let fallback = RuntimeCodexControlRequestTracker(
            workerID: workerID,
            request: request,
            createdAt: occurredAt,
            lastUpdatedAt: occurredAt,
            state: .queued,
            sentAt: nil,
            receiptAt: nil,
            errorCode: nil,
            errorMessage: nil,
            retryable: false,
            response: nil
        )
        snapshotsCache.append(fallback)
        queuedCache.append(fallback)
        return fallback
    }

    func markRunning(
        requestID: String,
        occurredAt: String
    ) -> RuntimeCodexControlRequestTracker? {
        apply(
            Command(
                op: "mark_running",
                workerID: nil,
                request: nil,
                requestID: requestID,
                message: nil,
                code: nil,
                retryable: nil,
                occurredAt: occurredAt,
                receipt: nil
            )
        )?.tracker
    }

    func requeue(
        requestID: String,
        message: String?,
        occurredAt: String
    ) -> RuntimeCodexControlRequestTracker? {
        apply(
            Command(
                op: "requeue",
                workerID: nil,
                request: nil,
                requestID: requestID,
                message: message,
                code: nil,
                retryable: nil,
                occurredAt: occurredAt,
                receipt: nil
            )
        )?.tracker
    }

    func markDispatchError(
        requestID: String,
        code: String,
        message: String,
        retryable: Bool,
        occurredAt: String
    ) -> RuntimeCodexControlRequestTracker? {
        apply(
            Command(
                op: "mark_dispatch_error",
                workerID: nil,
                request: nil,
                requestID: requestID,
                message: message,
                code: code,
                retryable: retryable,
                occurredAt: occurredAt,
                receipt: nil
            )
        )?.tracker
    }

    func markTimeout(
        requestID: String,
        occurredAt: String
    ) -> RuntimeCodexControlRequestTracker? {
        apply(
            Command(
                op: "mark_timeout",
                workerID: nil,
                request: nil,
                requestID: requestID,
                message: nil,
                code: nil,
                retryable: nil,
                occurredAt: occurredAt,
                receipt: nil
            )
        )?.tracker
    }

    func reconcile(
        workerID: String,
        receipt: RuntimeCodexControlReceipt
    ) -> RuntimeCodexControlRequestTracker? {
        apply(
            Command(
                op: "reconcile",
                workerID: workerID,
                request: nil,
                requestID: nil,
                message: nil,
                code: nil,
                retryable: nil,
                occurredAt: nil,
                receipt: receipt
            )
        )?.tracker
    }

    private func apply(_ command: Command) -> ApplyResult? {
        guard let rawCoordinator,
              let encoded = try? JSONEncoder().encode(command),
              let commandJSON = String(data: encoded, encoding: .utf8),
              let responseJSON = RustClientCoreBridge.applyControlCoordinator(
                  rawCoordinator,
                  commandJSON: commandJSON
              ),
              let responseData = responseJSON.data(using: .utf8),
              let result = try? JSONDecoder().decode(ApplyResult.self, from: responseData) else {
            return nil
        }

        snapshotsCache = result.snapshots
        queuedCache = result.queued
        return result
    }
}

struct RuntimeMissionControlWorkerState: Decodable, Equatable {
    let workerID: String
    let status: String
    let heartbeatState: String?
    let latestSeq: Int?
    let lagEvents: Int?
    let reconnectState: String?
    let lastEventAt: String?
    let runningTurns: Int
    let queuedRequests: Int
    let failedRequests: Int

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case status
        case heartbeatState = "heartbeat_state"
        case latestSeq = "latest_seq"
        case lagEvents = "lag_events"
        case reconnectState = "reconnect_state"
        case lastEventAt = "last_event_at"
        case runningTurns = "running_turns"
        case queuedRequests = "queued_requests"
        case failedRequests = "failed_requests"
    }
}

struct RuntimeMissionControlThreadState: Decodable, Equatable {
    let workerID: String
    let threadID: String
    let activeTurnID: String?
    let lastSummary: String
    let lastEventAt: String?
    let freshnessSeq: Int?
    let unreadCount: Int
    let muted: Bool

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case threadID = "thread_id"
        case activeTurnID = "active_turn_id"
        case lastSummary = "last_summary"
        case lastEventAt = "last_event_at"
        case freshnessSeq = "freshness_seq"
        case unreadCount = "unread_count"
        case muted
    }
}

struct RuntimeMissionControlTimelineItem: Decodable, Equatable, Identifiable {
    let id: String
    let role: String
    let text: String
    let isStreaming: Bool
    let workerID: String
    let threadID: String
    let turnID: String?
    let itemID: String?
    let occurredAt: String?

    enum CodingKeys: String, CodingKey {
        case id
        case role
        case text
        case isStreaming = "is_streaming"
        case workerID = "worker_id"
        case threadID = "thread_id"
        case turnID = "turn_id"
        case itemID = "item_id"
        case occurredAt = "occurred_at"
    }
}

struct RuntimeMissionControlThreadTimeline: Decodable, Equatable {
    let workerID: String
    let threadID: String
    let entries: [RuntimeMissionControlTimelineItem]

    enum CodingKeys: String, CodingKey {
        case workerID = "worker_id"
        case threadID = "thread_id"
        case entries
    }
}

enum RuntimeMissionControlEventSeverity: String, Decodable, Equatable {
    case info
    case warning
    case error
}

struct RuntimeMissionControlEventRecord: Decodable, Equatable, Identifiable {
    let id: Int
    let topic: String
    let seq: Int?
    let workerID: String?
    let threadID: String?
    let turnID: String?
    let requestID: String?
    let eventType: String?
    let method: String?
    let summary: String
    let severity: RuntimeMissionControlEventSeverity
    let occurredAt: String?
    let payload: JSONValue
    let resyncMarker: Bool

    enum CodingKeys: String, CodingKey {
        case id
        case topic
        case seq
        case workerID = "worker_id"
        case threadID = "thread_id"
        case turnID = "turn_id"
        case requestID = "request_id"
        case eventType = "event_type"
        case method
        case summary
        case severity
        case occurredAt = "occurred_at"
        case payload
        case resyncMarker = "resync_marker"
    }
}

struct RuntimeMissionControlRequestState: Codable, Equatable, Identifiable {
    let requestID: String
    let workerID: String
    let threadID: String?
    let method: String
    let state: String
    let occurredAt: String?
    let errorCode: String?
    let errorMessage: String?
    let retryable: Bool
    let response: JSONValue?

    var id: String { requestID }

    enum CodingKeys: String, CodingKey {
        case requestID = "request_id"
        case workerID = "worker_id"
        case threadID = "thread_id"
        case method
        case state
        case occurredAt = "occurred_at"
        case errorCode = "error_code"
        case errorMessage = "error_message"
        case retryable
        case response
    }
}

struct RuntimeMissionControlProjection: Decodable, Equatable {
    let workers: [RuntimeMissionControlWorkerState]
    let threads: [RuntimeMissionControlThreadState]
    let timelines: [RuntimeMissionControlThreadTimeline]
    let events: [RuntimeMissionControlEventRecord]
    let requests: [RuntimeMissionControlRequestState]
    let activeWorkerID: String?
    let activeThreadID: String?
    let activeTurnID: String?
    let compatibilityChatMessages: [RuntimeMissionControlTimelineItem]

    enum CodingKeys: String, CodingKey {
        case workers
        case threads
        case timelines
        case events
        case requests
        case activeWorkerID = "active_worker_id"
        case activeThreadID = "active_thread_id"
        case activeTurnID = "active_turn_id"
        case compatibilityChatMessages = "compatibility_chat_messages"
    }

    static let empty = RuntimeMissionControlProjection(
        workers: [],
        threads: [],
        timelines: [],
        events: [],
        requests: [],
        activeWorkerID: nil,
        activeThreadID: nil,
        activeTurnID: nil,
        compatibilityChatMessages: []
    )
}

final class RuntimeMissionControlStore {
    private struct Command: Encodable {
        let op: String
        let topic: String?
        let seq: Int?
        let workerID: String?
        let payload: JSONValue?
        let status: String?
        let heartbeatState: String?
        let latestSeq: Int?
        let lagEvents: Int?
        let reconnectState: String?
        let occurredAt: String?
        let request: RuntimeMissionControlRequestState?
        let threadID: String?
        let muted: Bool?
        let fromSeq: Int?
        let toSeq: Int?
        let maxEvents: Int?
        let maxTimelineEntries: Int?

        enum CodingKeys: String, CodingKey {
            case op
            case topic
            case seq
            case workerID = "worker_id"
            case payload
            case status
            case heartbeatState = "heartbeat_state"
            case latestSeq = "latest_seq"
            case lagEvents = "lag_events"
            case reconnectState = "reconnect_state"
            case occurredAt = "occurred_at"
            case request
            case threadID = "thread_id"
            case muted
            case fromSeq = "from_seq"
            case toSeq = "to_seq"
            case maxEvents = "max_events"
            case maxTimelineEntries = "max_timeline_entries"
        }
    }

    private let rawStore: UnsafeMutableRawPointer?
    private var projectionCache: RuntimeMissionControlProjection = .empty

    init() {
        rawStore = RustClientCoreBridge.createMissionControlStore()
    }

    deinit {
        RustClientCoreBridge.freeMissionControlStore(rawStore)
    }

    var projection: RuntimeMissionControlProjection {
        projectionCache
    }

    @discardableResult
    func configure(maxEvents: Int?, maxTimelineEntries: Int?) -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "configure",
                topic: nil,
                seq: nil,
                workerID: nil,
                payload: nil,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: nil,
                threadID: nil,
                muted: nil,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: maxEvents,
                maxTimelineEntries: maxTimelineEntries
            )
        )
    }

    @discardableResult
    func reset() -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "reset",
                topic: nil,
                seq: nil,
                workerID: nil,
                payload: nil,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: nil,
                threadID: nil,
                muted: nil,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    func ingestWorkerSummary(
        workerID: String,
        status: String,
        heartbeatState: String?,
        latestSeq: Int?,
        lagEvents: Int?,
        reconnectState: String?,
        occurredAt: String? = nil
    ) -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "ingest_worker_summary",
                topic: nil,
                seq: nil,
                workerID: workerID,
                payload: nil,
                status: status,
                heartbeatState: heartbeatState,
                latestSeq: latestSeq,
                lagEvents: lagEvents,
                reconnectState: reconnectState,
                occurredAt: occurredAt,
                request: nil,
                threadID: nil,
                muted: nil,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    func ingestWorkerSummary(
        _ worker: RuntimeCodexWorkerSummary,
        reconnectState: String?,
        occurredAt: String? = nil
    ) -> RuntimeMissionControlProjection {
        ingestWorkerSummary(
            workerID: worker.workerID,
            status: worker.status,
            heartbeatState: worker.heartbeatState,
            latestSeq: worker.latestSeq,
            lagEvents: worker.khalaProjection?.lagEvents,
            reconnectState: reconnectState,
            occurredAt: occurredAt
        )
    }

    @discardableResult
    func ingestStreamEvent(
        topic: String,
        seq: Int?,
        workerID: String?,
        payload: JSONValue
    ) -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "ingest_stream_event",
                topic: topic,
                seq: seq,
                workerID: workerID,
                payload: payload,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: nil,
                threadID: nil,
                muted: nil,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    func upsertRequest(_ tracker: RuntimeCodexControlRequestTracker) -> RuntimeMissionControlProjection {
        let request = RuntimeMissionControlRequestState(
            requestID: tracker.requestID,
            workerID: tracker.workerID,
            threadID: tracker.request.threadID,
            method: tracker.request.method.rawValue,
            state: tracker.state.rawValue,
            occurredAt: tracker.lastUpdatedAt,
            errorCode: tracker.errorCode,
            errorMessage: tracker.errorMessage,
            retryable: tracker.retryable,
            response: tracker.response
        )

        return apply(
            Command(
                op: "upsert_request",
                topic: nil,
                seq: nil,
                workerID: nil,
                payload: nil,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: request,
                threadID: nil,
                muted: nil,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    func setActiveLane(workerID: String?, threadID: String?) -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "set_active_lane",
                topic: nil,
                seq: nil,
                workerID: workerID,
                payload: nil,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: nil,
                threadID: threadID,
                muted: nil,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    func setLaneMuted(workerID: String, threadID: String, muted: Bool) -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "set_lane_muted",
                topic: nil,
                seq: nil,
                workerID: workerID,
                payload: nil,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: nil,
                threadID: threadID,
                muted: muted,
                fromSeq: nil,
                toSeq: nil,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    func markResynced(topic: String, fromSeq: Int, toSeq: Int, workerID: String?) -> RuntimeMissionControlProjection {
        apply(
            Command(
                op: "mark_resynced",
                topic: topic,
                seq: nil,
                workerID: workerID,
                payload: nil,
                status: nil,
                heartbeatState: nil,
                latestSeq: nil,
                lagEvents: nil,
                reconnectState: nil,
                occurredAt: nil,
                request: nil,
                threadID: nil,
                muted: nil,
                fromSeq: fromSeq,
                toSeq: toSeq,
                maxEvents: nil,
                maxTimelineEntries: nil
            )
        )
    }

    @discardableResult
    private func apply(_ command: Command) -> RuntimeMissionControlProjection {
        guard let rawStore,
              let commandData = try? JSONEncoder().encode(command),
              let commandJSON = String(data: commandData, encoding: .utf8),
              let responseJSON = RustClientCoreBridge.applyMissionControlStore(
                  rawStore,
                  commandJSON: commandJSON
              ),
              let responseData = responseJSON.data(using: .utf8),
              let projection = try? JSONDecoder().decode(
                  RuntimeMissionControlProjection.self,
                  from: responseData
              ) else {
            return projectionCache
        }

        projectionCache = projection
        return projection
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
        let workerID: String?
        let source: String?
        let method: String?
        let handshakeID: String?
        let deviceID: String?
        let desktopSessionID: String?
        let params: [String: JSONValue]?
        let occurredAt: String?
    }

    struct CodexEventEnvelope {
        let seq: Int?
        let workerID: String?
        let source: String?
        let method: String
        let params: [String: JSONValue]
        let threadID: String?
        let turnID: String?
        let itemID: String?
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

    struct ControlReceiptEnvelope: Codable, Equatable {
        let eventType: String
        let receipt: RuntimeCodexControlReceipt

        enum CodingKeys: String, CodingKey {
            case eventType = "event_type"
            case receipt
        }
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
            workerID: normalizedString(
                object["workerId"]?.stringValue
                    ?? object["worker_id"]?.stringValue
                    ?? workerPayload["workerId"]?.stringValue
                    ?? workerPayload["worker_id"]?.stringValue
            ),
            source: normalizedString(workerPayload["source"]?.stringValue),
            method: normalizedString(workerPayload["method"]?.stringValue),
            handshakeID: normalizedString(
                workerPayload["handshake_id"]?.stringValue ?? workerPayload["handshakeId"]?.stringValue
            ),
            deviceID: normalizedString(workerPayload["device_id"]?.stringValue),
            desktopSessionID: normalizedString(workerPayload["desktop_session_id"]?.stringValue),
            params: workerPayload["params"]?.objectValue,
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

    static func decodeControlReceipt(from payload: JSONValue) -> ControlReceiptEnvelope? {
        guard let payloadData = try? JSONEncoder().encode(payload),
              let payloadJSON = String(data: payloadData, encoding: .utf8),
              let receiptJSON = RustClientCoreBridge.decodeControlReceipt(payloadJSON: payloadJSON),
              let receiptData = receiptJSON.data(using: .utf8),
              let envelope = try? JSONDecoder().decode(ControlReceiptEnvelope.self, from: receiptData) else {
            return nil
        }
        return envelope
    }

    static func decodeCodexEventEnvelope(from payload: JSONValue) -> CodexEventEnvelope? {
        guard let event = decodeWorkerEvent(from: payload),
              event.eventType.hasPrefix("worker."),
              let method = event.payload.method else {
            return nil
        }

        let params = event.payload.params ?? [:]
        let threadID = extractThreadID(from: params)
        let turnID = extractTurnID(from: params)
        let itemID = extractItemID(from: params)

        return CodexEventEnvelope(
            seq: event.seq,
            workerID: event.payload.workerID,
            source: event.payload.source,
            method: method,
            params: params,
            threadID: threadID,
            turnID: turnID,
            itemID: itemID,
            occurredAt: event.payload.occurredAt
        )
    }

    private static func normalizedString(_ raw: String?) -> String? {
        guard let raw else {
            return nil
        }

        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private static func extractThreadID(from params: [String: JSONValue]) -> String? {
        if let id = normalizedString(params["threadId"]?.stringValue ?? params["thread_id"]?.stringValue) {
            return id
        }

        if let id = normalizedString(params["conversationId"]?.stringValue ?? params["conversation_id"]?.stringValue) {
            return id
        }

        if let thread = params["thread"]?.objectValue,
           let id = normalizedString(thread["id"]?.stringValue) {
            return id
        }

        if let msg = params["msg"]?.objectValue,
           let id = normalizedString(
               msg["thread_id"]?.stringValue
                   ?? msg["threadId"]?.stringValue
                   ?? msg["conversation_id"]?.stringValue
                   ?? msg["conversationId"]?.stringValue
           ) {
            return id
        }

        return nil
    }

    private static func extractTurnID(from params: [String: JSONValue]) -> String? {
        if let id = normalizedString(params["turnId"]?.stringValue ?? params["turn_id"]?.stringValue) {
            return id
        }

        if let turn = params["turn"]?.objectValue,
           let id = normalizedString(turn["id"]?.stringValue) {
            return id
        }

        if let msg = params["msg"]?.objectValue,
           let id = normalizedString(msg["turn_id"]?.stringValue ?? msg["turnId"]?.stringValue) {
            return id
        }

        // Legacy codex/event/user_message uses top-level `params.id` for turn id.
        if let id = normalizedString(params["id"]?.stringValue) {
            return id
        }

        return nil
    }

    private static func extractItemID(from params: [String: JSONValue]) -> String? {
        if let id = normalizedString(params["itemId"]?.stringValue ?? params["item_id"]?.stringValue) {
            return id
        }

        if let item = params["item"]?.objectValue,
           let id = normalizedString(item["id"]?.stringValue) {
            return id
        }

        if let msg = params["msg"]?.objectValue {
            if let id = normalizedString(msg["item_id"]?.stringValue ?? msg["itemId"]?.stringValue) {
                return id
            }

            if let item = msg["item"]?.objectValue,
               let id = normalizedString(item["id"]?.stringValue) {
                return id
            }
        }

        return nil
    }
}

struct RuntimeCodexAuthSession: Equatable {
    let userID: String?
    let email: String?
    let tokenType: String?
    let token: String
    let refreshToken: String?
    let sessionID: String?
    let accessExpiresAt: String?
    let refreshExpiresAt: String?
}

struct RuntimeCodexAuthChallenge: Equatable {
    let challengeID: String
    let email: String
}

struct RuntimeCodexSessionSnapshot: Equatable {
    let sessionID: String
    let userID: String
    let deviceID: String
    let status: String
    let reauthRequired: Bool
    let activeOrgID: String?
}

struct CodexAuthVerifyContext: Equatable {
    let generation: UInt64
    let challengeID: String
    let email: String
}

struct CodexAuthFlowState: Equatable {
    private(set) var generation: UInt64 = 0
    private(set) var pendingEmail: String?
    private(set) var pendingChallengeID: String?

    mutating func beginSend(email: String) -> UInt64 {
        generation &+= 1
        pendingEmail = email
        pendingChallengeID = nil
        return generation
    }

    mutating func resolveSend(generation: UInt64, challengeID: String) -> Bool {
        guard generation == self.generation else {
            return false
        }
        pendingChallengeID = challengeID
        return true
    }

    func beginVerify() -> CodexAuthVerifyContext? {
        guard let email = pendingEmail,
              let challengeID = pendingChallengeID else {
            return nil
        }
        return CodexAuthVerifyContext(generation: generation, challengeID: challengeID, email: email)
    }

    func shouldAcceptResponse(generation: UInt64) -> Bool {
        generation == self.generation
    }

    mutating func invalidate() {
        generation &+= 1
        pendingEmail = nil
        pendingChallengeID = nil
    }
}

struct CodexLifecycleResumeState: Equatable {
    private(set) var generation: UInt64 = 0
    private(set) var suspendedAtGeneration: UInt64?

    mutating func markBackground() -> UInt64 {
        generation &+= 1
        suspendedAtGeneration = generation
        return generation
    }

    mutating func beginForegroundResume() -> UInt64? {
        guard suspendedAtGeneration != nil else {
            return nil
        }
        generation &+= 1
        suspendedAtGeneration = nil
        return generation
    }

    func shouldAccept(generation: UInt64) -> Bool {
        self.generation == generation
    }

    mutating func invalidate() {
        generation &+= 1
        suspendedAtGeneration = nil
    }
}

struct CodexResumeCheckpoint: Codable, Equatable {
    let namespace: String
    let workerID: String
    var topicWatermarks: [String: Int]
    var sessionID: String?
    var updatedAt: String
}

struct CodexResumeCheckpointStore: Codable, Equatable {
    private(set) var checkpoints: [String: CodexResumeCheckpoint] = [:]

    func watermark(namespace: String, workerID: String, topic: String) -> Int {
        guard let checkpoint = checkpoints[checkpointKey(namespace: namespace, workerID: workerID)] else {
            return 0
        }
        return max(0, checkpoint.topicWatermarks[topic] ?? 0)
    }

    func maxWatermark(namespace: String, topic: String) -> Int {
        checkpoints.values
            .filter { $0.namespace == namespace }
            .map { max(0, $0.topicWatermarks[topic] ?? 0) }
            .max() ?? 0
    }

    mutating func upsert(
        namespace: String,
        workerID: String,
        topic: String,
        watermark: Int,
        sessionID: String?,
        updatedAt: String
    ) {
        let key = checkpointKey(namespace: namespace, workerID: workerID)
        var checkpoint = checkpoints[key] ?? CodexResumeCheckpoint(
            namespace: namespace,
            workerID: workerID,
            topicWatermarks: [:],
            sessionID: nil,
            updatedAt: updatedAt
        )

        let normalizedWatermark = max(0, watermark)
        let existingWatermark = max(0, checkpoint.topicWatermarks[topic] ?? 0)
        checkpoint.topicWatermarks[topic] = max(existingWatermark, normalizedWatermark)

        if let sessionID = sessionID?.trimmingCharacters(in: .whitespacesAndNewlines),
           !sessionID.isEmpty {
            checkpoint.sessionID = sessionID
        }
        checkpoint.updatedAt = updatedAt
        checkpoints[key] = checkpoint
    }

    mutating func resetTopic(namespace: String, workerID: String, topic: String, updatedAt: String) {
        let key = checkpointKey(namespace: namespace, workerID: workerID)
        guard var checkpoint = checkpoints[key] else {
            return
        }

        checkpoint.topicWatermarks.removeValue(forKey: topic)
        checkpoint.updatedAt = updatedAt

        if checkpoint.topicWatermarks.isEmpty {
            checkpoints.removeValue(forKey: key)
        } else {
            checkpoints[key] = checkpoint
        }
    }

    mutating func removeNamespace(_ namespace: String) {
        checkpoints = checkpoints.filter { _, value in
            value.namespace != namespace
        }
    }

    private func checkpointKey(namespace: String, workerID: String) -> String {
        "\(namespace)|\(workerID)"
    }
}

struct RuntimeCodexSyncToken: Decodable, Equatable {
    let token: String
    let tokenType: String?
    let expiresIn: Int?
    let expiresAt: String?
    let issuer: String?
    let audience: String?
    let subject: String?
    let orgID: String?
    let scopes: [String]?

    enum CodingKeys: String, CodingKey {
        case token
        case tokenType = "token_type"
        case expiresIn = "expires_in"
        case expiresAt = "expires_at"
        case issuer
        case audience
        case subject
        case orgID = "org_id"
        case scopes
    }
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

enum KhalaLifecycleDisconnectReason: String, Equatable {
    case streamClosed = "stream_closed"
    case gatewayRestart = "gateway_restart"
    case staleCursor = "stale_cursor"
    case unauthorized = "unauthorized"
    case forbidden = "forbidden"
    case network = "network"
    case unknown = "unknown"
}

struct KhalaLifecycleSnapshot: Equatable {
    var connectAttempts: Int = 0
    var reconnectAttempts: Int = 0
    var successfulSessions: Int = 0
    var recoveredSessions: Int = 0
    var lastBackoffMs: Int = 0
    var lastRecoveryLatencyMs: Int = 0
    var lastDisconnectReason: KhalaLifecycleDisconnectReason?
}

struct KhalaReconnectPolicy: Equatable {
    let baseDelayMs: Int
    let maxDelayMs: Int
    let jitterRatio: Double

    static let `default` = KhalaReconnectPolicy(
        baseDelayMs: 250,
        maxDelayMs: 8_000,
        jitterRatio: 0.5
    )

    func delayMs(attempt: Int, jitterUnit: Double) -> Int {
        guard attempt > 0 else {
            return 0
        }

        // Cap exponent growth so reconnect cadence stays bounded under long outages.
        let exponent = min(attempt - 1, 10)
        let scaled = min(baseDelayMs * (1 << exponent), maxDelayMs)
        let clampedUnit = max(0.0, min(1.0, jitterUnit))
        let jitterMax = Int(Double(scaled) * jitterRatio)
        let jitter = Int(Double(jitterMax) * clampedUnit)
        return scaled + jitter
    }
}

enum KhalaReconnectClassifier {
    static func classify(_ error: Error) -> KhalaLifecycleDisconnectReason {
        if let runtimeError = error as? RuntimeCodexApiError {
            switch runtimeError.code {
            case .auth:
                return .unauthorized
            case .forbidden:
                return .forbidden
            case .conflict:
                return .staleCursor
            case .network:
                if runtimeError.message.localizedCaseInsensitiveContains("stream_closed")
                    || runtimeError.message.localizedCaseInsensitiveContains("reply_cancelled") {
                    return .streamClosed
                }
                return .network
            case .invalid, .unknown:
                return .unknown
            }
        }

        let nsError = error as NSError
        if nsError.domain == NSURLErrorDomain {
            switch nsError.code {
            case NSURLErrorNetworkConnectionLost, NSURLErrorTimedOut, NSURLErrorCannotConnectToHost:
                return .gatewayRestart
            default:
                return .network
            }
        }

        return .unknown
    }
}

enum CodexChatRole: String, Equatable {
    case user
    case assistant
    case reasoning
    case tool
    case system
    case error
}

enum CodexChatEventDisplayPolicy {
    static func shouldDisplaySystemMethod(_ method: String) -> Bool {
        method != "thread/started"
    }
}

enum CodexStreamingTextAssembler {
    static func append(existing: String, delta: String) -> String {
        guard !delta.isEmpty else {
            return existing
        }
        guard !existing.isEmpty else {
            return delta
        }

        if existing.hasSuffix(delta) {
            return existing
        }

        let overlap = overlapLength(existing: existing, delta: delta)
        guard overlap > 0 else {
            return existing + delta
        }

        return existing + delta.dropFirst(overlap)
    }

    private static func overlapLength(existing: String, delta: String) -> Int {
        let maxOverlap = min(existing.count, delta.count)
        guard maxOverlap > 0 else {
            return 0
        }

        for length in stride(from: maxOverlap, through: 1, by: -1) {
            if existing.hasSuffix(delta.prefix(length)) {
                return length
            }
        }

        return 0
    }
}

enum CodexAssistantDeltaSource: Equatable {
    case modern
    case legacyContent
}

struct CodexAssistantDeltaDecision: Equatable {
    let selectedSource: CodexAssistantDeltaSource
    let shouldAccept: Bool
    let shouldReset: Bool
}

enum CodexAssistantDeltaPolicy {
    static func decide(
        current: CodexAssistantDeltaSource?,
        incoming: CodexAssistantDeltaSource
    ) -> CodexAssistantDeltaDecision {
        guard let current else {
            return CodexAssistantDeltaDecision(
                selectedSource: incoming,
                shouldAccept: true,
                shouldReset: false
            )
        }

        switch (current, incoming) {
        case (.modern, .modern), (.legacyContent, .legacyContent):
            return CodexAssistantDeltaDecision(
                selectedSource: current,
                shouldAccept: true,
                shouldReset: false
            )

        case (.modern, .legacyContent):
            return CodexAssistantDeltaDecision(
                selectedSource: .legacyContent,
                shouldAccept: true,
                shouldReset: true
            )

        case (.legacyContent, .modern):
            return CodexAssistantDeltaDecision(
                selectedSource: .legacyContent,
                shouldAccept: false,
                shouldReset: false
            )
        }
    }
}

struct CodexChatMessage: Identifiable, Equatable {
    let id: String
    var role: CodexChatRole
    var text: String
    var isStreaming: Bool
    let threadID: String?
    let turnID: String?
    let itemID: String?
    let occurredAt: String?

    init(
        id: String = UUID().uuidString.lowercased(),
        role: CodexChatRole,
        text: String,
        isStreaming: Bool = false,
        threadID: String? = nil,
        turnID: String? = nil,
        itemID: String? = nil,
        occurredAt: String? = nil
    ) {
        self.id = id
        self.role = role
        self.text = text
        self.isStreaming = isStreaming
        self.threadID = threadID
        self.turnID = turnID
        self.itemID = itemID
        self.occurredAt = occurredAt
    }
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

    var arrayValue: [JSONValue]? {
        switch self {
        case .array(let value):
            return value
        default:
            return nil
        }
    }

    var boolValue: Bool? {
        switch self {
        case .bool(let value):
            return value
        case .string(let value):
            switch value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
            case "true":
                return true
            case "false":
                return false
            default:
                return nil
            }
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
