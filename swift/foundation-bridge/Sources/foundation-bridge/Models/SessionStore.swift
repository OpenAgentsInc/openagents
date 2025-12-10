import Foundation
import FoundationModels

/// Actor for managing LanguageModelSession instances
actor SessionStore {
    private var sessions: [String: SessionEntry] = [:]

    struct SessionEntry {
        let id: String
        let session: LanguageModelSession
        let created: Date
        var lastUsed: Date
        var transcript: [ChatMessage]

        init(id: String, session: LanguageModelSession, transcript: [ChatMessage] = []) {
            self.id = id
            self.session = session
            self.created = Date()
            self.lastUsed = Date()
            self.transcript = transcript
        }
    }

    /// Create a new session
    func createSession(model: SystemLanguageModel? = nil) -> String {
        let id = UUID().uuidString
        let sessionModel = model ?? SystemLanguageModel.default
        let session = LanguageModelSession(model: sessionModel)

        let entry = SessionEntry(id: id, session: session)
        sessions[id] = entry

        return id
    }

    /// Create a session with initial transcript
    func createSession(model: SystemLanguageModel? = nil, transcript: [ChatMessage]) -> String {
        let id = UUID().uuidString
        let sessionModel = model ?? SystemLanguageModel.default

        // Build transcript for FoundationModels
        var fmTranscript: [LanguageModelSession.Message] = []
        for msg in transcript {
            let role: LanguageModelSession.Message.Role = msg.role == "user" ? .user : .assistant
            fmTranscript.append(LanguageModelSession.Message(role: role, content: msg.content))
        }

        let session = LanguageModelSession(
            model: sessionModel,
            transcript: fmTranscript
        )

        let entry = SessionEntry(id: id, session: session, transcript: transcript)
        sessions[id] = entry

        return id
    }

    /// Get a session by ID
    func getSession(_ id: String) -> LanguageModelSession? {
        guard var entry = sessions[id] else {
            return nil
        }

        entry.lastUsed = Date()
        sessions[id] = entry

        return entry.session
    }

    /// Get session info (without the actual session object)
    func getSessionInfo(_ id: String) -> SessionInfo? {
        guard let entry = sessions[id] else {
            return nil
        }

        return SessionInfo(
            id: entry.id,
            created: entry.created,
            lastUsed: entry.lastUsed,
            messageCount: entry.transcript.count
        )
    }

    /// List all sessions
    func listSessions() -> [SessionInfo] {
        return sessions.values.map { entry in
            SessionInfo(
                id: entry.id,
                created: entry.created,
                lastUsed: entry.lastUsed,
                messageCount: entry.transcript.count
            )
        }
    }

    /// Get session transcript
    func getTranscript(_ id: String) -> [ChatMessage]? {
        guard let entry = sessions[id] else {
            return nil
        }

        return entry.transcript
    }

    /// Update session transcript (add messages)
    func appendToTranscript(_ id: String, messages: [ChatMessage]) {
        guard var entry = sessions[id] else {
            return
        }

        entry.transcript.append(contentsOf: messages)
        entry.lastUsed = Date()
        sessions[id] = entry
    }

    /// Delete a session
    func deleteSession(_ id: String) -> Bool {
        return sessions.removeValue(forKey: id) != nil
    }

    /// Delete all sessions
    func deleteAllSessions() {
        sessions.removeAll()
    }

    /// Clean up old sessions (older than specified seconds)
    func cleanupOldSessions(olderThan seconds: TimeInterval) -> Int {
        let cutoff = Date().addingTimeInterval(-seconds)
        let oldSessionIds = sessions.filter { $0.value.lastUsed < cutoff }.map { $0.key }

        for id in oldSessionIds {
            sessions.removeValue(forKey: id)
        }

        return oldSessionIds.count
    }
}

/// Session information for API responses
struct SessionInfo: Codable {
    let id: String
    let created: Date
    let lastUsed: Date
    let messageCount: Int
}

/// Response for session creation
struct CreateSessionResponse: Codable {
    let id: String
    let created: Date
}

/// Response for session listing
struct ListSessionsResponse: Codable {
    let sessions: [SessionInfo]
    let count: Int
}

/// Request for creating a session with transcript
struct CreateSessionRequest: Codable {
    let model: String?
    let transcript: [ChatMessage]?
}
