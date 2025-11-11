import Foundation
import Combine
import OpenAgentsCore

#if os(macOS)
extension BridgeManager {
    private static let workingDirectoryKey = "oa.bridge.working_directory"

    func start() {
        loadWorkingDirectory()
        let conn = DesktopConnectionManager()
        conn.workingDirectoryURL = workingDirectory
        connection = conn

        // Connection events
        conn.statusPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] st in
                guard let self = self else { return }
                self.status = st
                // If RPC becomes available after start, (re)initialize dispatcher
                if self.dispatcher == nil, let rpc = self.connection?.rpcClient {
                    self.dispatcher = PromptDispatcher(
                        rpc: rpc,
                        timeline: self.timeline,
                        logger: { [weak self] tag, msg in self?.log(tag, msg) }
                    )
                    self.log("dispatcher", "ready (rpc available post-start)")
                }
            }
            .store(in: &subscriptions)
        conn.logPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] line in self?.log("conn", line) }
            .store(in: &subscriptions)
        conn.connectedClientCountPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] count in self?.connectedClientCount = count }
            .store(in: &subscriptions)

        // Forward session/update notifications into TimelineStore
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

        // Start server first so local RPC is created
        conn.start()
        // Initialize dispatcher after start, when rpcClient is set
        if let rpc = conn.rpcClient {
            dispatcher = PromptDispatcher(
                rpc: rpc,
                timeline: timeline,
                logger: { [weak self] tag, msg in self?.log(tag, msg) }
            )
            log("dispatcher", "initialized (rpc available)")
        } else {
            log("dispatcher", "deferred init: rpc not yet available")
        }
    }

    func stop() { connection?.stop(); connection = nil }

    func setWorkingDirectory(_ url: URL) {
        workingDirectory = url
        saveWorkingDirectory(url)
        (connection as? DesktopConnectionManager)?.workingDirectoryURL = url
    }

    func loadWorkingDirectory() {
        if let path = UserDefaults.standard.string(forKey: Self.workingDirectoryKey) {
            let url = URL(fileURLWithPath: path)
            if FileManager.default.fileExists(atPath: path) {
                workingDirectory = url
                log("workdir", "Loaded working directory: \(path)")
            }
        }
    }

    private func saveWorkingDirectory(_ url: URL) {
        UserDefaults.standard.set(url.path, forKey: Self.workingDirectoryKey)
        log("workdir", "Saved working directory: \(url.path)")
    }

    // Published properties are defined in BridgeManager

    // MARK: - Chat controls (shared with iOS patterns)
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

    func fetchRecentSessions() {
        dispatcher?.fetchRecentSessions { [weak self] items in
            guard let self = self else { return }
            self.recentSessions = items
            // Load any persisted titles from Tinyvex
            for s in items { self.syncSessionTitleFromDB(sessionId: s.session_id) }
        }
    }

    func loadSessionTimeline(sessionId: String) {
        currentSessionId = nil
        timeline.clearAll()
        dispatcher?.loadSessionTimeline(sessionId: sessionId) { [weak self] arr in
            self?.timeline.replaceAll(with: arr)
            self?.currentSessionId = ACPSessionId(sessionId)
            self?.log("history", "Loaded timeline for session \(sessionId): \(arr.count) updates")
        }
    }

    func deleteSession(_ sessionId: String, completion: (() -> Void)? = nil) {
        struct Params: Codable { let session_id: String }
        connection?.rpcClient?.sendJSONRPC(method: "tinyvex/history.deleteSession", params: Params(session_id: sessionId), id: "delete-session-\(UUID().uuidString)") { (resp: [String: Bool]?) in
            DispatchQueue.main.async { [weak self] in
                // Refresh recent sessions after delete
                self?.fetchRecentSessions()
                // Clear timeline if deleting the active session
                if self?.currentSessionId?.value == sessionId {
                    self?.currentSessionId = nil
                    self?.timeline.clearAll()
                }
                completion?()
            }
        }
    }

    func setSessionMode(_ mode: ACPSessionModeId) {
        dispatcher?.setSessionMode(mode, getSessionId: { self.currentSessionId })
    }

    // MARK: - Orchestration (Conversational Setup)
    func startOrchestrationSetup() {
        let ws = workingDirectory?.path
        dispatcher?.orchestrateSetupStart(workspaceRoot: ws, onSessionId: { [weak self] sid in
            DispatchQueue.main.async { self?.currentSessionId = sid }
        }, completion: { [weak self] _ in
            // Clear local timeline so the setup conversation renders in the current chat column
            self?.timeline.clearAll()
        })
    }

    func setSessionTitle(sessionId: String, title: String) {
        struct Params: Codable { let session_id: String; let title: String }
        connection?.rpcClient?.sendJSONRPC(method: "tinyvex/history.setSessionTitle", params: Params(session_id: sessionId, title: title), id: "set-title-\(UUID().uuidString)") { (_: [String: Bool]?) in }
    }

    func clearSessionTitle(sessionId: String) {
        struct Params: Codable { let session_id: String }
        connection?.rpcClient?.sendJSONRPC(method: "tinyvex/history.clearSessionTitle", params: Params(session_id: sessionId), id: "clear-title-\(UUID().uuidString)") { (_: [String: Bool]?) in
            DispatchQueue.main.async { [weak self] in self?.conversationTitles.removeValue(forKey: sessionId) }
        }
    }

    private func syncSessionTitleFromDB(sessionId: String) {
        struct Params: Codable { let session_id: String }
        struct Resp: Codable { let title: String? }
        connection?.rpcClient?.sendJSONRPC(method: "tinyvex/history.getSessionTitle", params: Params(session_id: sessionId), id: "get-title-\(UUID().uuidString)") { (resp: Resp?) in
            guard let t = resp?.title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
            DispatchQueue.main.async { [weak self] in self?.conversationTitles[sessionId] = t }
        }
    }

}
#endif
