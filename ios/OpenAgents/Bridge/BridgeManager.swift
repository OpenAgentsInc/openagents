import Foundation
import OSLog
import Combine
import OpenAgentsCore
import OpenAgentsCore
#if os(macOS)
import Network
#endif

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

    @Published var status: Status = .idle
    @Published var lastLog: String = ""
    @Published var logs: [String] = [] // recent logs (ring buffer)
    private var currentHost: String?
    private var currentPort: Int?
    // Active session identifier (cross‑platform; used by UI chips)
    @Published var currentSessionId: ACPSessionId? = nil

#if os(macOS)
    private var server: DesktopWebSocketServer?
    @Published var connectedClientCount: Int = 0
    @Published var workingDirectory: URL? = nil
    private static let workingDirectoryKey = "oa.bridge.working_directory"

    func start() {
        // Load persisted working directory
        loadWorkingDirectory()

        // Start Desktop WebSocket server automatically on macOS
        let srv = DesktopWebSocketServer()
        do {
            srv.delegate = self
            srv.workingDirectory = workingDirectory  // Set working directory on server
            try srv.start(port: BridgeConfig.defaultPort, advertiseService: true, serviceName: Host.current().localizedName, serviceType: BridgeConfig.serviceType)
            server = srv
            log("server", "Started on ws://0.0.0.0:\(BridgeConfig.defaultPort)")
            status = .advertising(port: BridgeConfig.defaultPort)
        } catch {
            status = .error("server_start_failed: \(error.localizedDescription)")
            log("server", "Failed to start: \(error.localizedDescription)")
        }
    }

    func stop() {
        server?.stop(); server = nil
    }

    func setWorkingDirectory(_ url: URL) {
        workingDirectory = url
        saveWorkingDirectory(url)
        // Update server's working directory so new connections get the updated value
        server?.workingDirectory = url
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
    #endif

    #if os(iOS)
    private var client: MobileWebSocketClient?
    private var browser: BonjourBrowser?
    private static let lastHostKey = "oa.bridge.last_host"
    private static let lastPortKey = "oa.bridge.last_port"
    @Published var threads: [ThreadSummary] = []
    @Published var updates: [ACP.Client.SessionNotificationWire] = []
    @Published var availableCommands: [ACP.Client.AvailableCommand] = []
    @Published var currentMode: ACPSessionModeId = .default_mode
    // Working directory from macOS server (received during initialize handshake)
    @Published var workingDirectory: String? = nil
    // Map tool call_id -> tool name for rendering updates
    @Published var toolCallNames: [String: String] = [:]
    // Track latest index in `updates` for each tool_call_update call_id to coalesce progress
    private var lastUpdateRowIndexByCallId: [String: Int] = [:]
    // Inspector data: latest raw JSON (full notification) and extracted `output` JSON per call_id
    @Published var rawJSONByCallId: [String: String] = [:]
    @Published var outputJSONByCallId: [String: String] = [:]

    func start() {
        let (h, p) = BridgeManager.pickInitialEndpoint()
        log("start", "initial endpoint host=\(h) port=\(p) multicast=\(Features.multicastEnabled)")
        connect(host: h, port: p)

        // Optionally also start Bonjour discovery in parallel if enabled.
        if Features.multicastEnabled {
            let b = BonjourBrowser()
            browser = b
            status = .discovering
            log("bonjour", "Searching for \(BridgeConfig.serviceType)")
            b.start(onResolved: { [weak self] host, port in
                Task { @MainActor in
                    self?.log("bonjour", "Resolved host=\(host) port=\(port)")
                    self?.connect(host: host, port: port)
                }
            }, onLog: { [weak self] msg in
                Task { @MainActor in self?.log("bonjour", msg) }
            })
        }
    }

    private func connect(host: String, port: Int) {
        let urlStr = "ws://\(host):\(port)"
        guard let url = URL(string: urlStr) else { return }
        if client == nil { client = MobileWebSocketClient() }
        client?.delegate = self
        status = .connecting(host: host, port: port)
        log("client", "Connecting to \(urlStr)")
        currentHost = host; currentPort = port
        client?.connect(url: url)
    }

    // Manual connection entry point (used by UI)
    func performManualConnect(host: String, port: Int) {
        stop() // stop existing client/browser
        #if os(iOS)
        browser = nil
        #endif
        connect(host: host, port: port)
    }

    func stop() {
        browser?.stop(); browser = nil
        client?.disconnect(); client = nil
    }
    #endif
}

