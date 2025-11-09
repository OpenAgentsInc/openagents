import Foundation
import Combine
import OpenAgentsCore

// MARK: - iOS Mobile Connection Manager
#if os(iOS)
final class MobileConnectionManager: NSObject, ConnectionManaging, MobileWebSocketClientDelegate {
    private let statusSubject = PassthroughSubject<BridgeManager.Status, Never>()
    private let logSubject = PassthroughSubject<String, Never>()
    private let workingDirectorySubject = CurrentValueSubject<String?, Never>(nil)
    private let notificationSubject = PassthroughSubject<(method: String, payload: Data), Never>()

    var statusPublisher: AnyPublisher<BridgeManager.Status, Never> { statusSubject.eraseToAnyPublisher() }
    var logPublisher: AnyPublisher<String, Never> { logSubject.eraseToAnyPublisher() }
    var workingDirectoryPublisher: AnyPublisher<String?, Never> { workingDirectorySubject.eraseToAnyPublisher() }
    var notificationPublisher: AnyPublisher<(method: String, payload: Data), Never> { notificationSubject.eraseToAnyPublisher() }

    private(set) var rpcClient: JSONRPCSending?
    private var client: MobileWebSocketClient? { rpcClient as? MobileWebSocketClient }
    private var browser: BonjourBrowser?

    private var currentHost: String?
    private var currentPort: Int?

    func start() {
        let (h, p) = BridgeManager.pickInitialEndpoint()
        log("start", "initial endpoint host=\(h) port=\(p) multicast=\(Features.multicastEnabled)")
        connect(host: h, port: p)

        if Features.multicastEnabled {
            let b = BonjourBrowser()
            browser = b
            statusSubject.send(.discovering)
            log("bonjour", "Searching for \(BridgeConfig.serviceType)")
            b.start(onResolved: { [weak self] host, port in
                self?.log("bonjour", "Resolved host=\(host) port=\(port)")
                self?.connect(host: host, port: port)
            }, onLog: { [weak self] msg in
                self?.log("bonjour", msg)
            })
        }
    }

    func performManualConnect(host: String, port: Int) {
        stop()
        browser = nil
        connect(host: host, port: port)
    }

    private func connect(host: String, port: Int) {
        guard let url = URL(string: "ws://\(host):\(port)") else { return }
        if client == nil {
            let c = MobileWebSocketClient()
            c.delegate = self
            rpcClient = c
        }
        client?.delegate = self
        statusSubject.send(.connecting(host: host, port: port))
        log("client", "Connecting to ws://\(host):\(port)")
        currentHost = host; currentPort = port
        client?.connect(url: url)
    }

    func stop() {
        browser?.stop(); browser = nil
        client?.disconnect(); rpcClient = nil
    }

    // MARK: - MobileWebSocketClientDelegate
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient, workingDirectory: String?) {
        log("client", "Connected; workingDir=\(workingDirectory ?? "nil")")
        workingDirectorySubject.send(workingDirectory)
        if let h = currentHost, let p = currentPort { statusSubject.send(.connected(host: h, port: p)) }
        if let h = currentHost, let p = currentPort {
            BridgeManager.saveLastSuccessfulEndpoint(host: h, port: p)
            log("client", "persisted endpoint host=\(h) port=\(p)")
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let e = error { log("client", "Disconnected: \(e.localizedDescription)"); statusSubject.send(.error("disconnect: \(e.localizedDescription)")) }
        else { log("client", "Disconnected"); statusSubject.send(.idle) }
        #if targetEnvironment(simulator)
        if currentHost != "127.0.0.1" {
            log("client", "Simulator fallback to 127.0.0.1:\(BridgeConfig.defaultPort)")
            connect(host: "127.0.0.1", port: Int(BridgeConfig.defaultPort))
        }
        #endif
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data) {
        notificationSubject.send((method, payload))
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCRequest method: String, id: String, params: Data) -> Data? {
        switch method {
        case ACPRPC.sessionRequestPermission:
            let resp = ACP.Client.RequestPermissionResponse(outcome: .cancelled)
            return try? JSONEncoder().encode(resp)
        case ACPRPC.fsReadTextFile, ACPRPC.fsWriteTextFile, ACPRPC.terminalRun:
            return nil
        default:
            return nil
        }
    }

    // MARK: - Logging helper
    private func log(_ tag: String, _ message: String) {
        let ts = ISO8601DateFormatter().string(from: Date())
        logSubject.send("[\(ts)] [\(tag)] \(message)")
    }
}
#endif

