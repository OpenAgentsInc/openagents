#if os(macOS)
import Foundation
import Network

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
            OpenAgentsLog.server.debug("send text=\(text, privacy: .public)")
            let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
            let context = NWConnection.ContentContext(identifier: "textContext", metadata: [metadata])
            let data = text.data(using: .utf8) ?? Data()
            connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    OpenAgentsLog.server.error("send error: \(sendError)")
                }
            })
        }

        /// Send a Ping frame
        public func sendPing() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .ping)
            let context = NWConnection.ContentContext(identifier: "pingContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    OpenAgentsLog.server.error("ping send error: \(sendError)")
                }
            })
        }

        /// Send a Pong frame
        public func sendPong() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .pong)
            let context = NWConnection.ContentContext(identifier: "pongContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    OpenAgentsLog.server.error("pong send error: \(sendError)")
                }
            })
        }

        /// Close connection with optional NWError
        public func close(reason: NWError? = nil) {
            connection.cancel()
        }
    }

    private let queue = DispatchQueue(label: "DesktopWebSocketServerQueue")
    private var listener: NWListener?
    private var clients = Set<Client>()
    public weak var delegate: DesktopWebSocketServerDelegate?
    // Optional Tinyvex persistence for ACP updates
    private var tinyvexDb: TinyvexDbLayer?
    // Session update hub for persistence+broadcast
    private var updateHub: SessionUpdateHub?
    // History API for Tinyvex queries
    private var historyApi: HistoryApi?
    // JSON-RPC router for method dispatch
    private let router = JsonRpcRouter()
    // Current client context for handlers (set during routing)
    private var currentClient: Client?
    // Agent registry for provider management
    private let agentRegistry = AgentRegistry()

    // Feature flags
    /// Enable auto-tailing of latest Claude Code session on client connect (default: false)
    public var enableAutoTailing = false

    // Working directory (set by delegate/owner)
    public var workingDirectory: URL? = nil

    // Live tailer state
    private var tailTimer: DispatchSourceTimer?
    private var tailURL: URL?
    private var tailSessionId: ACPSessionId?
    private var tailProvider: String?
    private var tailOffset: UInt64 = 0
    private var tailBuffer = Data()
    // Stdout buffer per session for partial lines (Codex exec --json)
    public var useProviderTailer: Bool = false // disable global tailer by default

    // Session mode selection (per session)
    private var modeBySession: [String: ACPSessionModeId] = [:]

    // Session file tracking (for optional tailer)
    private var sessionFiles: [String: URL] = [:]  // session_id -> JSONL file URL

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
        OpenAgentsLog.server.info("Registered \(count) agent providers")
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

    private func registerSessionNewHandler() {
        router.register(method: ACPRPC.sessionNew) { [weak self] id, _, _ in
            guard let self = self, let client = self.currentClient else { return }
            let sid = ACPSessionId(UUID().uuidString)
            let result = ACP.Agent.SessionNewResponse(session_id: sid)

            JsonRpcRouter.sendResponse(id: id, result: result) { text in
                client.send(text: text)
            }
        }
    }

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

    private func registerThreadHandlers() {
        // threads/list
        router.register(method: "threads/list") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            // Delegate to existing implementation (will refactor later)
            await self.handleThreadsList(id: id, params: params, rawDict: rawDict, client: client)
        }

        // thread/load_latest
        router.register(method: "thread/load_latest") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleThreadLoadLatest(id: id, params: params, rawDict: rawDict, client: client)
        }

        // thread/load_latest_typed
        router.register(method: "thread/load_latest_typed") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleThreadLoadLatestTyped(id: id, params: params, rawDict: rawDict, client: client)
        }

        // index.status
        router.register(method: "index.status") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleIndexStatus(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    private func registerSessionHandlers() {
        // session/prompt
        router.register(method: ACPRPC.sessionPrompt) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSessionPrompt(id: id, params: params, rawDict: rawDict, client: client)
        }

        // session/set_mode
        router.register(method: ACPRPC.sessionSetMode) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSessionSetMode(id: id, params: params, rawDict: rawDict, client: client)
        }

        // session/cancel
        router.register(method: ACPRPC.sessionCancel) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSessionCancel(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    private func registerFileSystemHandlers() {
        // fs/readTextFile
        router.register(method: ACPRPC.fsReadTextFile) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleFsReadTextFile(id: id, params: params, rawDict: rawDict, client: client)
        }

        // fs/writeTextFile
        router.register(method: ACPRPC.fsWriteTextFile) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleFsWriteTextFile(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    private func registerTerminalHandler() {
        router.register(method: ACPRPC.terminalRun) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleTerminalRun(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    private func registerOrchestrationHandler() {
        router.register(method: ACPRPC.orchestrateExploreStart) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleOrchestrationStart(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    // MARK: - Handler Implementation Methods (delegate to existing logic)

    private func handleThreadsList(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Extract logic from switch case "threads/list" (lines ~814-834)
        struct Params: Codable { let topK: Int? }
        let topK: Int = {
            if let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let pr = try? JSONDecoder().decode(Params.self, from: d) { return pr.topK ?? 10 }
            return 10
        }()
        let limit = min(10, max(1, topK))
        let base = CodexScanner.defaultBaseDir()
        let urls = CodexScanner.listRecentTopN(at: base, topK: limit)
        var items = urls.map { CodexScanner.makeSummary(for: $0, base: base) }
        items.sort { ($0.last_message_ts ?? $0.updated_at) > ($1.last_message_ts ?? $1.updated_at) }
        struct Result: Codable { let items: [ThreadSummary] }
        let result = Result(items: items)
        JsonRpcRouter.sendResponse(id: id, result: result) { text in
            client.send(text: text)
        }
    }

    private func handleThreadLoadLatest(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Deprecated raw JSONL hydrate; prefer thread/load_latest_typed
        struct LatestResult: Codable {
            let id: String
            let lines: [String]
        }

        let paramLimit = (params?["limit_lines"] as? NSNumber)?.intValue
        let paramBytes = (params?["max_bytes"] as? NSNumber)?.intValue
        let base = CodexScanner.defaultBaseDir()
        let urls = CodexScanner.listRecentTopN(at: base, topK: 1)

        if let file = urls.first {
            let tid = CodexScanner.scanForThreadID(file) ?? CodexScanner.relativeId(for: file, base: base)
            // Read a larger window of lines before pruning/capping to fit mobile limits
            let maxLines = paramLimit ?? 16000
            let maxBytes = paramBytes ?? 1_000_000
            var lines = DesktopWebSocketServer.tailJSONLLines(at: file, maxBytes: maxBytes, maxLines: maxLines)
            // Prune heavy payloads (e.g., tool results and large strings) to fit mobile WS limits
            lines = DesktopWebSocketServer.pruneHeavyPayloads(in: lines)
            // Keep total payload comfortably under 1MB (account for envelope overhead)
            lines = DesktopWebSocketServer.capTotalBytes(lines, limit: 900_000)

            let result = LatestResult(id: tid, lines: lines)
            JsonRpcRouter.sendResponse(id: id, result: result) { text in
                OpenAgentsLog.server.debug("send rpc result method=thread/load_latest id=\(id.value) bytes=\(text.utf8.count)")
                client.send(text: text)
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32002, message: "No threads found") { text in
                client.send(text: text)
            }
        }
    }

    private func handleThreadLoadLatestTyped(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct LatestTypedResult: Codable {
            let id: String
            let updates: [ACP.Client.SessionUpdate]
        }

        // Load Claude Code sessions ONLY (fall back to Codex if none found)
        let claudeBase = ClaudeCodeScanner.defaultBaseDir()
        let claudeFiles = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: 1)

        var file: URL?
        var provider: String = "claude-code"
        var base: URL = claudeBase

        if let clf = claudeFiles.first {
            // Use Claude Code session
            file = clf
            provider = "claude-code"
            base = claudeBase
        } else {
            // Fallback to Codex only if no Claude Code sessions exist
            let codexBase = CodexScanner.defaultBaseDir()
            let codexFiles = CodexScanner.listRecentTopN(at: codexBase, topK: 1)
            if let cf = codexFiles.first {
                file = cf
                provider = "codex"
                base = codexBase
            }
        }

        if let file = file {
            let tid: String
            if provider == "codex" {
                tid = CodexScanner.scanForThreadID(file) ?? CodexScanner.relativeId(for: file, base: base)
            } else {
                tid = ClaudeCodeScanner.scanForSessionID(file) ?? ClaudeCodeScanner.relativeId(for: file, base: base)
            }

            // Read and translate to ACP
            let lines = DesktopWebSocketServer.tailJSONLLines(at: file, maxBytes: 1_000_000, maxLines: 16000)
            let updates = DesktopWebSocketServer.makeTypedUpdates(from: lines, provider: provider)

            let result = LatestTypedResult(id: tid, updates: updates)
            JsonRpcRouter.sendResponse(id: id, result: result) { text in
                OpenAgentsLog.server.debug("send rpc result method=thread/load_latest_typed id=\(id.value) provider=\(provider) bytes=\(text.utf8.count)")
                client.send(text: text)
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32002, message: "No threads found") { text in
                client.send(text: text)
            }
        }
    }

    private func handleIndexStatus(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Minimal Phase-1: respond and emit a tiny synthetic stream for visibility
        // Params may contain session_id to stream under
        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
        let sid = ACPSessionId(sidStr)

        struct StatusResult: Codable {
            let workspace_path: String
            let exists: Bool
            let files_indexed: Int?
            let chunks: Int?
        }

        // Default workspace path (env override TEST_WORKSPACE_ROOT)
        let ws = ProcessInfo.processInfo.environment["TEST_WORKSPACE_ROOT"] ?? "\(NSHomeDirectory())/code/openagents"
        let exists = FileManager.default.fileExists(atPath: ws)

        let result = StatusResult(workspace_path: ws, exists: exists, files_indexed: nil, chunks: nil)
        JsonRpcRouter.sendResponse(id: id, result: result) { text in
            OpenAgentsLog.server.debug("send rpc result method=index.status id=\(id.value) text=\(text)")
            client.send(text: text)
        }

        // Emit synthetic tool stream: index.rebuild started→completed
        let call = ACPToolCallWire(call_id: UUID().uuidString, name: "index.rebuild", arguments: [
            "workspace": AnyEncodable(ws)
        ])
        let toolNote = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCall(call))
        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: toolNote)), let jtext = String(data: out, encoding: .utf8) {
            client.send(text: jtext)
        }

        // started
        let started = ACPToolCallUpdateWire(call_id: call.call_id, status: .started, output: nil, error: nil, _meta: ["progress": AnyEncodable(0.0)])
        let startedNote = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCallUpdate(started))
        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: startedNote)), let jtext = String(data: out, encoding: .utf8) {
            client.send(text: jtext)
        }

        // completed with tiny payload
        let payload: [String: AnyEncodable] = [
            "workspace": AnyEncodable(ws),
            "indexed": AnyEncodable(false)
        ]
        let completed = ACPToolCallUpdateWire(call_id: call.call_id, status: .completed, output: AnyEncodable(payload), error: nil, _meta: nil)
        let completedNote = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCallUpdate(completed))
        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: completedNote)), let jtext = String(data: out, encoding: .utf8) {
            client.send(text: jtext)
        }
    }

    private func handleSessionPrompt(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Parse request
        guard let data = try? JSONSerialization.data(withJSONObject: rawDict),
              let req = try? JSONDecoder().decode(JSONRPC.Request<ACP.Agent.SessionPromptRequest>.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid session/prompt parameters") { text in
                client.send(text: text)
            }
            return
        }

        let sessionId = req.params.session_id
        let sidStr = sessionId.value

        // Extract prompt text from content blocks
        let promptText = req.params.content.compactMap { block -> String? in
            if case .text(let textBlock) = block {
                return textBlock.text
            }
            return nil
        }.joined(separator: "\n")

        guard !promptText.isEmpty else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Empty prompt") { text in
                client.send(text: text)
            }
            return
        }

        OpenAgentsLog.server.debug("handleSessionPrompt session=\(sidStr, privacy: .public) prompt=\(promptText.prefix(50), privacy: .private)...")

        // Get mode (defaults to .default_mode if not set)
        let mode = modeBySession[sidStr] ?? .default_mode

        // Get provider from registry
        guard let provider = await agentRegistry.provider(for: mode) else {
            JsonRpcRouter.sendError(id: id, code: -32601, message: "No agent provider for mode: \(mode.rawValue)") { text in
                client.send(text: text)
            }
            return
        }

        // Check if provider is available
        guard await provider.isAvailable() else {
            JsonRpcRouter.sendError(id: id, code: -32601, message: "\(provider.displayName) is not available. Please install the required CLI.") { text in
                client.send(text: text)
            }
            return
        }

        // Build context
        let context = AgentContext(
            workingDirectory: workingDirectory,
            mcpServers: nil,
            client: client,
            metadata: [:]
        )

        // Get update hub
        guard let updateHub = self.updateHub else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Update hub not initialized") { text in
                client.send(text: text)
            }
            return
        }

        // Check if we have an existing handle (resume scenario)
        if let existingHandle = await agentRegistry.handle(for: sessionId) {
            // Resume existing session
            do {
                try await provider.resume(
                    sessionId: sessionId,
                    prompt: promptText,
                    handle: existingHandle,
                    context: context,
                    updateHub: updateHub
                )

                // Send success response
                JsonRpcRouter.sendResponse(id: id, result: ["status": "resumed"]) { text in
                    client.send(text: text)
                }
            } catch {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to resume: \(error.localizedDescription)") { text in
                    client.send(text: text)
                }
            }
        } else {
            // Start new session
            do {
                let handle = try await provider.start(
                    sessionId: sessionId,
                    prompt: promptText,
                    context: context,
                    updateHub: updateHub
                )

                // Store handle
                await agentRegistry.setHandle(handle, for: sessionId)

                // Send success response
                JsonRpcRouter.sendResponse(id: id, result: ["status": "started"]) { text in
                    client.send(text: text)
                }
            } catch {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to start: \(error.localizedDescription)") { text in
                    client.send(text: text)
                }
            }
        }
    }

    private func handleSessionSetMode(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Update current mode and broadcast a current_mode_update
        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
        let sid = ACPSessionId(sidStr)

        // Store mode for this session
        let modeStr = (params?["mode_id"] as? String) ?? ACPSessionModeId.default_mode.rawValue
        let modeId = ACPSessionModeId(rawValue: modeStr) ?? .default_mode
        self.modeBySession[sidStr] = modeId

        // Echo a current_mode_update
        let update = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: modeId))
        let note = ACP.Client.SessionNotificationWire(session_id: sid, update: update)
        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: note)), let jtext = String(data: out, encoding: .utf8) {
            OpenAgentsLog.server.debug("send rpc notify method=\(ACPRPC.sessionUpdate) text=\(jtext, privacy: .public)")
            client.send(text: jtext)
        }

        // Respond with an empty SetSessionModeResponse
        let result = ACP.Agent.SetSessionModeResponse()
        JsonRpcRouter.sendResponse(id: id, result: result) { responseText in
            OpenAgentsLog.server.debug("send rpc result method=\(ACPRPC.sessionSetMode) id=\(id.value) text=\(responseText, privacy: .public)")
            client.send(text: responseText)
        }
    }

    private func handleSessionCancel(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Parse session ID from params
        guard let sessionIdStr = params?["session_id"] as? String else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing session_id") { text in
                client.send(text: text)
            }
            return
        }

        let sessionId = ACPSessionId(sessionIdStr)

        // Get handle
        guard let handle = await agentRegistry.handle(for: sessionId) else {
            // No active session, send success anyway
            JsonRpcRouter.sendResponse(id: id, result: ["status": "no_active_session"]) { text in
                client.send(text: text)
            }
            return
        }

        // Get provider
        guard let provider = await agentRegistry.provider(for: handle.mode) else {
            JsonRpcRouter.sendError(id: id, code: -32601, message: "Provider not found for mode: \(handle.mode.rawValue)") { text in
                client.send(text: text)
            }
            return
        }

        // Cancel the session
        await provider.cancel(sessionId: sessionId, handle: handle)

        // Remove handle from registry
        await agentRegistry.removeHandle(for: sessionId)

        // Send success response
        JsonRpcRouter.sendResponse(id: id, result: ["status": "cancelled"]) { text in
            client.send(text: text)
        }
    }

    private func handleFsReadTextFile(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct ReqURI: Codable { let session_id: ACPSessionId; let uri: String }
        struct ReqPath: Codable { let session_id: ACPSessionId; let path: String; let line: UInt32?; let limit: UInt32? }
        struct Resp: Codable { let content: String }

        if let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p) {
            var text: String? = nil
            var attempted: String? = nil
            if let req = try? JSONDecoder().decode(ReqPath.self, from: d) {
                attempted = req.path
                text = DesktopWebSocketServer.readText(fromURI: req.path)
            } else if let req = try? JSONDecoder().decode(ReqURI.self, from: d) {
                attempted = req.uri
                text = DesktopWebSocketServer.readText(fromURI: req.uri)
            }
            if let text = text {
                JsonRpcRouter.sendResponse(id: id, result: Resp(content: text)) { responseText in
                    OpenAgentsLog.server.debug("send rpc result method=\(ACPRPC.fsReadTextFile) id=\(id.value) bytes=\(responseText.utf8.count)")
                    client.send(text: responseText)
                }
            } else {
                let msg = "Resource not found"
                JsonRpcRouter.sendError(id: id, code: -32002, message: attempted.map { "\(msg): \($0)" } ?? msg) { text in
                    client.send(text: text)
                }
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
        }
    }

    private func handleFsWriteTextFile(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct ReqPath: Codable { let session_id: ACPSessionId; let path: String; let content: String }
        struct ReqURI: Codable { let session_id: ACPSessionId; let uri: String; let text: String }
        struct Resp: Codable { let _meta: [String:String]? }

        if let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p) {
            var ok = false
            if let req = try? JSONDecoder().decode(ReqPath.self, from: d) {
                ok = DesktopWebSocketServer.writeText(toURI: req.path, text: req.content)
            } else if let req = try? JSONDecoder().decode(ReqURI.self, from: d) {
                ok = DesktopWebSocketServer.writeText(toURI: req.uri, text: req.text)
            }
            if ok {
                JsonRpcRouter.sendResponse(id: id, result: Resp(_meta: nil)) { responseText in
                    OpenAgentsLog.server.debug("send rpc result method=\(ACPRPC.fsWriteTextFile) id=\(id.value) bytes=\(responseText.utf8.count)")
                    client.send(text: responseText)
                }
            } else {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Write failed") { text in
                    client.send(text: text)
                }
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
        }
    }

    private func handleTerminalRun(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct Req: Codable { let session_id: ACPSessionId; let command: [String]; let cwd: String?; let env: [String:String]?; let output_byte_limit: Int? }
        struct Resp: Codable { let output: String; let truncated: Bool; let exit_status: Int32? }

        guard let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let req = try? JSONDecoder().decode(Req.self, from: d) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        let result = DesktopWebSocketServer.runCommand(req.command, cwd: req.cwd, env: req.env, limit: req.output_byte_limit)
        JsonRpcRouter.sendResponse(id: id, result: Resp(output: result.output, truncated: result.truncated, exit_status: result.exitStatus)) { responseText in
            OpenAgentsLog.server.debug("send rpc result method=\(ACPRPC.terminalRun) id=\(id.value) bytes=\(responseText.utf8.count)")
            client.send(text: responseText)
        }
    }

    private func handleOrchestrationStart(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Phase 2: On-device Foundation Models orchestrator
        OpenAgentsLog.server.info("recv orchestrate.explore.start")

        // Parse request
        guard let p = rawDict["params"],
              let d = try? JSONSerialization.data(withJSONObject: p),
              let req = try? JSONDecoder().decode(OrchestrateExploreStartRequest.self, from: d) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        // Generate session and plan IDs
        let sessionId = ACPSessionId(UUID().uuidString)
        let planId = UUID().uuidString

        // Send immediate response
        let response = OrchestrateExploreStartResponse(
            session_id: sessionId.value,
            plan_id: planId,
            status: "started"
        )
        JsonRpcRouter.sendResponse(id: id, result: response) { responseText in
            OpenAgentsLog.server.debug("send rpc result method=orchestrate.explore.start id=\(id.value)")
            client.send(text: responseText)
        }

        // Start orchestration async (non-blocking)
        if #available(macOS 26.0, *) {
            Task.detached {
                await self.runOrchestration(
                    request: req,
                    sessionId: sessionId,
                    planId: planId,
                    client: client
                )
            }
        } else {
            // Foundation Models not available on this OS version
            OpenAgentsLog.server.warning("orchestrate.explore.start requires macOS 26.0+")
            JsonRpcRouter.sendError(
                id: id,
                code: -32603,
                message: "orchestrate.explore.start requires macOS 26.0+ for Foundation Models"
            ) { text in
                client.send(text: text)
            }
        }
    }

    public func setTinyvexDb(path: String) {
        if let db = try? TinyvexDbLayer(path: path) {
            self.tinyvexDb = db
            // Initialize SessionUpdateHub with broadcast callback
            self.updateHub = SessionUpdateHub(tinyvexDb: db) { [weak self] notificationJSON in
                await self?.broadcastToClients(notificationJSON)
            }
            // Initialize HistoryApi for Tinyvex queries
            self.historyApi = HistoryApi(tinyvexDb: db)
            OpenAgentsLog.server.info("Tinyvex DB attached at \(path, privacy: .private)")
        } else {
            OpenAgentsLog.server.error("Failed to open Tinyvex DB at \(path, privacy: .private)")
        }
    }

    /// Start listening on given port
    public func start(port: UInt16, advertiseService: Bool = true, serviceName: String? = nil, serviceType: String = BridgeConfig.serviceType) throws {
        let params = NWParameters(tls: nil)
        let wsOptions = NWProtocolWebSocket.Options()
        params.defaultProtocolStack.applicationProtocols.insert(wsOptions, at: 0)
        params.acceptLocalOnly = false
        listener = try NWListener(using: params, on: NWEndpoint.Port(rawValue: port)!)
        // Advertise Bonjour service for discovery when supported
        if advertiseService {
            #if os(macOS)
            if #available(macOS 12.0, *) {
                let name = serviceName ?? Host.current().localizedName ?? "OpenAgents"
                listener?.service = NWListener.Service(name: name, type: serviceType, domain: "local.")
            }
            #endif
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

    private func findAndTailSessionFile(sessionId: ACPSessionId, client: Client) {
        let sidStr = sessionId.value
        let chosenMode = modeBySession[sidStr] ?? .default_mode
        var picked: URL? = nil
        var provider = "claude-code"
        if chosenMode == .codex {
            let codexBase = CodexScanner.defaultBaseDir()
            picked = CodexScanner.listRecentTopN(at: codexBase, topK: 1).first
            provider = "codex"
        }
        if picked == nil {
            let claudeBase = ClaudeCodeScanner.defaultBaseDir()
            picked = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: 1).first
            provider = "claude-code"
        }

        guard let file = picked else {
            OpenAgentsLog.server.warning("no session file found for \(sessionId.value) (mode=\(chosenMode.rawValue))")
            return
        }

        sessionFiles[sidStr] = file

        OpenAgentsLog.server.debug("tailing session file: \(file.path, privacy: .private)")

        // Update the live tailer to watch this file
        queue.async { [weak self] in
            guard let self = self else { return }
            self.tailURL = file
            self.tailSessionId = sessionId
            self.tailProvider = provider
            let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber)?.uint64Value ?? 0
            self.tailOffset = size
            self.tailBuffer.removeAll(keepingCapacity: true)

            // Start tailer if not already running
            if self.tailTimer == nil {
                let timer = DispatchSource.makeTimerSource(queue: self.queue)
                timer.schedule(deadline: .now() + .milliseconds(100), repeating: .milliseconds(100))
                timer.setEventHandler { [weak self] in self?.pollTail() }
                self.tailTimer = timer
                timer.resume()
                OpenAgentsLog.server.debug("tailer started for session \(sidStr)")
            }
        }
    }

    private func sendErrorMessage(sessionId: ACPSessionId, message: String, client: Client) {
        let errorChunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ Error: \(message)")))
        let errorUpdate = ACP.Client.SessionUpdate.agentMessageChunk(errorChunk)
        let note = ACP.Client.SessionNotificationWire(session_id: sessionId, update: errorUpdate)

        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: note)),
           let jtext = String(data: out, encoding: .utf8) {
            OpenAgentsLog.server.error("send error message: \(message)")
            client.send(text: jtext)
        }
    }

    private func handleTextMessage(_ text: String, from client: Client) {
        if !client.isHandshakeComplete {
            OpenAgentsLog.server.debug("recv handshake text=\(text, privacy: .public)")
            if let data = text.data(using: .utf8), let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any], (dict["jsonrpc"] as? String) == "2.0", let method = dict["method"] as? String, method == "initialize" {
                let inIdStr: String = {
                    if let idNum = dict["id"] as? Int { return String(idNum) }
                    if let idStr = dict["id"] as? String { return idStr }
                    return "1"
                }()
                OpenAgentsLog.server.info("recv rpc request method=initialize id=\(inIdStr)")
                let idVal: JSONRPC.ID
                if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                else { idVal = JSONRPC.ID("1") }
                // Include working directory in _meta if available
                OpenAgentsLog.server.debug("workingDirectory=\(self.workingDirectory?.path ?? "nil", privacy: .private)")
                let meta: [String: AnyEncodable]? = {
                    if let wd = self.workingDirectory {
                        OpenAgentsLog.server.debug("including working_directory in _meta: \(wd.path, privacy: .private)")
                        return ["working_directory": AnyEncodable(wd.path)]
                    }
                    OpenAgentsLog.server.debug("workingDirectory is nil, not including in _meta")
                    return nil
                }()
                let resp = ACP.Agent.InitializeResponse(protocol_version: "0.2.2", agent_capabilities: .init(), auth_methods: [], agent_info: ACP.Agent.Implementation(name: "openagents-mac", title: "OpenAgents macOS", version: "0.1.0"), _meta: meta)
                if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: resp)), let jtext = String(data: out, encoding: .utf8) {
                    OpenAgentsLog.server.debug("send rpc result method=initialize id=\(inIdStr) text=\(jtext, privacy: .public)")
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
            OpenAgentsLog.server.debug("recv payload text=\(text, privacy: .public)")
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               (dict["jsonrpc"] as? String) == "2.0" {
                // JSON-RPC request/notification
                if let method = dict["method"] as? String {
                    if let idAny = dict["id"] {
                        let idStr = (idAny as? String) ?? (idAny as? Int).map(String.init) ?? "1"
                        OpenAgentsLog.server.debug("recv rpc request method=\(method) id=\(idStr)")
                    } else {
                        OpenAgentsLog.server.debug("recv rpc notify method=\(method)")
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
                            OpenAgentsLog.server.warning("method '\(method)' not handled by any registered handler")
                        }
                    }
                }
                return
            }
            OpenAgentsLog.server.debug("ignoring non JSON-RPC payload")
        }
    }

    // MARK: - Orchestration (Phase 2)

    @available(macOS 26.0, *)
    private func runOrchestration(
        request: OrchestrateExploreStartRequest,
        sessionId: ACPSessionId,
        planId: String,
        client: Client
    ) async {
        OpenAgentsLog.orchestration.info("Starting exploration of \(request.root, privacy: .private)")

        // Policy defaults to on-device only
        let policy = request.policy ?? ExplorationPolicy(allow_external_llms: false, allow_network: false)

        // Create stream handler that sends updates via WebSocket
        let streamHandler: ACPUpdateStreamHandler = { [weak self] update in
            await self?.sendSessionUpdate(sessionId: sessionId, update: update)
        }

        // Create and run orchestrator
        let orchestrator = ExploreOrchestrator(
            workspaceRoot: request.root,
            goals: request.goals ?? ["Understand workspace structure"],
            policy: policy,
            streamHandler: streamHandler
        )

        do {
            let summary = try await orchestrator.startExploration()
            OpenAgentsLog.orchestration.info("Completed exploration: \(summary.repo_name)")

            // Send summary as final agent message (no leading title/header)
            var sections: [String] = []

            sections.append("**Repository:** \(summary.repo_name)")

            if false { // removed deterministic languages block {
                let langs = summary.languages.map { "\($0.key): \($0.value) lines" }.joined(separator: ", ")
                sections.append("**Languages:** \(langs)")
            }

            if false { // removed deterministic entrypoints block {
                sections.append("**Entry points:** \(summary.entrypoints.joined(separator: ", "))")
            }

            if false { // removed deterministic top files block {
                sections.append("\n**Top Files:**")
                summary.top_files.forEach { sections.append("- `\($0)`") }
            }

            if false { // removed deterministic insights block {
                sections.append("\n**Insights:**")
                summary.followups.forEach { sections.append("• \($0)") }
            }

            // FM-only: inferred intent with visible status (tool call) while it prepares
            if #available(macOS 26.0, *) {
                let callId = UUID().uuidString
                // Announce analysis start
                let toolCall = ACPToolCallWire(
                    call_id: callId,
                    name: "fm.analysis",
                    arguments: nil,
                    _meta: ["stage": AnyEncodable("summary")] 
                )
                await sendSessionUpdate(sessionId: sessionId, update: .toolCall(toolCall))
                let started = ACPToolCallUpdateWire(
                    call_id: callId,
                    status: .started,
                    output: nil,
                    error: nil,
                    _meta: ["progress": AnyEncodable(0.0)]
                )
                await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(started))

                if let analysis = await orchestrator.fmAnalysis(), !analysis.text.isEmpty {
                    let heading = (analysis.source == .sessionAnalyze) ? "\n**Intent From Session History:**" : "\n**Inferred Intent (FM):**"
                    sections.append(heading)
                    sections.append(analysis.text)
                    let outPayload: [String: AnyEncodable] = [
                        "summary_bytes": AnyEncodable(analysis.text.utf8.count),
                        "source": AnyEncodable(analysis.source.rawValue)
                    ]
                    let completed = ACPToolCallUpdateWire(
                        call_id: callId,
                        status: .completed,
                        output: AnyEncodable(outPayload),
                        error: nil,
                        _meta: nil
                    )
                    await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(completed))
                } else {
                    let completed = ACPToolCallUpdateWire(
                        call_id: callId,
                        status: .completed,
                        output: AnyEncodable(["summary_bytes": 0]),
                        error: nil,
                        _meta: nil
                    )
                    await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(completed))
                }
            }

            let summaryText = sections.joined(separator: "\n")

            let summaryChunk = ACP.Client.ContentChunk(
                content: .text(.init(text: summaryText))
            )
            await sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(summaryChunk))
        } catch {
            OpenAgentsLog.orchestration.error("Error: \(error)")

            // Send error message
            let errorChunk = ACP.Client.ContentChunk(
                content: .text(.init(text: "❌ Orchestration failed: \(error.localizedDescription)"))
            )
            await sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(errorChunk))
        }
    }

    /// Send session update via WebSocket (delegates to SessionUpdateHub)
    private func sendSessionUpdate(
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
                OpenAgentsLog.server.error("Failed to encode session update")
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
}

