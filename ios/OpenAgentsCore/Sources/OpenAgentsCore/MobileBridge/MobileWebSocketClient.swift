#if canImport(Foundation)
import Foundation

public protocol MobileWebSocketClientDelegate: AnyObject {
    /// Called when the client successfully connects and authenticates
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient)

    /// Called when the client disconnects or fails to connect
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?)

    /// Called when the client receives a message
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage)

    /// Called when a JSON-RPC notification is received (ACP path)
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data)

    /// Called when a JSON-RPC request is received (client-handled services).
    /// Implementations should synchronously return a JSON-encoded result Data, or nil to send a JSON-RPC error.
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCRequest method: String, id: String, params: Data) -> Data?
}

public final class MobileWebSocketClient {
    public weak var delegate: MobileWebSocketClientDelegate?

    private var webSocketTask: URLSessionWebSocketTask?
    private let session: URLSession

    private var isConnected: Bool = false
    private var pending: [String: (Data) -> Void] = [:]

    // Retry/reconnect configuration
    public var maxRetryAttempts: Int = 5
    public var initialRetryDelay: TimeInterval = 1.0
    public var maxRetryDelay: TimeInterval = 30.0
    /// Deprecated: legacy single-value timeout (unused when initial/retry are set)
    public var handshakeTimeout: TimeInterval = 10.0
    /// Shorter timeout for the first connection attempt to avoid long black-screen impressions.
    public var initialHandshakeTimeout: TimeInterval = 3.0
    /// Timeout for retry attempts after the first.
    public var retryHandshakeTimeout: TimeInterval = 10.0
    public var autoReconnect: Bool = true

    // Retry state
    private var retryCount: Int = 0
    private var lastConnectURL: URL?
    private var lastConnectToken: String?
    private var handshakeTimer: Timer?
    private var isManualDisconnect: Bool = false
    private var connectStartedAt: Date?

    public init(session: URLSession = .shared) {
        self.session = session
    }

    /// Connect to the WebSocket URL and authenticate with token
    /// - Parameters:
    ///   - url: The WebSocket URL to connect to
    ///   - token: The authentication token to send in Hello message
    public func connect(url: URL, token: String) {
        // Save connection parameters for retry
        lastConnectURL = url
        lastConnectToken = token
        isManualDisconnect = false

        // Reset retry count on new explicit connect
        retryCount = 0
        print("[Bridge][client] connect requested url=\(url.absoluteString) retryCount=\(retryCount)")
        performConnect(url: url, token: token)
    }

    private func performConnect(url: URL, token: String) {
        disconnect(error: nil, notifyDelegate: false)

        let request = URLRequest(url: url)
        let webSocketTask = session.webSocketTask(with: request)
        self.webSocketTask = webSocketTask
        connectStartedAt = Date()
        let hs = Self.handshakeTimeoutForAttempt(retryCount: retryCount, initial: initialHandshakeTimeout, retry: retryHandshakeTimeout)
        print("[Bridge][client] performConnect attempt=\(retryCount + 1) url=\(url.absoluteString) hsTimeout=\(String(format: "%.2f", hs))s autoReconnect=\(autoReconnect) initialDelay=\(initialRetryDelay)s maxDelay=\(maxRetryDelay)s")
        webSocketTask.resume()

        // Start handshake timeout (short on first attempt, longer on retries)
        startHandshakeTimeout()

        // Send ACP initialize via JSON-RPC
        sendInitialize(expectedToken: token)
    }

    /// Send ping to keep connection alive
    public func sendPing() {
        webSocketTask?.sendPing { [weak self] error in
            if let error = error {
                self?.disconnect(error: error, notifyDelegate: true)
            }
        }
    }

    /// Send an envelope with a typed message
    public func send<T: Codable>(type: String, message: T) {
        do {
            let env = try BridgeMessage.envelope(for: message, type: type)
            if let json = try? env.jsonString(prettyPrinted: false) {
                print("[Bridge][client] send text=\(json)")
                webSocketTask?.send(.string(json)) { _ in }
            } else {
                let data = try JSONEncoder().encode(env)
                print("[Bridge][client] send bytes \(type)")
                webSocketTask?.send(.data(data)) { _ in }
            }
        } catch {
            // ignore
        }
    }

