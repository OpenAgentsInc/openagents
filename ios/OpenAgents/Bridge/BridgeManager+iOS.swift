import Foundation
import Combine
import OpenAgentsCore

#if os(iOS)
extension BridgeManager {
    // Managers
    private var mobileConn: MobileConnectionManager? { connection as? MobileConnectionManager }

    // Dependencies
    func start() {
        let conn = MobileConnectionManager()
        wireConnection(conn)
        conn.start()
    }

    // Internal for tests: inject a custom connection manager and wire subscriptions
    func wireConnection(_ conn: ConnectionManaging) {
        connection = conn
        // Connection events
        conn.statusPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] st in self?.status = st }
            .store(in: &subscriptions)
        conn.logPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] line in self?.log("conn", line) }
            .store(in: &subscriptions)
        conn.workingDirectoryPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] wd in self?.workingDirectory = wd }
            .store(in: &subscriptions)
        conn.notificationPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] evt in
                guard let self = self else { return }
                if evt.method == ACPRPC.sessionUpdate {
                    self.timeline.applySessionUpdatePayload(evt.payload)
                } else if let s = String(data: evt.payload, encoding: .utf8) {
                    self.log("client", "notify \(evt.method): \(s)")
                }
            }
            .store(in: &subscriptions)

        // Mirror timeline state to published fields
        timeline.updatesPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.updates = $0 }
            .store(in: &subscriptions)
        timeline.availableCommandsPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.availableCommands = $0 }
            .store(in: &subscriptions)
        timeline.currentModePublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.currentMode = $0 }
            .store(in: &subscriptions)
        timeline.toolCallNamesPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.toolCallNames = $0 }
            .store(in: &subscriptions)
        timeline.rawJSONByCallIdPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.rawJSONByCallId = $0 }
            .store(in: &subscriptions)
        timeline.outputJSONByCallIdPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.outputJSONByCallId = $0 }
            .store(in: &subscriptions)

        // Initialize dispatcher with RPC client
        dispatcher = PromptDispatcher(rpc: conn.rpcClient, timeline: timeline)
    }

    // Manual connection entry point
    func performManualConnect(host: String, port: Int) {
        mobileConn?.performManualConnect(host: host, port: port)
    }

    func stop() { connection?.stop() }

    /// Start a fresh server-side session and clear the local timeline/cache.
    func startNewSession(desiredMode: ACPSessionModeId? = nil) {
        // Clear local state immediately
        timeline.clearAll()
        objectWillChange.send()
        currentSessionId = nil

        guard let rpc = connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: "session-new-\(UUID().uuidString)") { (resp: ACP.Agent.SessionNewResponse?) in
            guard let resp = resp else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.currentSessionId = resp.session_id
                if let mode = desiredMode {
                    struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
                    rpc.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: resp.session_id, mode_id: mode), id: "session-set-mode-\(UUID().uuidString)") { (_: ACP.Agent.SetSessionModeResponse?) in }
                }
            }
        }
    }

    // MARK: - RPC convenience
    struct EmptyResult: Codable {}
    struct Empty: Codable {}
    struct LatestThreadResult: Codable { let id: String; let lines: [String] }
    func sendRPC<P: Codable, R: Codable>(method: String, params: P, id: String = UUID().uuidString, completion: @escaping (R?) -> Void) {
        guard let client = self.connection?.rpcClient else { completion(nil); return }
        client.sendJSONRPC(method: method, params: params, id: id) { (result: R?) in
            DispatchQueue.main.async { completion(result) }
        }
    }

    // MARK: - Prompt APIs
    func sendPrompt(text: String) {
        dispatcher?.sendPrompt(text: text, desiredMode: nil, getSessionId: { self.currentSessionId }, setSessionId: { self.currentSessionId = $0 })
    }
    func sendPrompt(text: String, mode: ACPSessionModeId?) {
        dispatcher?.sendPrompt(text: text, desiredMode: mode, getSessionId: { self.currentSessionId }, setSessionId: { self.currentSessionId = $0 })
    }
    func setSessionMode(_ mode: ACPSessionModeId) {
        dispatcher?.setSessionMode(mode, getSessionId: { self.currentSessionId })
    }
    func cancelCurrentSession() {
        dispatcher?.cancelCurrentSession(getSessionId: { self.currentSessionId })
    }

    // MARK: - Orchestration
    func orchestrateExploreStart(root: String, goals: [String]? = nil, completion: ((OrchestrateExploreStartResponse?) -> Void)? = nil) {
        dispatcher?.orchestrateExploreStart(root: root, goals: goals, onSessionId: { [weak self] sid in self?.currentSessionId = sid }, completion: completion)
    }

    // MARK: - History
    func fetchRecentSessions() {
        dispatcher?.fetchRecentSessions { [weak self] sessions in
            guard let self = self else { return }
            self.recentSessions = sessions
            self.log("history", "Loaded \(sessions.count) recent sessions from Tinyvex")
        }
    }
    func loadSessionTimeline(sessionId: String) {
        currentSessionId = nil
        timeline.clearAll()
        dispatcher?.loadSessionTimeline(sessionId: sessionId) { [weak self] arr in
            self?.timeline.replaceAll(with: arr)
            self?.log("history", "Loaded timeline for session \(sessionId): \(arr.count) updates")
        }
    }
}
#endif