// MARK: - Service helpers & error sender
extension DesktopWebSocketServer {
    fileprivate static func urlFromURI(_ uri: String) -> URL? {
        if let u = URL(string: uri), u.scheme != nil { return u }
        return URL(fileURLWithPath: uri)
    }
    fileprivate static func readText(fromURI uri: String) -> String? {
        guard let url = urlFromURI(uri) else { return nil }
        if url.isFileURL {
            return try? String(contentsOf: url, encoding: .utf8)
        }
        return nil
    }
    fileprivate static func writeText(toURI uri: String, text: String) -> Bool {
        guard let url = urlFromURI(uri) else { return false }
        if url.isFileURL {
            do { try text.data(using: .utf8)!.write(to: url); return true } catch { return false }
        }
        return false
    }
    fileprivate static func runCommand(_ cmd: [String], cwd: String?, env: [String:String]?, limit: Int?) -> (output: String, truncated: Bool, exitStatus: Int32?) {
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
    fileprivate static func sendJSONRPCError(client: DesktopWebSocketServer.Client, id: JSONRPC.ID, code: Int, message: String) {
        let idAny: Any = Int(id.value) ?? id.value
        let envelope: [String: Any] = [
            "jsonrpc": "2.0",
            "id": idAny,
            "error": ["code": code, "message": message]
        ]
        if let out = try? JSONSerialization.data(withJSONObject: envelope), let text = String(data: out, encoding: .utf8) {
            OpenAgentsLog.server.debug("send rpc error id=\(id.value) code=\(code) bytes=\(text.utf8.count)")
            client.send(text: text)
        }
    }
    fileprivate static func tailJSONLLines(at url: URL, maxBytes: Int, maxLines: Int) -> [String] {
        guard let fh = try? FileHandle(forReadingFrom: url) else { return [] }
        defer { try? fh.close() }
        let chunk = 64 * 1024
        let fileSize = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue ?? 0
        var offset = fileSize
        var buffer = Data()
        var totalRead = 0
        while offset > 0 && totalRead < maxBytes {
            let toRead = min(chunk, offset)
            offset -= toRead
            try? fh.seek(toOffset: UInt64(offset))
            let data = (try? fh.read(upToCount: toRead)) ?? Data()
            buffer.insert(contentsOf: data, at: 0)
            totalRead += data.count
            if buffer.count >= maxBytes { break }
        }
        var text = String(data: buffer, encoding: .utf8) ?? String(decoding: buffer, as: UTF8.self)
        if !text.hasSuffix("\n") { text.append("\n") }
        var lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
        if lines.count > maxLines { lines = Array(lines.suffix(maxLines)) }
        return lines
    }

    /// Prune oversized payloads from Codex JSONL lines before sending to mobile clients.
    /// - Strategy:
    ///   - For ACP `tool_result` objects, strip or summarize the `result`/`output` fields.
    ///   - For `session/update` with `tool_call_update`, replace `output` with a short summary.
    ///   - Cap any single string value to a reasonable length to avoid overflows.
    ///   - Operates best‑effort; if parsing fails, returns the original line.
    fileprivate static func pruneHeavyPayloads(in lines: [String]) -> [String] {
        return lines.map { line in
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return line }

            let pruned = pruneObject(obj)
            if let out = try? JSONSerialization.data(withJSONObject: pruned, options: []),
               let text = String(data: out, encoding: .utf8) {
                return text
            }
            return line
        }
    }