    /// Send a JSON-RPC request and capture the response via an id-bound completion closure
    public func sendJSONRPC<P: Codable, R: Codable>(method: String, params: P, id: String = UUID().uuidString, completion: @escaping (R?) -> Void) {
        let req = JSONRPC.Request(id: JSONRPC.ID(id), method: method, params: params)
        guard let data = try? JSONEncoder().encode(req), let text = String(data: data, encoding: .utf8) else {
            completion(nil); return
        }
        // store pending handler
        pending[id] = { data in
            if let r = try? JSONDecoder().decode(R.self, from: data) { completion(r) } else { completion(nil) }
        }
        print("[Bridge][client] send rpc method=\(method) id=\(id) bytes=\(text.utf8.count)")
        webSocketTask?.send(.string(text)) { [weak self] error in
            if let error = error {
                _ = self?.pending.removeValue(forKey: id)
                completion(nil)
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected)
    public func sendJSONRPCNotification<P: Codable>(method: String, params: P) {
        let note = JSONRPC.Notification(method: method, params: params)
        guard let data = try? JSONEncoder().encode(note), let text = String(data: data, encoding: .utf8) else { return }
        print("[Bridge][client] send rpc notify method=\(method) bytes=\(text.utf8.count)")
        webSocketTask?.send(.string(text)) { _ in }
    }

    /// Disconnect the WebSocket connection
    public func disconnect() {
        isManualDisconnect = true
        disconnect(error: nil, notifyDelegate: true)
    }

    private func disconnect(error: Error?, notifyDelegate: Bool = true) {
        isConnected = false
        handshakeTimer?.invalidate()
        handshakeTimer = nil
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil

        if notifyDelegate {
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.delegate?.mobileWebSocketClient(self, didDisconnect: error)
            }

            // Attempt retry if not manual disconnect and retry is enabled
            if !isManualDisconnect && autoReconnect && error != nil {
                scheduleReconnect()
            }
        }
    }

    private func startHandshakeTimeout() {
        handshakeTimer?.invalidate()
        let interval = Self.handshakeTimeoutForAttempt(
            retryCount: retryCount,
            initial: initialHandshakeTimeout,
            retry: retryHandshakeTimeout
        )
        print("[Bridge][client] handshake timer scheduled in \(String(format: "%.2f", interval))s (attempt=\(retryCount + 1))")
        handshakeTimer = Timer.scheduledTimer(withTimeInterval: interval, repeats: false) { [weak self] _ in
            self?.handleHandshakeTimeout()
        }
    }

    private func handleHandshakeTimeout() {
        guard !isConnected else { return }
        let since = connectStartedAt.map { String(format: "%.1f", Date().timeIntervalSince($0) * 1000) } ?? "-"
        print("[Bridge][client] handshake timeout fired sinceConnectMs=\(since) attempt=\(retryCount + 1)")
        let error = NSError(domain: "MobileWebSocketClient", code: 7, userInfo: [NSLocalizedDescriptionKey: "Handshake timeout"])
        disconnect(error: error, notifyDelegate: true)
    }

    private func scheduleReconnect() {
        guard let url = lastConnectURL, let token = lastConnectToken else { return }
        guard retryCount < maxRetryAttempts else {
            let error = NSError(domain: "MobileWebSocketClient", code: 8, userInfo: [NSLocalizedDescriptionKey: "Max retry attempts reached"])
            DispatchQueue.main.async { [weak self] in
                guard let self = self else { return }
                self.delegate?.mobileWebSocketClient(self, didDisconnect: error)
            }
            return
        }

        retryCount += 1
        let delay = calculateBackoff(retryCount: retryCount)

        print("[Bridge][client] Scheduling reconnect attempt \(retryCount)/\(maxRetryAttempts) in \(String(format: "%.1f", delay))s")

        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            guard let self = self else { return }
            guard !self.isManualDisconnect else { return }
            if let url = self.lastConnectURL {
                print("[Bridge][client] reconnecting attempt=\(self.retryCount + 1) url=\(url.absoluteString)")
            }
            self.performConnect(url: url, token: token)
        }
    }

    private func calculateBackoff(retryCount: Int) -> TimeInterval {
        // Exponential backoff: initialDelay * 2^(retryCount - 1), capped at maxDelay
        let exponential = initialRetryDelay * pow(2.0, Double(retryCount - 1))
        return min(exponential, maxRetryDelay)
    }

    /// Compute the handshake timeout to use for the current attempt.
    /// - Note: attempt 0 (retryCount == 0) uses the shorter `initial` window.
    internal static func handshakeTimeoutForAttempt(retryCount: Int, initial: TimeInterval, retry: TimeInterval) -> TimeInterval {
        return (retryCount == 0) ? initial : retry
    }

