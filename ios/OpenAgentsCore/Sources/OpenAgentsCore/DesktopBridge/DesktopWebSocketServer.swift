#if os(macOS)
import Foundation
import Network
import Combine

/// Delegate protocol to notify connection and handshake results
public protocol DesktopWebSocketServerDelegate: AnyObject {
    func webSocketServer(_ server: DesktopWebSocketServer, didAccept client: DesktopWebSocketServer.Client)
    func webSocketServer(_ server: DesktopWebSocketServer, didCompleteHandshakeFor client: DesktopWebSocketServer.Client, success: Bool)
    func webSocketServer(_ server: DesktopWebSocketServer, didDisconnect client: DesktopWebSocketServer.Client, reason: NWError?)
}

/// A minimal WebSocket server for macOS using Network framework
public class DesktopWebSocketServer {
    /// Wrapped client connection
    public final class Client: Hashable {
        fileprivate let connection: NWConnection
        fileprivate var isHandshakeComplete = false

        fileprivate init(connection: NWConnection) {
            self.connection = connection
        }

        public static func == (lhs: Client, rhs: Client) -> Bool {
            return lhs.connection === rhs.connection
        }

        public func hash(into hasher: inout Hasher) {
            hasher.combine(ObjectIdentifier(connection))
        }

        /// Send a text message to the client (log full payload)
        public func send(text: String) {
            OpenAgentsLog.bridgeServer.debug("send text=\(text, privacy: .public)")
            let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
            let context = NWConnection.ContentContext(identifier: "textContext", metadata: [metadata])
            let data = text.data(using: .utf8) ?? Data()
            connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    OpenAgentsLog.bridgeServer.error("send error: \(sendError)")
                }
            })
        }

        /// Send a Ping frame
        public func sendPing() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .ping)
            let context = NWConnection.ContentContext(identifier: "pingContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    OpenAgentsLog.bridgeServer.error("ping send error: \(sendError)")
                }
            })
        }

        /// Send a Pong frame
        public func sendPong() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .pong)
            let context = NWConnection.ContentContext(identifier: "pongContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    OpenAgentsLog.bridgeServer.error("pong send error: \(sendError)")
                }
            })
        }

        /// Close connection with optional NWError
        public func close(reason: NWError? = nil) {
            connection.cancel()
        }
    }

    let queue = DispatchQueue(label: "DesktopWebSocketServerQueue")
    private var listener: NWListener?
    private var clients = Set<Client>()
    public weak var delegate: DesktopWebSocketServerDelegate?
    // Optional Tinyvex persistence for ACP updates
    var tinyvexDb: TinyvexDbLayer?
    // Session update hub for persistence+broadcast
    var updateHub: SessionUpdateHub?
    // History API for Tinyvex queries
    var historyApi: HistoryApi?
    // JSON-RPC router for method dispatch
    let router = JsonRpcRouter()
    // Current client context for handlers (set during routing)
    var currentClient: Client?
    // Agent registry for provider management
    let agentRegistry = AgentRegistry()

    // MARK: - Local app broadcast (Combine)
    private let broadcastSubject = PassthroughSubject<(method: String, payload: Data), Never>()
    public var notificationPublisher: AnyPublisher<(method: String, payload: Data), Never> {
        broadcastSubject.eraseToAnyPublisher()
    }

    // Ext capabilities negotiated/advertised during initialize
    private(set) var advertisedExtCapabilities: ACP.Agent.ExtCapabilities = .init(orchestrate_explore: false)
    /// Test/feature-flag override for orchestrate.explore.* support
    public var overrideExtOrchestrateExplore: Bool? = nil

    // Feature flags
    /// Enable auto-tailing of latest Claude Code session on client connect (default: false)
    public var enableAutoTailing = false

    // Working directory (set by delegate/owner)
    public var workingDirectory: URL? = nil

    // Live tailer state
    var tailTimer: DispatchSourceTimer?
    var tailURL: URL?
    var tailSessionId: ACPSessionId?
    var tailProvider: String?
    var tailOffset: UInt64 = 0
    var tailBuffer = Data()
    // Stdout buffer per session for partial lines (Codex exec --json)
    public var useProviderTailer: Bool = false // disable global tailer by default

    // Session mode selection (per session)
    var modeBySession: [String: ACPSessionModeId] = [:]

    // Setup session tracking (session_id -> conversation_id for conversational orchestration)
    var setupSessionById: [String: String] = [:]

    // Session file tracking (for optional tailer)
    var sessionFiles: [String: URL] = [:]  // session_id -> JSONL file URL

    public init() {
        // Register agent providers
        Task {
            await registerAgentProviders()
        }
        registerHandlers()
    }

    /// Register all available agent providers
    private func registerAgentProviders() async {
        await agentRegistry.register(CodexAgentProvider())
        await agentRegistry.register(ClaudeCodeAgentProvider())
        let count = await agentRegistry.allProviders().count
        OpenAgentsLog.bridgeServer.info("Registered \(count) agent providers")
    }

    /// Register all JSON-RPC method handlers with the router
    private func registerHandlers() {
        // Simple handlers first
        registerSessionNewHandler()
        registerHistoryHandlers()
        registerThreadHandlers()
        registerSessionHandlers()
        registerFileSystemHandlers()
        registerTerminalHandler()
        registerOrchestrationHandler()
    }

    // MARK: - Handler Registration Methods

    // moved to DesktopWebSocketServer+Session.swift: registerSessionNewHandler

    private func registerHistoryHandlers() {
        // tinyvex/history.recentSessions
        router.register(method: "tinyvex/history.recentSessions") { [weak self] id, _, _ in
            guard let self = self, let client = self.currentClient else { return }
            guard let api = self.historyApi else {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "History API not initialized") { text in
                    client.send(text: text)
                }
                return
            }

            do {
                let items = try await api.recentSessions()
                JsonRpcRouter.sendResponse(id: id, result: items) { text in
                    client.send(text: text)
                }
            } catch let error as HistoryApi.HistoryError {
                JsonRpcRouter.sendError(id: id, code: error.jsonRpcCode, message: error.localizedDescription) { text in
                    client.send(text: text)
                }
            } catch {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to query recent sessions: \(error)") { text in
                    client.send(text: text)
                }
            }
        }

        // tinyvex/history.sessionTimeline
        router.register(method: "tinyvex/history.sessionTimeline") { [weak self] id, params, _ in
            guard let self = self, let client = self.currentClient else { return }
            guard let api = self.historyApi else {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "History API not initialized") { text in
                    client.send(text: text)
                }
                return
            }

            guard let sessionId = params?["session_id"] as? String else {
                JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing required parameter: session_id") { text in
                    client.send(text: text)
                }
                return
            }

            let limit = params?["limit"] as? Int

            do {
                let updates = try await api.sessionTimeline(sessionId: sessionId, limit: limit)
                JsonRpcRouter.sendResponse(id: id, result: updates) { text in
                    client.send(text: text)
                }
            } catch let error as HistoryApi.HistoryError {
                JsonRpcRouter.sendError(id: id, code: error.jsonRpcCode, message: error.localizedDescription) { text in
                    client.send(text: text)
                }
            } catch {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to load session timeline: \(error)") { text in
                    client.send(text: text)
                }
            }
        }
    }

    // moved to DesktopWebSocketServer+Threads.swift: registerThreadHandlers

    // moved to DesktopWebSocketServer+Session.swift: registerSessionHandlers

    // moved to DesktopWebSocketServer+FileSystem.swift: registerFileSystemHandlers

    // moved to DesktopWebSocketServer+Terminal.swift: registerTerminalHandler

    // moved to DesktopWebSocketServer+Orchestration.swift: registerOrchestrationHandler

    // MARK: - Handler Implementation Methods (delegate to existing logic)

    // moved to DesktopWebSocketServer+Threads.swift: handleThreadsList

    // moved to DesktopWebSocketServer+Threads.swift: handleThreadLoadLatestTyped

    // moved to DesktopWebSocketServer+Threads.swift: handleIndexStatus

    // moved to DesktopWebSocketServer+Session.swift: handleSessionPrompt

    // moved to DesktopWebSocketServer+Session.swift: handleSessionSetMode

    // moved to DesktopWebSocketServer+Session.swift: handleSessionCancel

    // moved to DesktopWebSocketServer+FileSystem.swift: handleFsReadTextFile

    // moved to DesktopWebSocketServer+FileSystem.swift: handleFsWriteTextFile

    // moved to DesktopWebSocketServer+Terminal.swift: handleTerminalRun

    // moved to DesktopWebSocketServer+Orchestration.swift: handleOrchestrationStart

    public func setTinyvexDb(path: String) {
        if let db = try? TinyvexDbLayer(path: path) {
            self.tinyvexDb = db
            // Initialize SessionUpdateHub with broadcast callback
            self.updateHub = SessionUpdateHub(tinyvexDb: db) { [weak self] notificationJSON in
                // Broadcast to any connected WebSocket clients
                Task { [weak self] in
                    await self?.broadcastToClients(notificationJSON)
                }
                // Also publish to local app subscribers
                self?.publishNotificationToApp(notificationJSON)
            }
            // Initialize HistoryApi for Tinyvex queries
            self.historyApi = HistoryApi(tinyvexDb: db)
            OpenAgentsLog.bridgeServer.info("Tinyvex DB attached at \(path, privacy: .private)")
        } else {
            OpenAgentsLog.bridgeServer.error("Failed to open Tinyvex DB at \(path, privacy: .private)")
        }
    }

    /// Start listening on given port
    public func start(port: UInt16, advertiseService: Bool = true, serviceName: String? = nil, serviceType: String = BridgeConfig.serviceType) throws {
        let params = NWParameters(tls: nil)
        let wsOptions = NWProtocolWebSocket.Options()
        params.defaultProtocolStack.applicationProtocols.insert(wsOptions, at: 0)
        params.acceptLocalOnly = false
        guard let nwPort = NWEndpoint.Port(rawValue: port) else {
            throw NSError(domain: "DesktopWebSocketServer", code: -1, userInfo: [NSLocalizedDescriptionKey: "Invalid port: \(port)"])
        }
        listener = try NWListener(using: params, on: nwPort)
        // Advertise Bonjour service for discovery when supported
        if advertiseService {
            if #available(macOS 12.0, *) {
                let name = serviceName ?? Host.current().localizedName ?? "OpenAgents"
                listener?.service = NWListener.Service(name: name, type: serviceType, domain: "local.")
            }
        }
        listener?.stateUpdateHandler = { [weak self] newState in
            guard let self = self else { return }
            switch newState {
            case .ready:
                // Listening started
                break
            case .failed(let error):
                self.stop()
            default:
                break
            }
        }
        listener?.newConnectionHandler = { [weak self] nwConnection in
            self?.handleNewConnection(nwConnection)
        }
        listener?.start(queue: queue)
    }

    /// Stop the server and all connected clients
    public func stop() {
        listener?.cancel()
        listener = nil
        for client in clients {
            client.close()
        }
        clients.removeAll()
    }

    private func handleNewConnection(_ connection: NWConnection) {
        let client = Client(connection: connection)
        clients.insert(client)
        DispatchQueue.main.async { [weak self] in
            guard let self = self else { return }
            self.delegate?.webSocketServer(self, didAccept: client)
        }

        connection.stateUpdateHandler = { [weak self, weak client] state in
            guard let self = self, let client = client else { return }
            switch state {
            case .ready:
                self.receiveNextMessage(client)
            case .failed(_), .cancelled:
                self.clients.remove(client)
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.webSocketServer(self, didDisconnect: client, reason: nil)
                }
            default:
                break
            }
        }

        connection.start(queue: queue)
    }

    private func receiveNextMessage(_ client: Client) {
        client.connection.receiveMessage { [weak self, weak client] (data, context, isComplete, error) in
            guard let self = self, let client = client else { return }
            if let error = error {
                self.clients.remove(client)
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.webSocketServer(self, didDisconnect: client, reason: error)
                }
                client.connection.cancel()
                if self.clients.isEmpty { self.stopLiveTailer() }
                return
            }

            guard let ctx = context else {
                // No context means connection closed
                self.clients.remove(client)
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.webSocketServer(self, didDisconnect: client, reason: nil)
                }
                client.connection.cancel()
                if self.clients.isEmpty { self.stopLiveTailer() }
                return
            }

            if let wsMetadata = ctx.protocolMetadata(definition: NWProtocolWebSocket.definition) as? NWProtocolWebSocket.Metadata {
                switch wsMetadata.opcode {
                case .text:
                    if let data = data, let text = String(data: data, encoding: .utf8) {
                        self.handleTextMessage(text, from: client)
                    }
                case .binary:
                    if let data = data, let text = String(data: data, encoding: .utf8) {
                        self.handleTextMessage(text, from: client)
                    }
                case .close:
                    self.clients.remove(client)
                    self.delegate?.webSocketServer(self, didDisconnect: client, reason: nil)
                    client.connection.cancel()
                    if self.clients.isEmpty { self.stopLiveTailer() }
                    return
                case .ping:
                    client.sendPong()
                case .pong:
                    break // Pong received, no action needed
                default:
                    break
                }
            }
            // Continue receiving next message
            self.receiveNextMessage(client)
        }
    }

    // moved to DesktopWebSocketServer+Tailer.swift: findAndTailSessionFile

    private func sendErrorMessage(sessionId: ACPSessionId, message: String, client: Client) {
        let errorChunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ Error: \(message)")))
        let errorUpdate = ACP.Client.SessionUpdate.agentMessageChunk(errorChunk)
        OpenAgentsLog.bridgeServer.error("send error message: \(message)")
        Task { [weak self] in
            await self?.sendSessionUpdate(sessionId: sessionId, update: errorUpdate)
        }
    }

    private func handleTextMessage(_ text: String, from client: Client) {
        if !client.isHandshakeComplete {
            OpenAgentsLog.bridgeServer.debug("recv handshake text=\(text, privacy: .public)")
            if let data = text.data(using: .utf8), let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any], (dict["jsonrpc"] as? String) == "2.0", let method = dict["method"] as? String, method == "initialize" {
                let inIdStr: String = {
                    if let idNum = dict["id"] as? Int { return String(idNum) }
                    if let idStr = dict["id"] as? String { return idStr }
                    return "1"
                }()
                OpenAgentsLog.bridgeServer.info("recv rpc request method=initialize id=\(inIdStr)")
                let idVal: JSONRPC.ID
                if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                else { idVal = JSONRPC.ID("1") }
                // Include working directory in _meta if available
                OpenAgentsLog.bridgeServer.debug("workingDirectory=\(self.workingDirectory?.path ?? "nil", privacy: .private)")
                let meta: [String: AnyEncodable]? = {
                    if let wd = self.workingDirectory {
                        OpenAgentsLog.bridgeServer.debug("including working_directory in _meta: \(wd.path, privacy: .private)")
                        return ["working_directory": AnyEncodable(wd.path)]
                    }
                    OpenAgentsLog.bridgeServer.debug("workingDirectory is nil, not including in _meta")
                    return nil
                }()
                // Compute and advertise extension capabilities
                let extCaps: ACP.Agent.ExtCapabilities = {
                    if let override = self.overrideExtOrchestrateExplore { return .init(orchestrate_explore: override) }
                    if #available(macOS 26.0, *) { return .init(orchestrate_explore: true) }
                    return .init(orchestrate_explore: false)
                }()
                self.advertisedExtCapabilities = extCaps
                let agentCaps = ACP.Agent.AgentCapabilities(
                    load_session: false,
                    prompt_capabilities: .init(),
                    mcp_capabilities: .init(),
                    ext_capabilities: extCaps,
                    _meta: nil
                )
                let resp = ACP.Agent.InitializeResponse(protocol_version: "0.2.2", agent_capabilities: agentCaps, auth_methods: [], agent_info: ACP.Agent.Implementation(name: "openagents-mac", title: "OpenAgents macOS", version: "0.1.0"), _meta: meta)
                if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: resp)), let jtext = String(data: out, encoding: .utf8) {
                    OpenAgentsLog.bridgeServer.debug("send rpc result method=initialize id=\(inIdStr) text=\(jtext, privacy: .public)")
                    client.send(text: jtext)
                }
                client.isHandshakeComplete = true
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: true)
                }
                // Start live tailer after first handshake (if enabled via feature flag)
                if self.enableAutoTailing {
                    self.startLiveTailerIfNeeded()
                }
            }
        } else {
            // Handle envelopes after handshake
            OpenAgentsLog.bridgeServer.debug("recv payload text=\(text, privacy: .public)")
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               (dict["jsonrpc"] as? String) == "2.0" {
                // JSON-RPC request/notification
                if let method = dict["method"] as? String {
                    if let idAny = dict["id"] {
                        let idStr = (idAny as? String) ?? (idAny as? Int).map(String.init) ?? "1"
                        OpenAgentsLog.bridgeServer.debug("recv rpc request method=\(method) id=\(idStr)")
                    } else {
                        OpenAgentsLog.bridgeServer.debug("recv rpc notify method=\(method)")
                    }

                    // Route through JsonRpcRouter
                    self.currentClient = client
                    Task { [weak self] in
                        guard let self = self else { return }
                        let handled = await self.router.route(text: text)
                        await MainActor.run {
                            self.currentClient = nil
                        }
                        if !handled {
                            OpenAgentsLog.bridgeServer.warning("method '\(method)' not handled by any registered handler")
                        }
                    }
                }
                return
            }
            OpenAgentsLog.bridgeServer.debug("ignoring non JSON-RPC payload")
        }
    }

    // MARK: - Orchestration (Phase 2)

    // moved to DesktopWebSocketServer+Orchestration.swift: runOrchestration

    /// Send session update via WebSocket (delegates to SessionUpdateHub)
    func sendSessionUpdate(
        sessionId: ACPSessionId,
        update: ACP.Client.SessionUpdate
    ) async {
        // Delegate to SessionUpdateHub if available
        if let hub = updateHub {
            await hub.sendSessionUpdate(sessionId: sessionId, update: update)
        } else {
            // Fallback: direct broadcast without persistence (for tests or if hub not initialized)
            let notification = ACP.Client.SessionNotificationWire(
                session_id: sessionId,
                update: update,
                _meta: nil
            )
            guard let data = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: notification)),
                  let jtext = String(data: data, encoding: .utf8) else {
                OpenAgentsLog.bridgeServer.error("Failed to encode session update")
                return
            }
            await broadcastToClients(jtext)
        }
    }

    /// Broadcast a JSON-RPC notification to all connected clients
    private func broadcastToClients(_ notificationJSON: String) async {
        await MainActor.run {
            for c in self.clients where c.isHandshakeComplete {
                c.send(text: notificationJSON)
            }
        }
    }

    /// Publish a JSON-RPC notification to the local app via Combine
    private func publishNotificationToApp(_ notificationJSON: String) {
        guard let data = notificationJSON.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let method = root["method"] as? String,
              let params = root["params"],
              let payload = try? JSONSerialization.data(withJSONObject: params)
        else { return }
        broadcastSubject.send((method: method, payload: payload))
    }
}

