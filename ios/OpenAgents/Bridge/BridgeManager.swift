import Foundation
import Combine
import OpenAgentsCore
import OpenAgentsCore

@MainActor
final class BridgeManager: ObservableObject {
    #if os(macOS)
    private var server: DesktopWebSocketServer?

    func start() {
        // Start Desktop WebSocket server automatically on macOS
        let srv = DesktopWebSocketServer(token: BridgeConfig.defaultToken)
        do {
            try srv.start(port: BridgeConfig.defaultPort, advertiseService: true, serviceName: Host.current().localizedName, serviceType: BridgeConfig.serviceType)
            server = srv
            print("[Bridge] Server running ws://0.0.0.0:\(BridgeConfig.defaultPort)")
        } catch {
            print("[Bridge] Failed to start server: \(error)")
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

    func start() {
        // Prefer Bonjour discovery
        let b = BonjourBrowser()
        browser = b
        b.start { [weak self] host, port in
            Task { @MainActor in self?.connect(host: host, port: port) }
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
        client?.connect(url: url, token: BridgeConfig.defaultToken)
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
        print("[Bridge] Connected to desktop")
        // Request thread summaries upon connect
        client.send(type: "threads.list.request", message: BridgeMessages.ThreadsListRequest(topK: 20))
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let e = error { print("[Bridge] Disconnected: \(e)") } else { print("[Bridge] Disconnected") }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        if message.type == "threads.list.response" {
            if let resp = try? message.decodedMessage(as: BridgeMessages.ThreadsListResponse.self) {
                self.threads = resp.items
            }
            return
        }
    }
}
#endif