    private func sendInitialize(expectedToken: String) {
        // Build request
        let initReq = ACP.Agent.InitializeRequest(
            protocol_version: "0.7.0",
            client_capabilities: .init(),
            client_info: ACP.Agent.Implementation(name: "openagents-ios", title: "OpenAgents iOS", version: "0.1.0")
        )
        let req = JSONRPC.Request(id: JSONRPC.ID("1"), method: "initialize", params: initReq)
        guard let data = try? JSONEncoder().encode(req), let text = String(data: data, encoding: .utf8) else {
            disconnect(error: NSError(domain: "MobileWebSocketClient", code: 4, userInfo: [NSLocalizedDescriptionKey: "Encode initialize failed"]), notifyDelegate: true)
            return
        }
        print("[Bridge][client] send rpc method=initialize id=1 bytes=\(text.utf8.count)")
        webSocketTask?.send(.string(text)) { [weak self] error in
            if let error = error { self?.disconnect(error: error, notifyDelegate: true); return }
            print("[Bridge][client] initialize sent; waiting for response…")
            self?.waitForInitializeResponse()
        }
    }

    private func waitForInitializeResponse() {
        webSocketTask?.receive { [weak self] result in
            guard let self = self else { return }

            switch result {
            case .failure(let error):
                self.disconnect(error: error, notifyDelegate: true)
            case .success(let message):
                switch message {
                case .data(let data):
                    if let text = String(data: data, encoding: .utf8) {
                        self.handleInitializeResponseText(text)
                    } else {
                        self.disconnect(error: NSError(domain: "MobileWebSocketClient", code: 5, userInfo: [NSLocalizedDescriptionKey: "Invalid initialize response"]), notifyDelegate: true)
                    }
                case .string(let text):
                    self.handleInitializeResponseText(text)
                @unknown default:
                    self.disconnect(error: NSError(domain: "MobileWebSocketClient", code: 3, userInfo: [NSLocalizedDescriptionKey: "Unknown message type"]), notifyDelegate: true)
                }
            }
        }
    }