#if os(iOS)
extension BridgeManager: MobileWebSocketClientDelegate {
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient, workingDirectory: String?) {
        log("client", "Connected; workingDir=\(workingDirectory ?? "nil")")
        // Store working directory from macOS server
        self.workingDirectory = workingDirectory
        if let h = currentHost, let p = currentPort { status = .connected(host: h, port: p) }
        // Persist last successful endpoint for fast reconnects on next launch
        if let h = currentHost, let p = currentPort {
            BridgeManager.saveLastSuccessfulEndpoint(host: h, port: p)
            log("client", "persisted endpoint host=\(h) port=\(p)")
        }
        // Hydration of previous threads is disabled per current product direction.
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let e = error { log("client", "Disconnected: \(e.localizedDescription)"); status = .error("disconnect: \(e.localizedDescription)") }
        else { log("client", "Disconnected"); status = .idle }
    }


    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data) {
        if method == ACPRPC.sessionUpdate {
            if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: payload) {
                log("client", "session.update for \(note.session_id.value)")
                // Keep currentSessionId in sync with incoming updates
                if currentSessionId == nil || currentSessionId != note.session_id {
                    currentSessionId = note.session_id
                }
                // Append/coalesce into ring buffer for UI to observe (BridgeManager is @MainActor)
                func adjustIndicesAfterPopFront() {
                    // Shift all stored indices down by 1; drop any that become negative
                    var newMap: [String:Int] = [:]
                    for (k, v) in self.lastUpdateRowIndexByCallId {
                        if v > 0 { newMap[k] = v - 1 }
                    }
                    self.lastUpdateRowIndexByCallId = newMap
                }

                switch note.update {
                case .toolCall(let call):
                    // Remember tool name for subsequent tool_call_update rows, but do NOT append a separate row.
                    // We want a single row per call_id that updates in place from 'started'→'completed'.
                    self.toolCallNames[call.call_id] = call.name
                case .toolCallUpdate(let upd):
                    // Keep raw JSON and extracted output for inspector
                    if let s = String(data: payload, encoding: .utf8) { self.rawJSONByCallId[upd.call_id] = s }
                    if let out = BridgeManager.extractOutputJSON(from: payload) { self.outputJSONByCallId[upd.call_id] = out }
                    if let idx = self.lastUpdateRowIndexByCallId[upd.call_id], idx < self.updates.count {
                        // Replace existing row for this call_id with the latest update (coalesce progress)
                        self.updates[idx] = note
                    } else {
                        if self.updates.count >= 200 { self.updates.removeFirst(); adjustIndicesAfterPopFront() }
                        self.updates.append(note)
                        self.lastUpdateRowIndexByCallId[upd.call_id] = self.updates.count - 1
                    }
                default:
                    if self.updates.count >= 200 { self.updates.removeFirst(); adjustIndicesAfterPopFront() }
                    self.updates.append(note)
                }
                // Force objectWillChange to notify observers even when count stays at 200
                self.objectWillChange.send()
                switch note.update {
                case .availableCommandsUpdate(let ac):
                    self.availableCommands = ac.available_commands
                case .currentModeUpdate(let cur):
                    self.currentMode = cur.current_mode_id
                default:
                    break
                }
            }
        } else {
            if let s = String(data: payload, encoding: .utf8) { log("client", "notify \(method): \(s)") }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCRequest method: String, id: String, params: Data) -> Data? {
        // Client-handled services stubs. iOS returns not-implemented for fs/terminal; session/request_permission returns Cancelled.
        switch method {
        case ACPRPC.sessionRequestPermission:
            let resp = ACP.Client.RequestPermissionResponse(outcome: .cancelled)
            return try? JSONEncoder().encode(resp)
        case ACPRPC.fsReadTextFile, ACPRPC.fsWriteTextFile, ACPRPC.terminalRun:
            return nil // triggers JSON-RPC error -32601 (not implemented)
        default:
            return nil
        }
    }
}
#endif