    /// Reduce the list of lines so that the total UTF‑8 byte count stays under `limit`.
    /// Keeps the most recent lines by taking from the end.
    fileprivate static func capTotalBytes(_ lines: [String], limit: Int) -> [String] {
        var total = 0
        var kept: [String] = []
        for line in lines.reversed() {
            let bytes = line.utf8.count + 1 // include newline/commas overhead
            if total + bytes > limit { break }
            total += bytes
            kept.append(line)
        }
        return kept.reversed()
    }

    private static func pruneObject(_ obj: [String: Any]) -> [String: Any] {
        var out = obj

        // Handle JSON‑RPC session/update envelopes containing tool_call_update
        if let method = out["method"] as? String, method == ACPRPC.sessionUpdate,
           let params = out["params"] as? [String: Any] {
            var p = params
            if let upd = p["update"] as? [String: Any],
               let kind = upd["sessionUpdate"] as? String, kind == "tool_call_update" {
                if var tc = upd["tool_call_update"] as? [String: Any] {
                    if let output = tc["output"], let s = summarize(value: output) { tc["output"] = s }
                    p["update"] = merge(upd, ["tool_call_update": tc])
                }
            }
            out["params"] = pruneRecursive(p)
            return capStrings(out)
        }

        // Generic Codex item payloads under `item`, `msg`, or `payload`
        let keys = ["item", "msg", "payload"]
        for k in keys {
            if var inner = out[k] as? [String: Any] {
                inner = pruneToolShapes(inner)
                out[k] = pruneRecursive(inner)
            }
        }

        return capStrings(out)
    }