// MARK: - macOS Desktop Connection Manager
#if os(macOS)
import Network
final class DesktopConnectionManager: ConnectionManaging, DesktopWebSocketServerDelegate {
    private let statusSubject = PassthroughSubject<BridgeManager.Status, Never>()
    private let logSubject = PassthroughSubject<String, Never>()
    private let connectedClientCountSubject = CurrentValueSubject<Int, Never>(0)
    private let notificationSubject = PassthroughSubject<(method: String, payload: Data), Never>()
    private var cancellables: Set<AnyCancellable> = []

    var statusPublisher: AnyPublisher<BridgeManager.Status, Never> { statusSubject.eraseToAnyPublisher() }
    var logPublisher: AnyPublisher<String, Never> { logSubject.eraseToAnyPublisher() }
    var connectedClientCountPublisher: AnyPublisher<Int, Never> { connectedClientCountSubject.eraseToAnyPublisher() }
    var notificationPublisher: AnyPublisher<(method: String, payload: Data), Never> { notificationSubject.eraseToAnyPublisher() }

    var rpcClient: JSONRPCSending? { localRpc }

    var workingDirectoryURL: URL? {
        didSet { server?.workingDirectory = workingDirectoryURL }
    }

    private var server: DesktopWebSocketServer?
    private var localRpc: LocalJsonRpcClient?

    func start() {
        let srv = DesktopWebSocketServer()
        do {
            srv.delegate = self
            let dbPath = TinyvexManager.defaultDbPath().path
            srv.setTinyvexDb(path: dbPath)
            srv.workingDirectory = workingDirectoryURL
            try srv.start(port: BridgeConfig.defaultPort, advertiseService: true, serviceName: Host.current().localizedName, serviceType: BridgeConfig.serviceType)
            server = srv
            // Forward server notifications to app subscribers
            srv.notificationPublisher
                .receive(on: RunLoop.main)
                .sink { [weak self] evt in self?.notificationSubject.send(evt) }
                .store(in: &cancellables)
            // Provide local JSON-RPC adapter
            localRpc = LocalJsonRpcClient(server: srv)
            log("server", "DesktopWebSocketServer on ws://0.0.0.0:\(BridgeConfig.defaultPort)")
            statusSubject.send(.advertising(port: BridgeConfig.defaultPort))
        } catch {
            statusSubject.send(.error("server_start_failed: \(error.localizedDescription)"))
            log("server", "Failed to start: \(error.localizedDescription)")
        }
    }

    func stop() { server?.stop(); server = nil }

    // DesktopWebSocketServerDelegate
    func webSocketServer(_ server: DesktopWebSocketServer, didAccept client: DesktopWebSocketServer.Client) {
        connectedClientCountSubject.value += 1
    }

    func webSocketServer(_ server: DesktopWebSocketServer, didCompleteHandshakeFor client: DesktopWebSocketServer.Client, success: Bool) {
        // no-op; status remains advertising
    }

    func webSocketServer(_ server: DesktopWebSocketServer, didDisconnect client: DesktopWebSocketServer.Client, reason: NWError?) {
        connectedClientCountSubject.value = max(0, connectedClientCountSubject.value - 1)
    }

    private func log(_ tag: String, _ message: String) {
        let ts = ISO8601DateFormatter().string(from: Date())
        logSubject.send("[\(ts)] [\(tag)] \(message)")
    }
}
#endif
