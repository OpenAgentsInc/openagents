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
            print("[Bridge][server] send text=\(text)")
            let metadata = NWProtocolWebSocket.Metadata(opcode: .text)
            let context = NWConnection.ContentContext(identifier: "textContext", metadata: [metadata])
            let data = text.data(using: .utf8) ?? Data()
            connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    print("[Bridge][server] send error: \(sendError)")
                }
            })
        }

        /// Send a Ping frame
        public func sendPing() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .ping)
            let context = NWConnection.ContentContext(identifier: "pingContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    print("[Bridge][server] ping send error: \(sendError)")
                }
            })
        }

        /// Send a Pong frame
        public func sendPong() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .pong)
            let context = NWConnection.ContentContext(identifier: "pongContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { sendError in
                if let sendError = sendError {
                    print("[Bridge][server] pong send error: \(sendError)")
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
    public var useProviderTailer: Bool = false // disable global tailer by default

    // Session mode selection (per session)
    private var modeBySession: [String: ACPSessionModeId] = [:]

    // Active agent sessions
    private var activeProcesses: [String: Process] = [:]  // session_id -> Process
    private var sessionFiles: [String: URL] = [:]  // session_id -> JSONL file URL

    public init() {}

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

    private func launchAgentProcess(sessionId: ACPSessionId, prompt: String, client: Client) {
        let sidStr = sessionId.value

        // Check if we already have a process for this session
        if let existing = activeProcesses[sidStr], existing.isRunning {
            print("[Bridge][server] session \(sidStr) already has running process")
            return
        }

        // Pick agent binary based on session mode
        let chosenMode = modeBySession[sidStr] ?? .default_mode
        var binary: String?
        var agentName = "claude"
        switch chosenMode {
        case .codex:
            binary = findCodexCLI()
            agentName = "codex"
        case .claude_code, .default_mode:
            binary = findClaudeCLI()
            agentName = "claude"
        }
        guard let execPath = binary else {
            let msg = chosenMode == .codex ? "Codex CLI not found" : "Claude CLI not found"
            print("[Bridge][server] ERROR: \(msg)")
            sendErrorMessage(sessionId: sessionId, message: "\(msg). Please install the \(agentName) CLI.", client: client)
            return
        }

        print("[Bridge][server] launching agent (mode=\(chosenMode.rawValue)): \(execPath)")

        // Create process
        let process = Process()
        process.executableURL = URL(fileURLWithPath: execPath)

        // Build CLI arguments per provider
        switch chosenMode {
        case .codex:
            process.arguments = [prompt]
            print("[Bridge][server] arguments (codex): \"\(prompt)\"")
        case .claude_code, .default_mode:
            // Claude CLI supports --continue to resume latest session
            process.arguments = ["--continue", prompt]
            print("[Bridge][server] arguments (claude): --continue \"\(prompt)\"")
        }

        // Set up environment with node in PATH
        var environment = ProcessInfo.processInfo.environment

        // Add fnm node to PATH
        let binDir = (execPath as NSString).deletingLastPathComponent
        let fnmNodePath = (binDir as NSString).deletingLastPathComponent

        if let existingPath = environment["PATH"] {
            environment["PATH"] = "\(binDir):\(fnmNodePath):\(existingPath)"
        } else {
            environment["PATH"] = "\(binDir):\(fnmNodePath):/usr/local/bin:/usr/bin:/bin"
        }

        print("[Bridge][server] PATH for process: \(environment["PATH"] ?? "none")")
        process.environment = environment

        // Set up output/error pipes (for logging)
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        // Store process
        activeProcesses[sidStr] = process

        // Start process
        do {
            try process.run()
            print("[Bridge][server] agent process started pid=\(process.processIdentifier)")

            // Monitor stdout/stderr for debugging
            stdoutPipe.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                if !data.isEmpty, let text = String(data: data, encoding: .utf8) {
                    print("[Bridge][agent stdout] \(text)")
                }
            }

            stderrPipe.fileHandleForReading.readabilityHandler = { handle in
                let data = handle.availableData
                if !data.isEmpty, let text = String(data: data, encoding: .utf8) {
                    print("[Bridge][agent stderr] \(text)")
                }
            }

            // Do not auto-attach a global provider tailer; it can pick the wrong file.
            // If explicitly enabled, attach a session-scoped tailer in a follow-up pass.
            if self.useProviderTailer {
                DispatchQueue.global(qos: .userInitiated).asyncAfter(deadline: .now() + 1.0) { [weak self] in
                    self?.findAndTailSessionFile(sessionId: sessionId, client: client)
                }
            }

        } catch {
            print("[Bridge][server] ERROR: failed to launch agent: \(error)")
            activeProcesses.removeValue(forKey: sidStr)
            sendErrorMessage(sessionId: sessionId, message: "Failed to launch agent: \(error.localizedDescription)", client: client)
        }
    }

    private func findClaudeCLI() -> String? {
        print("[Bridge][server] === STARTING CLAUDE CLI SEARCH ===")

        // Try fnm-managed paths first (where npm install -g puts things)
        let fnmPaths = [
            "\(NSHomeDirectory())/.local/state/fnm_multishells",
            "\(NSHomeDirectory())/.fnm/node-versions"
        ]

        for basePath in fnmPaths {
            print("[Bridge][server] searching fnm path: \(basePath)")
            if let found = searchForClaudeRecursive(in: basePath, maxDepth: 4) {
                print("[Bridge][server] ✓ found claude at: \(found)")
                return found
            }
        }

        // Try common locations for claude CLI
        let paths = [
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
            "\(NSHomeDirectory())/bin/claude",
            "\(NSHomeDirectory())/.local/bin/claude",
            "\(NSHomeDirectory())/.npm-global/bin/claude",
            "/usr/bin/claude"
        ]

        for path in paths {
            print("[Bridge][server] checking: \(path)")
            if FileManager.default.fileExists(atPath: path) {
                print("[Bridge][server] ✓ found claude at: \(path)")
                return path
            }
        }

        print("[Bridge][server] trying login shells...")

        // Try using shell to find claude (sources .zshrc/.bashrc for PATH)
        let shells = ["/bin/zsh", "/bin/bash"]
        for shell in shells {
            print("[Bridge][server] trying shell: \(shell)")
            let process = Process()
            process.executableURL = URL(fileURLWithPath: shell)
            process.arguments = ["-l", "-c", "which claude"]
            let pipe = Pipe()
            let errPipe = Pipe()
            process.standardOutput = pipe
            process.standardError = errPipe

            do {
                try process.run()
                process.waitUntilExit()

                print("[Bridge][server] \(shell) exit status: \(process.terminationStatus)")

                if process.terminationStatus == 0 {
                    let data = try? pipe.fileHandleForReading.readToEnd()
                    if let data = data, let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty {
                        print("[Bridge][server] ✓ found claude via \(shell): \(path)")
                        return path
                    } else {
                        print("[Bridge][server] \(shell) returned empty output")
                    }
                } else {
                    print("[Bridge][server] \(shell) failed with status \(process.terminationStatus)")
                }

                // Log error output for debugging
                if let errData = try? errPipe.fileHandleForReading.readToEnd(),
                   let errText = String(data: errData, encoding: .utf8), !errText.isEmpty {
                    print("[Bridge][server] \(shell) stderr: \(errText)")
                }
            } catch {
                print("[Bridge][server] ✗ failed to run \(shell): \(error)")
            }
        }

        print("[Bridge][server] ✗ claude not found in any location")
        return nil
    }

    private func findCodexCLI() -> String? {
        // Allow explicit override via env var
        let env = ProcessInfo.processInfo.environment
        if let override = env["OPENAGENTS_CODEX_CLI"], !override.isEmpty, FileManager.default.fileExists(atPath: override) {
            print("[Bridge][server] ✓ using OPENAGENTS_CODEX_CLI=\(override)")
            return override
        }

        // Try fnm-managed paths first (mirroring Claude search)
        let fnmPaths = [
            "\(NSHomeDirectory())/.local/state/fnm_multishells",
            "\(NSHomeDirectory())/.fnm/node-versions"
        ]
        for basePath in fnmPaths {
            if let found = searchForBinaryRecursive(in: basePath, basename: "codex", maxDepth: 4) {
                print("[Bridge][server] ✓ found codex at: \(found)")
                return found
            }
        }

        // Common locations
        let paths = [
            "/usr/local/bin/codex",
            "/opt/homebrew/bin/codex",
            "\(NSHomeDirectory())/bin/codex",
            "\(NSHomeDirectory())/.local/bin/codex",
            "\(NSHomeDirectory())/.npm-global/bin/codex",
            "/usr/bin/codex"
        ]
        for p in paths { if FileManager.default.fileExists(atPath: p) { return p } }

        // Fallback to login shells (try several likely binaries)
        for shell in ["/bin/zsh", "/bin/bash"] {
            for candidate in ["codex", "oa", "openai"] {
                do {
                    let process = Process()
                    process.executableURL = URL(fileURLWithPath: shell)
                    process.arguments = ["-l", "-c", "which \(candidate)"]
                    let out = Pipe(); process.standardOutput = out; process.standardError = Pipe()
                    try process.run(); process.waitUntilExit()
                    if process.terminationStatus == 0 {
                        let data = out.fileHandleForReading.readDataToEndOfFile()
                        if let s = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines), !s.isEmpty {
                            print("[Bridge][server] ✓ found candidate for Codex CLI (\(candidate)): \(s)")
                            return s
                        }
                    }
                } catch { continue }
            }
        }
        return nil
    }

    private func searchForBinaryRecursive(in basePath: String, basename: String, maxDepth: Int, currentDepth: Int = 0) -> String? {
        guard currentDepth < maxDepth else { return nil }
        guard FileManager.default.fileExists(atPath: basePath) else { return nil }
        let candidate = "\(basePath)/bin/\(basename)"
        if FileManager.default.fileExists(atPath: candidate) { return candidate }
        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: basePath) else { return nil }
        for item in contents {
            let fullPath = "\(basePath)/\(item)"
            if let found = searchForBinaryRecursive(in: fullPath, basename: basename, maxDepth: maxDepth, currentDepth: currentDepth + 1) {
                return found
            }
        }
        return nil
    }

    private func searchForClaudeRecursive(in basePath: String, maxDepth: Int, currentDepth: Int = 0) -> String? {
        guard currentDepth < maxDepth else { return nil }
        guard FileManager.default.fileExists(atPath: basePath) else { return nil }

        let claudePath = "\(basePath)/bin/claude"
        if FileManager.default.fileExists(atPath: claudePath) {
            return claudePath
        }

        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: basePath) else { return nil }

        for item in contents {
            let fullPath = "\(basePath)/\(item)"
            if let found = searchForClaudeRecursive(in: fullPath, maxDepth: maxDepth, currentDepth: currentDepth + 1) {
                return found
            }
        }

        return nil
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
            print("[Bridge][server] WARNING: no session file found for \(sessionId.value) (mode=\(chosenMode.rawValue))")
            return
        }

        sessionFiles[sidStr] = file

        print("[Bridge][server] tailing session file: \(file.path)")

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
                print("[Bridge][server] tailer started for session \(sidStr)")
            }
        }
    }

    private func sendErrorMessage(sessionId: ACPSessionId, message: String, client: Client) {
        let errorChunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ Error: \(message)")))
        let errorUpdate = ACP.Client.SessionUpdate.agentMessageChunk(errorChunk)
        let note = ACP.Client.SessionNotificationWire(session_id: sessionId, update: errorUpdate)

        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: note)),
           let jtext = String(data: out, encoding: .utf8) {
            print("[Bridge][server] send error message: \(message)")
            client.send(text: jtext)
        }
    }

    private func handleTextMessage(_ text: String, from client: Client) {
        if !client.isHandshakeComplete {
            print("[Bridge][server] recv handshake text=\(text)")
            if let data = text.data(using: .utf8), let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any], (dict["jsonrpc"] as? String) == "2.0", let method = dict["method"] as? String, method == "initialize" {
                let inIdStr: String = {
                    if let idNum = dict["id"] as? Int { return String(idNum) }
                    if let idStr = dict["id"] as? String { return idStr }
                    return "1"
                }()
                print("[Bridge][server] recv rpc request method=initialize id=\(inIdStr)")
                let idVal: JSONRPC.ID
                if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                else { idVal = JSONRPC.ID("1") }
                // Include working directory in _meta if available
                print("[Bridge][server] workingDirectory=\(self.workingDirectory?.path ?? "nil")")
                let meta: [String: AnyEncodable]? = {
                    if let wd = self.workingDirectory {
                        print("[Bridge][server] including working_directory in _meta: \(wd.path)")
                        return ["working_directory": AnyEncodable(wd.path)]
                    }
                    print("[Bridge][server] workingDirectory is nil, not including in _meta")
                    return nil
                }()
                let resp = ACP.Agent.InitializeResponse(protocol_version: "0.2.2", agent_capabilities: .init(), auth_methods: [], agent_info: ACP.Agent.Implementation(name: "openagents-mac", title: "OpenAgents macOS", version: "0.1.0"), _meta: meta)
                if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: resp)), let jtext = String(data: out, encoding: .utf8) {
                    print("[Bridge][server] send rpc result method=initialize id=\(inIdStr) text=\(jtext)")
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
            print("[Bridge][server] recv payload text=\(text)")
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               (dict["jsonrpc"] as? String) == "2.0" {
                // JSON-RPC request/notification
                if let method = dict["method"] as? String {
                    if let idAny = dict["id"] {
                        let idStr = (idAny as? String) ?? (idAny as? Int).map(String.init) ?? "1"
                        print("[Bridge][server] recv rpc request method=\(method) id=\(idStr)")
                    } else {
                        print("[Bridge][server] recv rpc notify method=\(method)")
                    }
                    switch method {
                    case "threads/list":
                        struct Params: Codable { let topK: Int? }
                        let topK: Int = {
                            if let p = dict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let pr = try? JSONDecoder().decode(Params.self, from: d) { return pr.topK ?? 10 }
                            return 10
                        }()
                        let limit = min(10, max(1, topK))
                        let base = CodexScanner.defaultBaseDir()
                        let urls = CodexScanner.listRecentTopN(at: base, topK: limit)
                        var items = urls.map { CodexScanner.makeSummary(for: $0, base: base) }
                        items.sort { ($0.last_message_ts ?? $0.updated_at) > ($1.last_message_ts ?? $1.updated_at) }
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        struct Result: Codable { let items: [ThreadSummary] }
                        let result = Result(items: items)
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=threads/list id=\(idVal.value) text=\(jtext)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.sessionNew:
                        // Generate a new session id
                        let sid = ACPSessionId(UUID().uuidString)
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        let result = ACP.Agent.SessionNewResponse(session_id: sid)
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=\(ACPRPC.sessionNew) id=\(idVal.value) text=\(jtext)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.sessionPrompt:
                        // Launch agent process with the prompt
                        let params = dict["params"] as? [String: Any]
                        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
                        let sid = ACPSessionId(sidStr)

                        // Extract prompt content
                        var promptText = ""
                        if let content = params?["content"] as? [[String: Any]] {
                            for block in content {
                                if let text = block["text"] as? String {
                                    promptText += text
                                }
                            }
                        } else if let prompt = params?["prompt"] as? [[String: Any]] {
                            for block in prompt {
                                if let text = block["text"] as? String {
                                    promptText += text
                                }
                            }
                        }

                        print("[Bridge][server] session/prompt sid=\(sidStr) prompt_length=\(promptText.count)")

                        // Launch agent process
                        self.launchAgentProcess(sessionId: sid, prompt: promptText, client: client)

                        // Respond to the request with empty result immediately
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        struct EmptyResult: Codable {}
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: EmptyResult())), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=\(ACPRPC.sessionPrompt) id=\(idVal.value) text=\(jtext)")
                            client.send(text: jtext)
                        }
                    case "thread/load_latest":
                        // Deprecated raw JSONL hydrate; prefer thread/load_latest_typed
                        struct LatestResult: Codable { let id: String; let lines: [String] }
                        let params = dict["params"] as? [String: Any]
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
                            let idVal: JSONRPC.ID
                            if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                            else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                            else { idVal = JSONRPC.ID("1") }
                            let result = LatestResult(id: tid, lines: lines)
                            if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                                print("[Bridge][server] send rpc result method=thread/load_latest id=\(idVal.value) bytes=\(jtext.utf8.count)")
                                client.send(text: jtext)
                            }
                        } else {
                            let idVal: JSONRPC.ID
                            if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                            else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                            else { idVal = JSONRPC.ID("1") }
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32002, message: "No threads found")
                        }
                    case "thread/load_latest_typed":
                        struct LatestTypedResult: Codable { let id: String; let updates: [ACP.Client.SessionUpdate] }

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

                            let idVal: JSONRPC.ID
                            if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                            else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                            else { idVal = JSONRPC.ID("1") }
                            let result = LatestTypedResult(id: tid, updates: updates)
                            if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                                print("[Bridge][server] send rpc result method=thread/load_latest_typed id=\(idVal.value) provider=\(provider) bytes=\(jtext.utf8.count)")
                                client.send(text: jtext)
                            }
                        } else {
                            let idVal: JSONRPC.ID
                            if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                            else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                            else { idVal = JSONRPC.ID("1") }
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32002, message: "No threads found")
                        }
                    case "index.status":
                        // Minimal Phase-1: respond and emit a tiny synthetic stream for visibility
                        // Params may contain session_id to stream under
                        let params = dict["params"] as? [String: Any]
                        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
                        let sid = ACPSessionId(sidStr)
                        struct StatusResult: Codable { let workspace_path: String; let exists: Bool; let files_indexed: Int?; let chunks: Int? }
                        // Default workspace path (env override TEST_WORKSPACE_ROOT)
                        let ws = ProcessInfo.processInfo.environment["TEST_WORKSPACE_ROOT"] ?? "\(NSHomeDirectory())/code/openagents"
                        let exists = FileManager.default.fileExists(atPath: ws)
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        let result = StatusResult(workspace_path: ws, exists: exists, files_indexed: nil, chunks: nil)
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=index.status id=\(idVal.value) text=\(jtext)")
                            client.send(text: jtext)
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
                    case ACPRPC.sessionSetMode:
                        // Update current mode and broadcast a current_mode_update
                        let params = dict["params"] as? [String: Any]
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
                            print("[Bridge][server] send rpc notify method=\(ACPRPC.sessionUpdate) text=\(jtext)")
                            client.send(text: jtext)
                        }
                        // Respond with an empty SetSessionModeResponse
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        let result = ACP.Agent.SetSessionModeResponse()
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=\(ACPRPC.sessionSetMode) id=\(idVal.value) text=\(jtext)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.sessionCancel:
                        // Notification: log and optionally send a status note
                        print("[Bridge][server] session/cancel received")
                        // No response required
                    case ACPRPC.fsReadTextFile:
                        struct ReqURI: Codable { let session_id: ACPSessionId; let uri: String }
                        struct ReqPath: Codable { let session_id: ACPSessionId; let path: String; let line: UInt32?; let limit: UInt32? }
                        struct Resp: Codable { let content: String }
                        let idVal: JSONRPC.ID = {
                            if let idNum = dict["id"] as? Int { return JSONRPC.ID(String(idNum)) }
                            if let idStr = dict["id"] as? String { return JSONRPC.ID(idStr) }
                            return JSONRPC.ID("1")
                        }()
                        if let p = dict["params"], let d = try? JSONSerialization.data(withJSONObject: p) {
                            var text: String? = nil
                            var attempted: String? = nil
                            if let req = try? JSONDecoder().decode(ReqPath.self, from: d) {
                                attempted = req.path
                                text = DesktopWebSocketServer.readText(fromURI: req.path)
                            } else if let req = try? JSONDecoder().decode(ReqURI.self, from: d) {
                                attempted = req.uri
                                text = DesktopWebSocketServer.readText(fromURI: req.uri)
                            }
                            if let text = text, let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: Resp(content: text))), let jtext = String(data: out, encoding: .utf8) {
                                print("[Bridge][server] send rpc result method=\(ACPRPC.fsReadTextFile) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                                client.send(text: jtext)
                            } else {
                                let msg = "Resource not found"
                                DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32002, message: attempted.map { "\(msg): \($0)" } ?? msg)
                            }
                        } else {
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32602, message: "Invalid params")
                        }
                    case ACPRPC.fsWriteTextFile:
                        struct ReqPath: Codable { let session_id: ACPSessionId; let path: String; let content: String }
                        struct ReqURI: Codable { let session_id: ACPSessionId; let uri: String; let text: String }
                        struct Resp: Codable { let _meta: [String:String]? }
                        let idVal: JSONRPC.ID = {
                            if let idNum = dict["id"] as? Int { return JSONRPC.ID(String(idNum)) }
                            if let idStr = dict["id"] as? String { return JSONRPC.ID(idStr) }
                            return JSONRPC.ID("1")
                        }()
                        if let p = dict["params"], let d = try? JSONSerialization.data(withJSONObject: p) {
                            var ok = false
                            if let req = try? JSONDecoder().decode(ReqPath.self, from: d) {
                                ok = DesktopWebSocketServer.writeText(toURI: req.path, text: req.content)
                            } else if let req = try? JSONDecoder().decode(ReqURI.self, from: d) {
                                ok = DesktopWebSocketServer.writeText(toURI: req.uri, text: req.text)
                            }
                            if ok, let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: Resp(_meta: nil))), let jtext = String(data: out, encoding: .utf8) {
                                print("[Bridge][server] send rpc result method=\(ACPRPC.fsWriteTextFile) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                                client.send(text: jtext)
                            } else {
                                DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32603, message: "Write failed")
                            }
                        } else {
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32602, message: "Invalid params")
                        }
                    case ACPRPC.terminalRun:
                        struct Req: Codable { let session_id: ACPSessionId; let command: [String]; let cwd: String?; let env: [String:String]?; let output_byte_limit: Int? }
                        struct Resp: Codable { let output: String; let truncated: Bool; let exit_status: Int32? }
                        let idVal: JSONRPC.ID = {
                            if let idNum = dict["id"] as? Int { return JSONRPC.ID(String(idNum)) }
                            if let idStr = dict["id"] as? String { return JSONRPC.ID(idStr) }
                            return JSONRPC.ID("1")
                        }()
                        guard let p = dict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let req = try? JSONDecoder().decode(Req.self, from: d) else {
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32602, message: "Invalid params"); break
                        }
                        let result = DesktopWebSocketServer.runCommand(req.command, cwd: req.cwd, env: req.env, limit: req.output_byte_limit)
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: Resp(output: result.output, truncated: result.truncated, exit_status: result.exitStatus))), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=\(ACPRPC.terminalRun) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.orchestrateExploreStart:
                        // Phase 2: On-device Foundation Models orchestrator
                        print("[Bridge][server] recv orchestrate.explore.start")
                        let idVal: JSONRPC.ID = {
                            if let idNum = dict["id"] as? Int { return JSONRPC.ID(String(idNum)) }
                            if let idStr = dict["id"] as? String { return JSONRPC.ID(idStr) }
                            return JSONRPC.ID("1")
                        }()

                        // Parse request
                        guard let p = dict["params"],
                              let d = try? JSONSerialization.data(withJSONObject: p),
                              let req = try? JSONDecoder().decode(OrchestrateExploreStartRequest.self, from: d) else {
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32602, message: "Invalid params")
                            break
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
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: response)),
                           let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=orchestrate.explore.start id=\(idVal.value)")
                            client.send(text: jtext)
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
                            print("[Bridge][server] orchestrate.explore.start requires macOS 26.0+")
                            DesktopWebSocketServer.sendJSONRPCError(
                                client: client,
                                id: idVal,
                                code: -32603,
                                message: "orchestrate.explore.start requires macOS 26.0+ for Foundation Models"
                            )
                        }
                    default:
                        break
                    }
                }
                return
            }
            print("[Bridge][server] ignoring non JSON-RPC payload")
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
        print("[Orchestrator] Starting exploration of \(request.root)")

        // Policy defaults to on-device only
        let policy = request.policy ?? ExplorationPolicy(allow_external_llms: false, allow_network: false)

        // Create stream handler that sends updates via WebSocket
        let streamHandler: ACPUpdateStreamHandler = { [weak self] update in
            await self?.sendSessionUpdate(sessionId: sessionId, update: update, client: client)
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
            print("[Orchestrator] Completed exploration: \(summary.repo_name)")

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
                await sendSessionUpdate(sessionId: sessionId, update: .toolCall(toolCall), client: client)
                let started = ACPToolCallUpdateWire(
                    call_id: callId,
                    status: .started,
                    output: nil,
                    error: nil,
                    _meta: ["progress": AnyEncodable(0.0)]
                )
                await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(started), client: client)

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
                    await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(completed), client: client)
                } else {
                    let completed = ACPToolCallUpdateWire(
                        call_id: callId,
                        status: .completed,
                        output: AnyEncodable(["summary_bytes": 0]),
                        error: nil,
                        _meta: nil
                    )
                    await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(completed), client: client)
                }
            }

            let summaryText = sections.joined(separator: "\n")

            let summaryChunk = ACP.Client.ContentChunk(
                content: .text(.init(text: summaryText))
            )
            await sendSessionUpdate(
                sessionId: sessionId,
                update: .agentMessageChunk(summaryChunk),
                client: client
            )
        } catch {
            print("[Orchestrator] Error: \(error)")

            // Send error message
            let errorChunk = ACP.Client.ContentChunk(
                content: .text(.init(text: "❌ Orchestration failed: \(error.localizedDescription)"))
            )
            await sendSessionUpdate(
                sessionId: sessionId,
                update: .agentMessageChunk(errorChunk),
                client: client
            )
        }
    }

    /// Send session update via WebSocket
    private func sendSessionUpdate(
        sessionId: ACPSessionId,
        update: ACP.Client.SessionUpdate,
        client: Client
    ) async {
        let notification = ACP.Client.SessionNotificationWire(
            session_id: sessionId,
            update: update,
            _meta: nil
        )

        guard let data = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: notification)),
              let jtext = String(data: data, encoding: .utf8) else {
            print("[Orchestrator] Failed to encode session update")
            return
        }

        // Send on main queue to avoid background thread warnings
        await MainActor.run {
            client.send(text: jtext)
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
            print("[Bridge][server] send rpc error id=\(id.value) code=\(code) bytes=\(text.utf8.count)")
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
        print("[Bridge][server] tailer started file=\(file.path) session=\(tid) provider=\(provider)")
    }
    private func stopLiveTailer() {
        tailTimer?.cancel(); tailTimer = nil
        tailURL = nil; tailSessionId = nil; tailProvider = nil; tailOffset = 0; tailBuffer.removeAll()
        print("[Bridge][server] tailer stopped")
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
        for u in updates {
            let note = ACP.Client.SessionNotificationWire(session_id: sid, update: u)
            if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: note)), let jtext = String(data: out, encoding: .utf8) {
                for c in clients where c.isHandshakeComplete { c.send(text: jtext) }
            }
        }
    }
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