#if os(macOS)
extension BridgeManager: DesktopWebSocketServerDelegate {
    func webSocketServer(_ server: DesktopWebSocketServer, didAccept client: DesktopWebSocketServer.Client) {
        log("server", "client accepted")
    }
    func webSocketServer(_ server: DesktopWebSocketServer, didCompleteHandshakeFor client: DesktopWebSocketServer.Client, success: Bool) {
        if success {
            connectedClientCount += 1
            log("server", "client handshake ok; connectedClients=\(connectedClientCount)")
        } else {
            log("server", "client handshake failed")
        }
    }
    func webSocketServer(_ server: DesktopWebSocketServer, didDisconnect client: DesktopWebSocketServer.Client, reason: NWError?) {
        if connectedClientCount > 0 { connectedClientCount -= 1 }
        log("server", "client disconnected; connectedClients=\(connectedClientCount)")
    }
}
#endif

// MARK: - Logging helper
extension BridgeManager {
    func log(_ tag: String, _ message: String) {
        let ts = ISO8601DateFormatter().string(from: Date())
        let line = "[\(ts)] [\(tag)] \(message)"
        print("[Bridge] \(line)")
        if Thread.isMainThread {
            lastLog = line
            logs.append(line)
            if logs.count > 200 { logs.removeFirst(logs.count - 200) }
        } else {
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.lastLog = line
                self.logs.append(line)
                if self.logs.count > 200 { self.logs.removeFirst(self.logs.count - 200) }
            }
        }
    }
}

#if os(iOS)
// MARK: - Prompt helpers
extension BridgeManager {
    struct EmptyResult: Codable {}
    struct Empty: Codable {}
    struct LatestThreadResult: Codable { let id: String; let lines: [String] }
    /// Send a JSON-RPC request via the mobile client, decoding the expected result type.
    func sendRPC<P: Codable, R: Codable>(method: String, params: P, id: String = UUID().uuidString, completion: @escaping (R?) -> Void) {
        guard let client = self.client else { completion(nil); return }
        client.sendJSONRPC(method: method, params: params, id: id) { (result: R?) in
            DispatchQueue.main.async { completion(result) }
        }
    }
    func sendPrompt(text: String) {
        sendPrompt(text: text, mode: nil)
    }

