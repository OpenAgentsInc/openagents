import Foundation
import SwiftUI
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
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let e = error { print("[Bridge] Disconnected: \(e)") } else { print("[Bridge] Disconnected") }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        // Placeholder for future envelopes
        if let json = try? message.jsonString(prettyPrinted: false) { print("[Bridge] recv: \(json)") }
    }
}
#endif
