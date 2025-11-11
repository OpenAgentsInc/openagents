import Foundation
import Combine
import OpenAgentsCore

@MainActor
final class BridgeManager: ObservableObject {
    enum Status: Equatable {
        case idle
        case advertising(port: UInt16)
        case discovering
        case connecting(host: String, port: Int)
        case handshaking(host: String, port: Int)
        case connected(host: String, port: Int)
        case error(String)
    }

    // Common published state
    @Published var status: Status = .idle
    @Published var lastLog: String = ""
    @Published var logs: [String] = [] // recent logs (ring buffer)
    @Published var currentSessionId: ACPSessionId? = nil

    // Subscriptions for composed components
    var subscriptions: Set<AnyCancellable> = []

    // Shared collaborators
    var connection: ConnectionManaging?

    // macOS/iOS shared chat state
    @Published var threads: [ThreadSummary] = []
    @Published var updates: [ACP.Client.SessionNotificationWire] = []
    @Published var availableCommands: [ACP.Client.AvailableCommand] = []
    @Published var currentMode: ACPSessionModeId = .default_mode
    @Published var toolCallNames: [String: String] = [:]
    @Published var rawJSONByCallId: [String: String] = [:]
    @Published var outputJSONByCallId: [String: String] = [:]
    @Published var recentSessions: [RecentSession] = []
    @Published var selectedAgent: ACP.Client.AvailableCommand? = nil
    @Published var conversationTitles: [String: String] = [:]
    @Published var selectedToolCallId: String? = nil

    // Shared collaborators for chat state
    var timeline = TimelineStore()
    var dispatcher: PromptDispatcher?
    
    // macOS state
    #if os(macOS)
    @Published var connectedClientCount: Int = 0
    @Published var workingDirectory: URL? = nil
    #endif

    // iOS state
    #if os(iOS)
    @Published var workingDirectory: String? = nil
    #endif
}

// MARK: - Shared convenience APIs
extension BridgeManager {
    /// Determine the preferred session mode for sending a prompt.
    /// Priority:
    /// - If a specific agent is selected, map its name to a mode.
    /// - Else, if currentMode is set (non-default), use it.
    /// - Else, return nil (server default).
    func preferredModeForSend() -> ACPSessionModeId? {
        if let agent = selectedAgent {
            let n = agent.name.lowercased()
            if n.contains("claude") { return .claude_code }
            if n.contains("codex") { return .codex }
        }
        // If a session already has a mode, use it
        if currentMode != .default_mode { return currentMode }
        // Else, consult persisted default
        if let stored = UserDefaults.standard.string(forKey: "defaultAgentMode"),
           let mode = ACPSessionModeId(rawValue: stored) { return mode }
        // Fallback: OpenAgents (default_mode)
        return .default_mode
    }

    func sendPrompt(text: String, desiredMode: ACPSessionModeId? = nil) {
        // Auto-route conversational questions to OpenAgents orchestrator
        // regardless of selected mode
        var finalMode = desiredMode
        if let mode = desiredMode,
           mode != .default_mode,
           ConversationalDetection.isConversational(text) {
            log("routing", "auto-routing conversational question to orchestrator (overriding \(mode.rawValue))")
            finalMode = .default_mode
        }

        dispatcher?.sendPrompt(
            text: text,
            desiredMode: finalMode,
            getSessionId: { self.currentSessionId },
            setSessionId: { self.currentSessionId = $0 }
        )
    }

    /// Generate a short conversation title for the current session, if missing.
    func generateConversationTitleIfNeeded() {
        guard let sid = currentSessionId?.value else { return }
        if conversationTitles[sid] != nil { return }
        // Build minimal ACPMessage list from updates for summarization
        var ts: Int64 = 0
        var messages: [ACPMessage] = []
        for note in updates where note.session_id.value == sid {
            ts += 1000
            switch note.update {
            case .userMessageChunk(let chunk):
                if case let .text(t) = chunk.content {
                    let parts: [ACPContentPart] = [.text(ACPText(text: t.text))]
                    messages.append(ACPMessage(id: UUID().uuidString, role: .user, parts: parts, ts: ts))
                }
            case .agentMessageChunk(let chunk):
                if case let .text(t) = chunk.content {
                    let parts: [ACPContentPart] = [.text(ACPText(text: t.text))]
                    messages.append(ACPMessage(id: UUID().uuidString, role: .assistant, parts: parts, ts: ts))
                }
            default:
                continue
            }
        }
        Task { @MainActor in
            let title = await ConversationSummarizer.summarizeTitle(messages: messages)
            self.conversationTitles[sid] = title
        }
    }
}
