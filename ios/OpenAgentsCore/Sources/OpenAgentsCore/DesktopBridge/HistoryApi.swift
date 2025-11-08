#if os(macOS)
import Foundation

/// API for Tinyvex history operations.
/// Provides JSON-RPC methods for querying session history from the Tinyvex database.
///
/// Responsibilities:
/// - List recent sessions
/// - Load session timeline (all updates for a session)
/// - Read-only operations (no writes/mutations)
///
/// Thread-safe via actor isolation.
public actor HistoryApi {
    // MARK: - Dependencies

    /// Tinyvex database layer for persistence
    private let tinyvexDb: TinyvexDbLayer?

    // MARK: - Types

    /// Recent session item returned by recentSessions
    public struct SessionItem: Codable, Sendable {
        public let session_id: String
        public let last_ts: Int64
        public let message_count: Int64

        public init(session_id: String, last_ts: Int64, message_count: Int64) {
            self.session_id = session_id
            self.last_ts = last_ts
            self.message_count = message_count
        }
    }

    /// Parameters for sessionTimeline request
    public struct SessionTimelineParams: Codable, Sendable {
        public let session_id: String
        public let limit: Int?

        public init(session_id: String, limit: Int? = nil) {
            self.session_id = session_id
            self.limit = limit
        }
    }

    // MARK: - Initialization

    /// Initialize the HistoryApi
    /// - Parameter tinyvexDb: Optional Tinyvex database for reading history
    public init(tinyvexDb: TinyvexDbLayer?) {
        self.tinyvexDb = tinyvexDb
    }

    // MARK: - Public API

    /// Get the 10 most recent sessions from Tinyvex DB
    /// - Returns: Array of recent session items
    /// - Throws: HistoryError if database not attached or query fails
    public func recentSessions() async throws -> [SessionItem] {
        guard let db = tinyvexDb else {
            throw HistoryError.databaseNotAttached
        }

        do {
            let sessions = try await db.recentSessions(limit: 10)
            let items = sessions.map {
                SessionItem(
                    session_id: $0.session_id,
                    last_ts: $0.last_ts,
                    message_count: $0.message_count
                )
            }
            OpenAgentsLog.server.info("HistoryApi recentSessions count=\(items.count)")
            return items
        } catch {
            OpenAgentsLog.server.error("HistoryApi recentSessions error: \(error)")
            throw HistoryError.queryFailed(reason: "Failed to query recent sessions: \(error)")
        }
    }

    /// Load session timeline (all updates for a given session)
    /// - Parameters:
    ///   - sessionId: The session ID to load timeline for
    ///   - limit: Optional limit on number of updates to return
    /// - Returns: Array of session notification wires containing the updates
    /// - Throws: HistoryError if database not attached, parameters invalid, or query fails
    public func sessionTimeline(
        sessionId: String,
        limit: Int? = nil
    ) async throws -> [ACP.Client.SessionNotificationWire] {
        guard let db = tinyvexDb else {
            throw HistoryError.databaseNotAttached
        }

        do {
            let updateJsons = try await db.sessionTimeline(sessionId: sessionId, limit: limit)
            OpenAgentsLog.server.info("HistoryApi sessionTimeline session=\(sessionId) rows=\(updateJsons.count)")

            // Parse each JSON string into ACP.Client.SessionNotificationWire
            // Note: DB stores just the "update" portion, so we need to decode SessionUpdate and wrap it
            var updates: [ACP.Client.SessionNotificationWire] = []
            for (idx, jsonStr) in updateJsons.enumerated() {
                guard let data = jsonStr.data(using: .utf8) else {
                    OpenAgentsLog.server.error("HistoryApi Failed to convert JSON string to data at index \(idx)")
                    continue
                }

                do {
                    // Decode the SessionUpdate from stored JSON
                    let update = try JSONDecoder().decode(ACP.Client.SessionUpdate.self, from: data)
                    // Wrap it in SessionNotificationWire with the session_id
                    let wire = ACP.Client.SessionNotificationWire(
                        session_id: ACPSessionId(sessionId),
                        update: update,
                        _meta: nil
                    )
                    updates.append(wire)
                } catch {
                    OpenAgentsLog.server.error("HistoryApi Failed to decode update at index \(idx): \(error)")
                    OpenAgentsLog.server.error("HistoryApi JSON preview: \(jsonStr.prefix(200), privacy: .public)...")
                    // Continue processing remaining updates
                }
            }

            return updates
        } catch {
            OpenAgentsLog.server.error("HistoryApi sessionTimeline error: \(error)")
            throw HistoryError.queryFailed(reason: "Failed to load session timeline: \(error)")
        }
    }
}

// MARK: - Error Types

extension HistoryApi {
    /// Errors that can occur during history operations
    public enum HistoryError: Error, LocalizedError {
        case databaseNotAttached
        case invalidParameters(String)
        case queryFailed(reason: String)

        public var errorDescription: String? {
            switch self {
            case .databaseNotAttached:
                return "Tinyvex DB not attached"
            case .invalidParameters(let param):
                return "Invalid parameter: \(param)"
            case .queryFailed(let reason):
                return reason
            }
        }

        /// JSON-RPC error code for this error
        public var jsonRpcCode: Int {
            switch self {
            case .databaseNotAttached:
                return -32603 // Internal error
            case .invalidParameters:
                return -32602 // Invalid params
            case .queryFailed:
                return -32603 // Internal error
            }
        }
    }
}
#endif