    private static func pruneToolShapes(_ obj: [String: Any]) -> [String: Any] {
        var o = obj
        let type = (o["type"] as? String)?.lowercased()
        if type == "tool_result" {
            // Remove heavy result payload; keep ok/error/call_id
            if let r = o["result"], let s = summarize(value: r) { o["result"] = s } else { o.removeValue(forKey: "result") }
            if let outp = o["output"], let s = summarize(value: outp) { o["output"] = s }
        }
        if type == "tool_call_update" {
            if var tc = o["tool_call_update"] as? [String: Any] {
                if let outv = tc["output"], let s = summarize(value: outv) { tc["output"] = s }
                o["tool_call_update"] = tc
            }
        }
        return o
    }

    private static func pruneRecursive(_ any: Any) -> Any {
        switch any {
        case let dict as [String: Any]:
            var out: [String: Any] = [:]
            for (k, v) in dict { out[k] = pruneRecursive(v) }
            return capStrings(out)
        case let arr as [Any]:
            return arr.map { pruneRecursive($0) }
        case let s as String:
            return capString(s)
        default:
            return any
        }
    }

    private static func summarize(value: Any) -> String? {
        // Produce a small placeholder indicating that content was omitted
        if let s = value as? String { return s.count > 256 ? "(omitted \(s.count) chars)" : s }
        if let d = try? JSONSerialization.data(withJSONObject: value, options: []), d.count > 256 {
            return "(omitted \(d.count) bytes)"
        }
        return nil
    }

