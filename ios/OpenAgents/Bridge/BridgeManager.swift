import Foundation
import Combine
import OpenAgentsCore
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

    @Published var status: Status = .idle
    @Published var lastLog: String = ""
    @Published var logs: [String] = [] // recent logs (ring buffer)
    private var currentHost: String?
    private var currentPort: Int?
    #if os(macOS)
    private var server: DesktopWebSocketServer?

    func start() {
        // Start Desktop WebSocket server automatically on macOS
        let srv = DesktopWebSocketServer(token: BridgeConfig.defaultToken)
        do {
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
        log("client", "Connected; sending threads.list.request")
        if let h = currentHost, let p = currentPort { status = .connected(host: h, port: p) }
        // Request thread summaries upon connect
        client.send(type: "threads.list.request", message: BridgeMessages.ThreadsListRequest(topK: 20))
        // Resend once after a short delay if nothing arrives
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) { [weak self] in
            guard let self = self else { return }
            if self.threads.isEmpty {
                self.log("client", "Resending threads.list.request (no response in 1s)")
                client.send(type: "threads.list.request", message: BridgeMessages.ThreadsListRequest(topK: 20))
            }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let e = error { log("client", "Disconnected: \(e.localizedDescription)"); status = .error("disconnect: \(e.localizedDescription)") }
        else { log("client", "Disconnected"); status = .idle }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        if message.type == "threads.list.response" {
            if let resp = try? message.decodedMessage(as: BridgeMessages.ThreadsListResponse.self) {
                self.threads = resp.items
                if let h = self.currentHost, let p = self.currentPort { self.status = .connected(host: h, port: p) }
                self.log("client", "Received threads.list.response count=\(resp.items.count)")
            }
            return
        }
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