    /// Send prompt with optional session mode (e.g., .codex, .claude_code)
    func sendPrompt(text: String, mode: ACPSessionModeId?) {
        guard let client = self.client else { return }
        let parts: [ACP.Client.ContentBlock] = [.text(.init(text: text))]

        // Optimistic UI: Add user message immediately to timeline
        let userChunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
        let userUpdate = ACP.Client.SessionUpdate.userMessageChunk(userChunk)
        let sessionId = currentSessionId ?? ACPSessionId("pending")
        let optimisticNotification = ACP.Client.SessionNotificationWire(
            session_id: sessionId,
            update: userUpdate
        )

        // Add to updates ring buffer (same logic as WebSocket delegate)
        if updates.count >= 200 { updates.removeFirst() }
        updates.append(optimisticNotification)
        objectWillChange.send()

        if currentSessionId == nil {
            client.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: "session-new-\(UUID().uuidString)") { (resp: ACP.Agent.SessionNewResponse?) in
                guard let resp = resp else { return }
                DispatchQueue.main.async { [weak self] in self?.currentSessionId = resp.session_id }
                // Optionally set mode before sending the first prompt
                if let mode = mode {
                    struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
                    client.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: resp.session_id, mode_id: mode), id: "session-set-mode-\(UUID().uuidString)") { (_: ACP.Agent.SetSessionModeResponse?) in
                        let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                        client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)" ) { (_: EmptyResult?) in }
                    }
                } else {
                    let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                    client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)" ) { (_: EmptyResult?) in }
                }
            }
        } else if let sid = currentSessionId {
            if let mode = mode, mode != currentMode {
                struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
                client.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: sid, mode_id: mode), id: "session-set-mode-\(UUID().uuidString)") { (_: ACP.Agent.SetSessionModeResponse?) in }
            }
            let req = ACP.Agent.SessionPromptRequest(session_id: sid, content: parts)
            client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)") { (_: EmptyResult?) in }
        }
    }

    func cancelCurrentSession() {
        guard let client = self.client, let sid = currentSessionId else { return }
        struct CancelReq: Codable { let session_id: ACPSessionId }
        client.sendJSONRPCNotification(method: ACPRPC.sessionCancel, params: CancelReq(session_id: sid))
    }

    /// Start workspace exploration using on-device Foundation Models (Phase 2)
    func orchestrateExploreStart(root: String, goals: [String]? = nil, completion: ((OrchestrateExploreStartResponse?) -> Void)? = nil) {
        guard let client = self.client else {
            completion?(nil)
            return
        }

        // Create request with policy (on-device only for Phase 2)
        let policy = ExplorationPolicy(allow_external_llms: false, allow_network: false)
        let request = OrchestrateExploreStartRequest(
            root: root,
            remote_url: nil,
            branch: nil,
            policy: policy,
            goals: goals
        )

        // Send JSON-RPC request
        client.sendJSONRPC(
            method: ACPRPC.orchestrateExploreStart,
            params: request,
            id: "orchestrate-explore-\(UUID().uuidString)"
        ) { (response: OrchestrateExploreStartResponse?) in
            DispatchQueue.main.async {
                if let response = response {
                    // Update current session ID for streaming updates
                    self.currentSessionId = ACPSessionId(response.session_id)
                    print("[Bridge] Orchestration started: session=\(response.session_id) plan=\(response.plan_id)")
                }
                completion?(response)
            }
        }
    }
}

// MARK: - Endpoint persistence / selection
extension BridgeManager {
    static func saveLastSuccessfulEndpoint(host: String, port: Int) {
        UserDefaults.standard.set(host, forKey: lastHostKey)
        UserDefaults.standard.set(port, forKey: lastPortKey)
        print("[Bridge][mgr] saved last endpoint host=\(host) port=\(port)")
    }
    static func readLastSuccessfulEndpoint() -> (String, Int)? {
        if let h = UserDefaults.standard.string(forKey: lastHostKey) {
            let p = UserDefaults.standard.integer(forKey: lastPortKey)
            if p > 0 { return (h, p) }
        }
        return nil
    }
    /// Decide the initial endpoint for iOS startup.
    /// Order: persisted last-successful → simulator loopback → configured default.
    static func pickInitialEndpoint() -> (String, Int) {
        if let last = readLastSuccessfulEndpoint() { print("[Bridge][mgr] pickInitialEndpoint using persisted host=\(last.0) port=\(last.1)"); return last }
        #if targetEnvironment(simulator)
        print("[Bridge][mgr] pickInitialEndpoint simulator loopback")
        return ("127.0.0.1", Int(BridgeConfig.defaultPort))
        #else
        print("[Bridge][mgr] pickInitialEndpoint default host=\(BridgeConfig.defaultHost) port=\(BridgeConfig.defaultPort)")
        return (BridgeConfig.defaultHost, Int(BridgeConfig.defaultPort))
        #endif
    }
}
#endif

// JSON-RPC threads list result shape
fileprivate struct ThreadsListResult: Codable { let items: [ThreadSummary] }

// MARK: - JSON helpers
extension BridgeManager {
    static func extractOutputJSON(from payload: Data) -> String? {
        guard let obj = try? JSONSerialization.jsonObject(with: payload) as? [String: Any],
              let params = obj["params"] as? [String: Any],
              let update = params["update"] as? [String: Any],
              let kind = update["sessionUpdate"] as? String, kind == "tool_call_update",
              let tcu = update["tool_call_update"] as? [String: Any],
              let output = tcu["output"]
        else { return nil }
        if let data = try? JSONSerialization.data(withJSONObject: output, options: [.prettyPrinted, .sortedKeys]),
           let text = String(data: data, encoding: .utf8) {
            return text
        }
        return nil
    }
}
