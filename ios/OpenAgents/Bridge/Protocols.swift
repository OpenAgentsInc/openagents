import Foundation
import Combine
import OpenAgentsCore

// MARK: - JSON-RPC Abstractions

protocol JSONRPCSending: AnyObject {
    func sendJSONRPC<P: Codable, R: Codable>(method: String, params: P, id: String, completion: @escaping (R?) -> Void)
    func sendJSONRPCNotification<P: Codable>(method: String, params: P)
}

#if os(iOS)
// Conform MobileWebSocketClient to JSONRPCSending for dependency injection
extension MobileWebSocketClient: JSONRPCSending {
    func sendJSONRPC<P: Codable, R: Codable>(method: String, params: P, id: String, completion: @escaping (R?) -> Void) {
        self.sendJSONRPC(method: method, params: params, id: id, completion: completion)
    }
    func sendJSONRPCNotification<P: Codable>(method: String, params: P) {
        self.sendJSONRPCNotification(method: method, params: params)
    }
}
#endif

// MARK: - Connection Managing

protocol ConnectionManaging: AnyObject {
    var statusPublisher: AnyPublisher<BridgeManager.Status, Never> { get }
    var logPublisher: AnyPublisher<String, Never> { get }
    func start()
    func stop()
    var rpcClient: JSONRPCSending? { get }
    var notificationPublisher: AnyPublisher<(method: String, payload: Data), Never> { get }
    #if os(iOS)
    func performManualConnect(host: String, port: Int)
    var workingDirectoryPublisher: AnyPublisher<String?, Never> { get }
    #endif
    #if os(macOS)
    var connectedClientCountPublisher: AnyPublisher<Int, Never> { get }
    var workingDirectoryURL: URL? { get set }
    #endif
}

// MARK: - Timeline Storing

protocol TimelineStoring: AnyObject {
    var updatesPublisher: AnyPublisher<[ACP.Client.SessionNotificationWire], Never> { get }
    var availableCommandsPublisher: AnyPublisher<[ACP.Client.AvailableCommand], Never> { get }
    var currentModePublisher: AnyPublisher<ACPSessionModeId, Never> { get }
    var toolCallNamesPublisher: AnyPublisher<[String: String], Never> { get }
    var rawJSONByCallIdPublisher: AnyPublisher<[String: String], Never> { get }
    var outputJSONByCallIdPublisher: AnyPublisher<[String: String], Never> { get }

    // Apply an incoming JSON-RPC notification payload for `session.update`
    func applySessionUpdatePayload(_ payload: Data)

    // Optimistic user echo
    func appendOptimisticUserMessage(text: String, sessionId: ACPSessionId) -> Int

    // Clear state for new session
    func clearAll()

    // Replace entire timeline (e.g., loading history)
    func replaceAll(with updates: [ACP.Client.SessionNotificationWire])
}

// MARK: - Prompt Dispatching

protocol PromptDispatching: AnyObject {
    func sendPrompt(text: String, desiredMode: ACPSessionModeId?, getSessionId: () -> ACPSessionId?, setSessionId: @escaping (ACPSessionId) -> Void)
    func setSessionMode(_ mode: ACPSessionModeId, getSessionId: () -> ACPSessionId?)
    func cancelCurrentSession(getSessionId: () -> ACPSessionId?)

    // Orchestrator
    func orchestrateExploreStart(root: String, goals: [String]?, onSessionId: @escaping (ACPSessionId) -> Void, completion: ((OrchestrateExploreStartResponse?) -> Void)?)
    func orchestrateCoordinatorRunOnce(configId: String?, configInline: OrchestrationConfig?, completion: ((PromptDispatcher.CoordinatorRunOnceResponse?) -> Void)?)
    func orchestrateSetupStart(workspaceRoot: String?, onSessionId: @escaping (ACPSessionId) -> Void, completion: ((PromptDispatcher.SetupStartResponse?) -> Void)?)

    // History
    func fetchRecentSessions(completion: @escaping ([RecentSession]) -> Void)
    func loadSessionTimeline(sessionId: String, completion: @escaping ([ACP.Client.SessionNotificationWire]) -> Void)
}