// MARK: - Service helpers & error sender
extension DesktopWebSocketServer {
    static func urlFromURI(_ uri: String) -> URL? {
        if let u = URL(string: uri), u.scheme != nil { return u }
        return URL(fileURLWithPath: uri)
    }
    static func readText(fromURI uri: String) -> String? {
        guard let url = urlFromURI(uri) else { return nil }
        if url.isFileURL {
            return try? String(contentsOf: url, encoding: .utf8)
        }
        return nil
    }
    static func writeText(toURI uri: String, text: String) -> Bool {
        guard let url = urlFromURI(uri) else { return false }
        if url.isFileURL {
            guard let data = text.data(using: .utf8) else { return false }
            do { try data.write(to: url); return true } catch { return false }
        }
        return false
    }
    static func runCommand(_ cmd: [String], cwd: String?, env: [String:String]?, limit: Int?) -> (output: String, truncated: Bool, exitStatus: Int32?) {
        guard let prog = cmd.first else { return ("", false, nil) }
        let args = Array(cmd.dropFirst())
        let p = Process()
        p.executableURL = URL(fileURLWithPath: prog)
        p.arguments = args
        if let cwd = cwd { p.currentDirectoryURL = URL(fileURLWithPath: cwd) }
        var environment = ProcessInfo.processInfo.environment
        if let env = env { for (k,v) in env { environment[k] = v } }
        p.environment = environment
        let pipe = Pipe(); p.standardOutput = pipe; p.standardError = pipe
        var data = Data()
        do {
            try p.run()
            let limitBytes = max(0, limit ?? Int.max)
            while p.isRunning {
                let chunk = try pipe.fileHandleForReading.read(upToCount: 8192) ?? Data()
                if !chunk.isEmpty { data.append(chunk) }
                if data.count > limitBytes { break }
            }
            let more = (try? pipe.fileHandleForReading.readToEnd()) ?? Data()
            if !more.isEmpty { data.append(more) }
            p.waitUntilExit()
        } catch {
            return ("", false, nil)
        }
        let outStr = String(decoding: data.prefix((limit ?? Int.max)), as: UTF8.self)
        let truncated = data.count > (limit ?? Int.max)
        return (outStr, truncated, p.terminationStatus)
    }
    static func sendJSONRPCError(client: DesktopWebSocketServer.Client, id: JSONRPC.ID, code: Int, message: String) {
        let idAny: Any = Int(id.value) ?? id.value
        let envelope: [String: Any] = [
            "jsonrpc": "2.0",
            "id": idAny,
            "error": ["code": code, "message": message]
        ]
        if let out = try? JSONSerialization.data(withJSONObject: envelope), let text = String(data: out, encoding: .utf8) {
            OpenAgentsLog.bridgeServer.debug("send rpc error id=\(id.value) code=\(code) bytes=\(text.utf8.count)")
            client.send(text: text)
        }
    }
    // moved to DesktopWebSocketServer+Tailer.swift: tailJSONLLines

    // moved to DesktopWebSocketServer+Tailer.swift: pruneHeavyPayloads

    // moved to DesktopWebSocketServer+Tailer.swift: capTotalBytes

    // moved to DesktopWebSocketServer+Tailer.swift: pruneObject

    // moved to DesktopWebSocketServer+Tailer.swift: pruneToolShapes

    // moved to DesktopWebSocketServer+Tailer.swift: pruneRecursive

    // moved to DesktopWebSocketServer+Tailer.swift: summarize

    // moved to DesktopWebSocketServer+Tailer.swift: capStrings

    // moved to DesktopWebSocketServer+Tailer.swift: capString

    // moved to DesktopWebSocketServer+Tailer.swift: merge

    // moved to DesktopWebSocketServer+Tailer.swift: makeTypedUpdates

    // moved to DesktopWebSocketServer+Tailer.swift: isReasoningLine

    // moved to DesktopWebSocketServer+Tailer.swift: textFromTranslatedLine

    // moved to DesktopWebSocketServer+Tailer.swift: jsonValueToAnyEncodable

    // moved to DesktopWebSocketServer+Tailer.swift: live tailer and JSON mapping helpers
}
#endif
