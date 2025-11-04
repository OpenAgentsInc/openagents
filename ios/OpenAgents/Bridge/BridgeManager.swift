import Foundation
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
#if os(macOS)
    private var server: DesktopWebSocketServer?
    @Published var connectedClientCount: Int = 0

    func start() {
        // Start Desktop WebSocket server automatically on macOS
        let srv = DesktopWebSocketServer(token: BridgeConfig.defaultToken)
        do {
            srv.delegate = self
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
    #endif

    #if os(iOS)
    private var client: MobileWebSocketClient?
    private var browser: BonjourBrowser?
    @Published var threads: [ThreadSummary] = []
    @Published var updates: [ACP.Client.SessionNotificationWire] = []
    @Published var currentSessionId: ACPSessionId? = nil

    func start() {
        // Prefer Bonjour discovery when feature flag is enabled
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
        } else {
            status = .idle
            log("bonjour", "Multicast discovery disabled (feature flag). Use Manual Connect.")
        }

        // Simulator fallback: localhost
        #if targetEnvironment(simulator)
        connect(host: "127.0.0.1", port: Int(BridgeConfig.defaultPort))
        #endif
    }

    private func connect(host: String, port: Int) {
        let urlStr = "ws://\(host):\(port)"
        guard let url = URL(string: urlStr) else { return }
        if client == nil { client = MobileWebSocketClient() }
        client?.delegate = self
        status = .connecting(host: host, port: port)
        log("client", "Connecting to \(urlStr)")
        currentHost = host; currentPort = port
        client?.connect(url: url, token: BridgeConfig.defaultToken)
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
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        log("client", "Connected; requesting threads/list")
        if let h = currentHost, let p = currentPort { status = .connected(host: h, port: p) }
        struct ThreadsListParams: Codable { let topK: Int? }
        client.sendJSONRPC(method: "threads/list", params: ThreadsListParams(topK: 10), id: "threads-list-1") { (resp: ThreadsListResult?) in
            guard let resp = resp else { return }
            DispatchQueue.main.async { [weak self] in
                self?.threads = resp.items
                if let h = self?.currentHost, let p = self?.currentPort { self?.status = .connected(host: h, port: p) }
                self?.log("client", "Received threads.list count=\(resp.items.count)")
            }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let e = error { log("client", "Disconnected: \(e.localizedDescription)"); status = .error("disconnect: \(e.localizedDescription)") }
        else { log("client", "Disconnected"); status = .idle }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        // Legacy path removed; JSON-RPC used exclusively now.
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data) {
        if method == ACPRPC.sessionUpdate {
            if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: payload) {
                log("client", "session.update for \(note.session_id.value)")
                // Append to ring buffer for UI to observe
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.updates.append(note)
                    if self.updates.count > 200 { self.updates.removeFirst(self.updates.count - 200) }
                }
            }
        } else {
            if let s = String(data: payload, encoding: .utf8) { log("client", "notify \(method): \(s)") }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCRequest method: String, id: String, params: Data) -> Data? {
        // Client-handled services stubs. iOS returns not-implemented for fs/terminal; session/request_permission returns Cancelled.
        struct ErrorOut: Codable { let _meta: [String: String]? }
        switch method {
        case ACPRPC.sessionRequestPermission:
            struct PermissionOutcome: Codable { let outcome: String }
            struct PermissionResponse: Codable { let outcome: PermissionOutcome }
            let resp = PermissionResponse(outcome: .init(outcome: "cancelled"))
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
    func sendPrompt(text: String) {
        guard let client = self.client else { return }
        let parts: [ACP.Client.ContentBlock] = [.text(text)]
        if currentSessionId == nil {
            client.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: "session-new-\(UUID().uuidString)") { (resp: ACP.Agent.SessionNewResponse?) in
                guard let resp = resp else { return }
                DispatchQueue.main.async { [weak self] in self?.currentSessionId = resp.session_id }
                let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)" ) { (_: EmptyResult?) in }
            }
        } else if let sid = currentSessionId {
            let req = ACP.Agent.SessionPromptRequest(session_id: sid, content: parts)
            client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)") { (_: EmptyResult?) in }
        }
    }
}
#endif

// JSON-RPC threads list result shape
fileprivate struct ThreadsListResult: Codable { let items: [ThreadSummary] }
