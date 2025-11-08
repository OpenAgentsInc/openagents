#if os(macOS)
import Foundation

/// Actor responsible for persisting ACP session updates to Tinyvex and broadcasting to all connected clients.
/// This is the single canonical path for all outbound session updates.
///
/// Responsibilities:
/// - Persist updates to Tinyvex database
/// - Broadcast updates to all connected clients via callback
/// - Track metrics (update count, last append timestamp, broadcast count)
///
/// Thread-safe via actor isolation.
public actor SessionUpdateHub {
    // MARK: - Dependencies

    /// Tinyvex database layer for persistence
    private let tinyvexDb: TinyvexDbLayer?

    /// Callback to broadcast JSON-RPC notification to all clients
    /// Parameters: (notificationJSON: String)
    private let broadcastCallback: @Sendable (String) async -> Void

    // MARK: - Metrics

    /// Total number of updates processed
    private(set) var updateCount: Int = 0

    /// Total number of updates successfully persisted to Tinyvex
    private(set) var persistedCount: Int = 0

    /// Total number of updates broadcast to clients
    private(set) var broadcastCount: Int = 0

    /// Timestamp of last successful append to Tinyvex (milliseconds since epoch)
    private(set) var lastAppendTimestamp: Int64?

    /// Queue depth (currently always 0 since we process synchronously)
    private(set) var queueDepth: Int = 0

    // MARK: - Initialization

    /// Initialize the SessionUpdateHub
    /// - Parameters:
    ///   - tinyvexDb: Optional Tinyvex database for persistence
    ///   - broadcastCallback: Async closure to broadcast JSON-RPC notification text to all clients
    public init(
        tinyvexDb: TinyvexDbLayer?,
        broadcastCallback: @escaping @Sendable (String) async -> Void
    ) {
        self.tinyvexDb = tinyvexDb
        self.broadcastCallback = broadcastCallback
    }

    // MARK: - Public API

    /// Send a session update: persist to Tinyvex and broadcast to all clients.
    /// This is the single canonical path for all outbound ACP session updates.
    ///
    /// - Parameters:
    ///   - sessionId: The ACP session identifier
    ///   - update: The session update to send
    public func sendSessionUpdate(
        sessionId: ACPSessionId,
        update: ACP.Client.SessionUpdate
    ) async {
        updateCount += 1

        // 1. Persist to Tinyvex if configured
        let kind = updateKind(update)
        if let db = tinyvexDb,
           let updJSON = encodeUpdate(update) {
            do {
                // Seq=0 placeholder for now; ts = now
                let timestamp = Int64(Date().timeIntervalSince1970 * 1000)
                try await db.appendEvent(
                    sessionId: sessionId.value,
                    seq: 0,
                    ts: timestamp,
                    updateJSON: updJSON
                )
                persistedCount += 1
                lastAppendTimestamp = timestamp
                OpenAgentsLog.server.debug("tinyvex append session=\(sessionId.value) kind=\(kind)")
            } catch {
                OpenAgentsLog.server.error("tinyvex append error session=\(sessionId.value) kind=\(kind): \(error)")
            }
        }

        // 2. Build JSON-RPC notification
        let notification = ACP.Client.SessionNotificationWire(
            session_id: sessionId,
            update: update,
            _meta: nil
        )

        guard let notificationJSON = encodeNotification(notification) else {
            OpenAgentsLog.server.error("SessionUpdateHub Failed to encode session update for session=\(sessionId.value)")
            return
        }

        // 3. Broadcast to all connected clients
        await broadcastCallback(notificationJSON)
        broadcastCount += 1
    }

    /// Get current metrics snapshot
    public func getMetrics() -> Metrics {
        Metrics(
            updateCount: updateCount,
            persistedCount: persistedCount,
            broadcastCount: broadcastCount,
            lastAppendTimestamp: lastAppendTimestamp,
            queueDepth: queueDepth
        )
    }

    /// Reset metrics counters (primarily for testing)
    public func resetMetrics() {
        updateCount = 0
        persistedCount = 0
        broadcastCount = 0
        lastAppendTimestamp = nil
        queueDepth = 0
    }

    // MARK: - Private Helpers

    /// Determine the update kind for logging
    private func updateKind(_ update: ACP.Client.SessionUpdate) -> String {
        switch update {
        case .userMessageChunk: return "user_message_chunk"
        case .agentMessageChunk: return "agent_message_chunk"
        case .agentThoughtChunk: return "agent_thought_chunk"
        case .toolCall: return "tool_call"
        case .toolCallUpdate: return "tool_call_update"
        case .plan: return "plan"
        case .availableCommandsUpdate: return "available_commands_update"
        case .currentModeUpdate: return "current_mode_update"
        }
    }

    /// Encode update to JSON string
    private func encodeUpdate(_ update: ACP.Client.SessionUpdate) -> String? {
        guard let data = try? JSONEncoder().encode(update),
              let json = String(data: data, encoding: .utf8) else {
            return nil
        }
        return json
    }

    /// Encode notification to JSON-RPC text
    private func encodeNotification(_ notification: ACP.Client.SessionNotificationWire) -> String? {
        let jsonrpcNotification = JSONRPC.Notification(
            method: ACPRPC.sessionUpdate,
            params: notification
        )
        guard let data = try? JSONEncoder().encode(jsonrpcNotification),
              let text = String(data: data, encoding: .utf8) else {
            return nil
        }
        return text
    }
}

// MARK: - Metrics Type

extension SessionUpdateHub {
    /// Metrics snapshot from the SessionUpdateHub
    public struct Metrics: Sendable {
        public let updateCount: Int
        public let persistedCount: Int
        public let broadcastCount: Int
        public let lastAppendTimestamp: Int64?
        public let queueDepth: Int
    }
}
#endif
