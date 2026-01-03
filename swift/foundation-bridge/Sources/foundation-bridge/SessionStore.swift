import Foundation
import FoundationModels

/// Actor that manages language model sessions
actor SessionStore {
    private var sessions: [String: LanguageModelSession] = [:]
    private var sessionInfos: [String: SessionInfo] = [:]
    private var transcripts: [String: [ChatMessage]] = [:]

    /// Create a new session with optional model
    func createSession(model: SystemLanguageModel? = nil) async -> String {
        let sessionId = UUID().uuidString
        let session = LanguageModelSession(model: model ?? .default)
        sessions[sessionId] = session
        sessionInfos[sessionId] = SessionInfo(
            id: sessionId,
            created: Date(),
            turnCount: 0
        )
        transcripts[sessionId] = []
        return sessionId
    }

    /// Create a new session with initial transcript
    func createSession(model: SystemLanguageModel? = nil, transcript: [ChatMessage]) async -> String {
        let sessionId = UUID().uuidString
        let session = LanguageModelSession(model: model ?? .default)
        sessions[sessionId] = session
        sessionInfos[sessionId] = SessionInfo(
            id: sessionId,
            created: Date(),
            turnCount: transcript.count / 2
        )
        transcripts[sessionId] = transcript
        return sessionId
    }

    /// Get session by ID
    func getSession(_ id: String) -> LanguageModelSession? {
        return sessions[id]
    }

    /// Get session info by ID
    func getSessionInfo(_ id: String) -> SessionInfo? {
        return sessionInfos[id]
    }

    /// Get transcript for a session
    func getTranscript(_ id: String) -> [ChatMessage]? {
        return transcripts[id]
    }

    /// Append messages to transcript
    func appendToTranscript(_ id: String, messages: [ChatMessage]) {
        if transcripts[id] != nil {
            transcripts[id]?.append(contentsOf: messages)
            if var info = sessionInfos[id] {
                info.turnCount += 1
                sessionInfos[id] = info
            }
        }
    }

    /// Delete a session
    func deleteSession(_ id: String) -> Bool {
        if sessions[id] != nil {
            sessions.removeValue(forKey: id)
            sessionInfos.removeValue(forKey: id)
            transcripts.removeValue(forKey: id)
            return true
        }
        return false
    }

    /// List all sessions
    func listSessions() -> [SessionInfo] {
        return Array(sessionInfos.values)
    }
}

/// Information about a session
struct SessionInfo: Codable {
    let id: String
    let created: Date
    var turnCount: Int

    enum CodingKeys: String, CodingKey {
        case id
        case created
        case turnCount = "turn_count"
    }
}

/// Request to create a session
struct CreateSessionRequest: Codable {
    let model: String?
    let transcript: [ChatMessage]?
}

/// Response for session creation
struct CreateSessionResponse: Codable {
    let id: String
    let created: Date
}

/// Response for listing sessions
struct ListSessionsResponse: Codable {
    let sessions: [SessionInfo]
    let count: Int
}