    private static func capStrings(_ dict: [String: Any]) -> [String: Any] {
        var out = dict
        for (k, v) in dict {
            if let s = v as? String { out[k] = capString(s) }
            else if let d = v as? [String: Any] { out[k] = capStrings(d) }
            else if let a = v as? [Any] { out[k] = a.map { pruneRecursive($0) } }
        }
        return out
    }

    private static func capString(_ s: String) -> String {
        let limit = 4096
        if s.utf8.count <= limit { return s }
        let idx = s.index(s.startIndex, offsetBy: min(s.count, 3000))
        let prefix = String(s[..<idx])
        return prefix + "… (truncated)"
    }

    private static func merge(_ a: [String: Any], _ b: [String: Any]) -> [String: Any] {
        var out = a
        for (k,v) in b { out[k] = v }
        return out
    }

    // MARK: - Typed updates assembly (Provider JSONL -> ACP SessionUpdate)
    fileprivate static func makeTypedUpdates(from lines: [String], provider: String = "codex") -> [ACP.Client.SessionUpdate] {
        var updates: [ACP.Client.SessionUpdate] = []

        // Claude Code uses a different structure - translate full conversation at once
        if provider == "claude-code" {
            let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "desktop"))
            for event in thread.events {
                // Map ACPEvent to SessionUpdate
                if let m = event.message {
                    let text = m.parts.compactMap { part -> String? in
                        if case let .text(tt) = part { return tt.text } else { return nil }
                    }.joined(separator: "\n")
                    guard !text.isEmpty else { continue }
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                    if m.role == .user {
                        updates.append(.userMessageChunk(chunk))
                    } else if m.role == .assistant {
                        // Check isThinking metadata to distinguish thinking from regular response
                        if m.isThinking == true {
                            updates.append(.agentThoughtChunk(chunk))
                        } else {
                            updates.append(.agentMessageChunk(chunk))
                        }
                    }
                }
                if let call = event.tool_call {
                    let args = jsonValueToAnyDict(call.arguments)
                    let wire = ACPToolCallWire(call_id: call.id, name: call.tool_name, arguments: args, _meta: nil)
                    updates.append(.toolCall(wire))
                }
                if let res = event.tool_result {
                    let status: ACPToolCallUpdateWire.Status = res.ok ? .completed : .error
                    let output = res.result.map { jsonValueToAnyEncodable($0) }
                    let wire = ACPToolCallUpdateWire(call_id: res.call_id, status: status, output: output, error: res.error, _meta: nil)
                    updates.append(.toolCallUpdate(wire))
                }
                if let ps = event.plan_state {
                    let entries = (ps.steps ?? []).map { ACPPlanEntry(content: $0, priority: .medium, status: .in_progress, _meta: nil) }
                    let plan = ACPPlan(entries: entries, _meta: nil)
                    updates.append(.plan(plan))
                }
            }
            return updates
        }

        // Codex: translate line-by-line (original logic)
        for line in lines {
            guard let data = line.data(using: .utf8),
                  let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { continue }
            // Reasoning first: emit agent_thought_chunk instead of assistant message
            if isReasoningLine(obj) {
                if let text = textFromTranslatedLine(line) {
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                    updates.append(.agentThoughtChunk(chunk))
                }
                continue
            }
            // For everything else, use translator then map to updates
            let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: "desktop"))
            if let m = t.events.compactMap({ $0.message }).first {
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(tt) = part { return tt.text } else { return nil }
                }.joined(separator: "\n")
                guard !text.isEmpty else { continue }
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
                updates.append(m.role == .user ? .userMessageChunk(chunk) : .agentMessageChunk(chunk))
                continue
            }
            if let call = t.events.compactMap({ $0.tool_call }).first {
                let args = jsonValueToAnyDict(call.arguments)
                let wire = ACPToolCallWire(call_id: call.id, name: call.tool_name, arguments: args, _meta: nil)
                updates.append(.toolCall(wire))
                continue
            }
            if let res = t.events.compactMap({ $0.tool_result }).first {
                let status: ACPToolCallUpdateWire.Status = res.ok ? .completed : .error
                let output = res.result.map { jsonValueToAnyEncodable($0) }
                let wire = ACPToolCallUpdateWire(call_id: res.call_id, status: status, output: output, error: res.error, _meta: nil)
                updates.append(.toolCallUpdate(wire))
                continue
            }
            if let ps = t.events.compactMap({ $0.plan_state }).first {
                // Represent plan state as a full plan update (simplified): convert steps to ACPPlan
                let entries = (ps.steps ?? []).map { ACPPlanEntry(content: $0, priority: .medium, status: .in_progress, _meta: nil) }
                let plan = ACPPlan(entries: entries, _meta: nil)
                updates.append(.plan(plan))
                continue
            }
        }
        return updates
    }

    private static func isReasoningLine(_ obj: [String: Any]) -> Bool {
        let t = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        if t == "event_msg", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any]),
           ((item["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if t == "response_item", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "reasoning" { return true }
        return false
    }

    private static func textFromTranslatedLine(_ line: String) -> String? {
        let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: "desktop"))
        guard let m = t.events.compactMap({ $0.message }).first else { return nil }
        let text = m.parts.compactMap { part -> String? in
            if case let .text(tt) = part { return tt.text } else { return nil }
        }.joined(separator: "\n")
        return text.isEmpty ? nil : text
    }

    private static func jsonValueToAnyEncodable(_ v: JSONValue) -> AnyEncodable {
        switch v {
        case .string(let s): return AnyEncodable(s)
        case .number(let n): return AnyEncodable(n)
        case .bool(let b): return AnyEncodable(b)
        case .null: return AnyEncodable(Optional<String>.none as String?)
        case .array(let arr):
            let encArr: [AnyEncodable] = arr.map { jsonValueToAnyEncodable($0) }
            return AnyEncodable(encArr)
        case .object(let obj):
            var encDict: [String: AnyEncodable] = [:]
            for (k, vv) in obj { encDict[k] = jsonValueToAnyEncodable(vv) }
            return AnyEncodable(encDict)
        }
    }

    // MARK: - Live tailer
    private func startLiveTailerIfNeeded() {
        guard tailTimer == nil else { return }

        // Prioritize Claude Code sessions, fall back to Codex only if none exist
        let claudeBase = ClaudeCodeScanner.defaultBaseDir()
        let claudeFiles = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: 1)

        var file: URL?
        var provider: String = "claude-code"
        var base: URL = claudeBase
        var tid: String?

        if let clf = claudeFiles.first {
            // Use Claude Code session
            file = clf
            provider = "claude-code"
            base = claudeBase
            tid = ClaudeCodeScanner.scanForSessionID(clf) ?? ClaudeCodeScanner.relativeId(for: clf, base: claudeBase)
        } else {
            // Fallback to Codex only if no Claude Code sessions exist
            let codexBase = CodexScanner.defaultBaseDir()
            let codexFiles = CodexScanner.listRecentTopN(at: codexBase, topK: 1)
            if let cf = codexFiles.first {
                file = cf
                provider = "codex"
                base = codexBase
                tid = CodexScanner.scanForThreadID(cf) ?? CodexScanner.relativeId(for: cf, base: codexBase)
            }
        }

        guard let file = file, let tid = tid else { return }

        tailURL = file
        tailSessionId = ACPSessionId(tid)
        tailProvider = provider
        let size = (try? FileManager.default.attributesOfItem(atPath: file.path)[.size] as? NSNumber)?.uint64Value ?? 0
        tailOffset = size
        tailBuffer.removeAll(keepingCapacity: true)

        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + .milliseconds(500), repeating: .milliseconds(500))
        timer.setEventHandler { [weak self] in self?.pollTail() }
        tailTimer = timer
        timer.resume()
        OpenAgentsLog.server.debug("tailer started file=\(file.path, privacy: .private) session=\(tid) provider=\(provider)")
    }
    private func stopLiveTailer() {
        tailTimer?.cancel(); tailTimer = nil
        tailURL = nil; tailSessionId = nil; tailProvider = nil; tailOffset = 0; tailBuffer.removeAll()
        OpenAgentsLog.server.debug("tailer stopped")
    }
    private func pollTail() {
        guard let url = tailURL, let sid = tailSessionId else { return }
        guard let fh = try? FileHandle(forReadingFrom: url) else { return }
        defer { try? fh.close() }
        let fileSize = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.uint64Value ?? 0
        if fileSize <= tailOffset { return }
        let toRead = Int(fileSize - tailOffset)
        do {
            try fh.seek(toOffset: tailOffset)
            let data = try fh.read(upToCount: toRead) ?? Data()
            tailOffset = fileSize
            if !data.isEmpty { tailBuffer.append(data) }
        } catch { return }
        var text = String(data: tailBuffer, encoding: .utf8) ?? String(decoding: tailBuffer, as: UTF8.self)
        var keepRemainder = Data()
        if !text.hasSuffix("\n"), let lastNewline = text.lastIndex(of: "\n") {
            let remainder = String(text[text.index(after: lastNewline)..<text.endIndex])
            keepRemainder = remainder.data(using: .utf8) ?? Data()
            text = String(text[..<text.index(after: lastNewline)])
        }
        let newLines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
        tailBuffer = keepRemainder
        if newLines.isEmpty { return }
        let updates = DesktopWebSocketServer.makeTypedUpdates(from: newLines, provider: tailProvider ?? "codex")
        guard !updates.isEmpty else { return }
        for u in updates { Task { await self.sendSessionUpdate(sessionId: sid, update: u) } }
    }
    
    // MARK: - Codex exec --json event mapper
    private static func jsonValueToAnyDict(_ v: JSONValue) -> [String: AnyEncodable]? {
        if case .object(let obj) = v {
            var out: [String: AnyEncodable] = [:]
            for (k, vv) in obj { out[k] = jsonValueToAnyEncodable(vv) }
            return out
        }
        return nil
    }
}
#endif
