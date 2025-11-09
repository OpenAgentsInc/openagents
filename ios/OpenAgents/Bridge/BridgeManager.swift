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