    private func handleInitializeResponseText(_ text: String) {
        if let data = text.data(using: .utf8) {
            // Try JSON-RPC success response for initialize
            if let resp = try? JSONDecoder().decode(JSONRPC.Response<ACP.Agent.InitializeResponse>.self, from: data), resp.result.protocol_version.hasPrefix("0.7") {
                // Cancel handshake timeout on success
                handshakeTimer?.invalidate()
                handshakeTimer = nil

                // Reset retry count on successful connection
                retryCount = 0

                self.isConnected = true
                let since = connectStartedAt.map { String(format: "%.1f", Date().timeIntervalSince($0) * 1000) } ?? "-"
                print("[Bridge][client] initialize ok; connected sinceConnectMs=\(since)")
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.mobileWebSocketClientDidConnect(self)
                }
                // Start general receive loop after handshake
                self.receive()
                return
            }
        }
        // If not JSON-RPC initialize response, disconnect (strict ACP handshake)
        let preview = Self.truncatePreview(text)
        print("[Bridge][client] initialize unexpected response preview=\(preview)")
        self.disconnect(error: NSError(domain: "MobileWebSocketClient", code: 6, userInfo: [NSLocalizedDescriptionKey: "Initialize failed: unexpected response"]), notifyDelegate: true)
    }

    private func receive() {
        print("[Bridge][client] entering receive loop")
        webSocketTask?.receive { [weak self] result in
            guard let self = self else { return }
            switch result {
            case .failure(let error):
                self.disconnect(error: error, notifyDelegate: true)
            case .success(let message):
                switch message {
                case .data(let data):
                    if let text = String(data: data, encoding: .utf8) {
                        // JSON-RPC path (ACP)
                        if text.contains("\"jsonrpc\":\"2.0\"") {
                            self.handleJSONRPCText(text)
                            break
                        }
                        if let env = try? BridgeMessage.from(jsonString: text) {
                            DispatchQueue.main.async { [weak self] in
                                guard let self = self else { return }
                                self.delegate?.mobileWebSocketClient(self, didReceiveMessage: env)
                            }
                            break
                        }
                    }
                    if let env = try? JSONDecoder().decode(BridgeMessage.self, from: data) {
                        DispatchQueue.main.async { [weak self] in
                            guard let self = self else { return }
                            self.delegate?.mobileWebSocketClient(self, didReceiveMessage: env)
                        }
                    } else {
                        print("[Bridge][client] failed to decode data message")
                    }
                case .string(let text):
                    if text.contains("\"jsonrpc\":\"2.0\"") {
                        self.handleJSONRPCText(text)
                        break
                    }
                    if let env = try? BridgeMessage.from(jsonString: text) {
                        DispatchQueue.main.async { [weak self] in
                            guard let self = self else { return }
                            self.delegate?.mobileWebSocketClient(self, didReceiveMessage: env)
                        }
                    } else if let data = text.data(using: .utf8),
                              let env = try? JSONDecoder().decode(BridgeMessage.self, from: data) {
                        DispatchQueue.main.async { [weak self] in
                            guard let self = self else { return }
                            self.delegate?.mobileWebSocketClient(self, didReceiveMessage: env)
                        }
                    } else {
                        let bytes = text.utf8.count
                        let preview = Self.truncatePreview(text)
                        print("[Bridge][client] failed to decode string message bytes=\(bytes) preview=\(preview)")
                    }
                @unknown default:
                    break
                }

                self.receive()
            }
        }
    }

    private func handleJSONRPCText(_ text: String) {
        // Avoid logging full payloads; summarize in per-branch handlers below
        guard let data = text.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return }
        if let method = root["method"] as? String {
            // Notification path
            if method == ACPRPC.sessionUpdate {
                if let payload = try? JSONSerialization.data(withJSONObject: root["params"] ?? [:]) {
                    let s = String(data: payload, encoding: .utf8) ?? ""
                    let preview = Self.truncatePreview(s)
                    print("[Bridge][client] <- notify method=\(method) bytes=\(payload.count) preview=\(preview)")
                    // Pretty-print small JSON payloads for Xcode logs
                    if payload.count <= 12_000, let obj = try? JSONSerialization.jsonObject(with: payload), let pp = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys]), let pps = String(data: pp, encoding: .utf8) {
                        print("[Bridge][client] pretty params:\n\(pps)")
                    }
                    DispatchQueue.main.async { [weak self] in
                        guard let self = self else { return }
                        self.delegate?.mobileWebSocketClient(self, didReceiveJSONRPCNotification: method, payload: payload)
                    }
                }
            } else if root["id"] == nil {
                if let payload = try? JSONSerialization.data(withJSONObject: root["params"] ?? [:]) {
                    let s = String(data: payload, encoding: .utf8) ?? ""
                    let preview = Self.truncatePreview(s)
                    print("[Bridge][client] <- notify method=\(method) bytes=\(payload.count) preview=\(preview)")
                    if payload.count <= 12_000, let obj = try? JSONSerialization.jsonObject(with: payload), let pp = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys]), let pps = String(data: pp, encoding: .utf8) {
                        print("[Bridge][client] pretty params:\n\(pps)")
                    }
                    DispatchQueue.main.async { [weak self] in
                        guard let self = self else { return }
                        self.delegate?.mobileWebSocketClient(self, didReceiveJSONRPCNotification: method, payload: payload)
                    }
                }
            } else if let idAny = root["id"], let idStr: String = (idAny as? String) ?? (idAny as? Int).map(String.init) {
                // Request path (client-handled) → delegate for a synchronous result
                print("[Bridge][client] recv rpc request method=\(method) id=\(idStr)")
                let paramsData = (try? JSONSerialization.data(withJSONObject: root["params"] ?? [:])) ?? Data("{}".utf8)
                let resultData = delegate?.mobileWebSocketClient(self, didReceiveJSONRPCRequest: method, id: idStr, params: paramsData)
                if let rd = resultData,
                   let respJSON = try? JSONSerialization.jsonObject(with: rd) {
                    let envelope: [String: Any] = ["jsonrpc": "2.0", "id": idStr, "result": respJSON]
                    if let out = try? JSONSerialization.data(withJSONObject: envelope), let text = String(data: out, encoding: .utf8) {
                        print("[Bridge][client] send rpc result method=\(method) id=\(idStr) bytes=\(text.utf8.count)")
                        webSocketTask?.send(.string(text)) { _ in }
                    }
                } else {
                    // Send JSON-RPC error if unsupported
                    let err: [String: Any] = ["code": -32601, "message": "Method not implemented"]
                    let envelope: [String: Any] = ["jsonrpc": "2.0", "id": idStr, "error": err]
                    if let out = try? JSONSerialization.data(withJSONObject: envelope), let text = String(data: out, encoding: .utf8) {
                        print("[Bridge][client] send rpc error method=\(method) id=\(idStr) bytes=\(text.utf8.count)")
                        webSocketTask?.send(.string(text)) { _ in }
                    }
                }
            }
            return
        }
        // Response path
        if let idAny = root["id"], let result = root["result"],
           let idStr: String = (idAny as? String) ?? (idAny as? Int).map { String($0) } {
            let bytes = text.utf8.count
            let preview = Self.truncatePreview(text)
            print("[Bridge][client] <- result id=\(idStr) bytes=\(bytes) preview=\(preview)")
            if let pp = try? JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]), pp.count <= 20_000, let pps = String(data: pp, encoding: .utf8) {
                print("[Bridge][client] pretty result:\n\(pps)")
            }
            if let handler = pending.removeValue(forKey: idStr),
               let d = try? JSONSerialization.data(withJSONObject: result) {
                handler(d)
            }
        }
    }

    private static func truncatePreview(_ s: String, limit: Int = 160) -> String {
        if s.isEmpty { return "" }
        let compact = s.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
        if compact.count <= limit { return compact }
        let idx = compact.index(compact.startIndex, offsetBy: limit)
        return String(compact[..<idx]) + "…"
    }
}

#endif
