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

        /// Send a text message to the client
        public func send(text: String) {
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
    private let token: String
    public weak var delegate: DesktopWebSocketServerDelegate?

    /// Initialize server with a token to validate Hello messages
    public init(token: String) {
        self.token = token
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

    private func handleTextMessage(_ text: String, from client: Client) {
        if !client.isHandshakeComplete {
            print("[Bridge][server] recv handshake text=\(text)")
            // Try ACP JSON-RPC initialize first
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
                let resp = ACP.Agent.InitializeResponse(protocol_version: "0.7.0", agent_capabilities: .init(), auth_methods: [], agent_info: ACP.Agent.Implementation(name: "openagents-mac", title: "OpenAgents macOS", version: "0.1.0"), _meta: nil)
                if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: resp)), let jtext = String(data: out, encoding: .utf8) {
                    print("[Bridge][server] send rpc result method=initialize id=\(inIdStr) bytes=\(jtext.utf8.count)")
                    client.send(text: jtext)
                }
                client.isHandshakeComplete = true
                DispatchQueue.main.async { [weak self] in
                    guard let self = self else { return }
                    self.delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: true)
                }
            } else if let data = text.data(using: .utf8), let json = try? JSONSerialization.jsonObject(with: data, options: []) as? [String: Any] {
                // Legacy token Hello fallback
                let type = (json["type"] as? String) ?? ""
                let receivedToken = (json["token"] as? String) ?? ""
                guard type == "Hello" || (!receivedToken.isEmpty) else {
                    DispatchQueue.main.async { [weak self] in
                        guard let self = self else { return }
                        self.delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: false)
                    }
                    client.close(); clients.remove(client); return
                }
                if receivedToken == token {
                    print("[Bridge][server] Hello received; token ok")
                    client.isHandshakeComplete = true
                    let ackObj: [String: Any] = ["type": "HelloAck", "token": token]
                    if let ackData = try? JSONSerialization.data(withJSONObject: ackObj, options: []), let ackText = String(data: ackData, encoding: .utf8) {
                        client.send(text: ackText)
                    }
                    DispatchQueue.main.async { [weak self] in
                        guard let self = self else { return }
                        self.delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: true)
                    }
                } else {
                    print("[Bridge][server] Hello token mismatch")
                    DispatchQueue.main.async { [weak self] in
                        guard let self = self else { return }
                        self.delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: false)
                    }
                    client.close(); clients.remove(client)
                }
            }
        } else {
            // Handle envelopes after handshake
            print("[Bridge][server] recv payload text=\(text)")
            // Support both JSON-RPC (ACP) and legacy envelopes
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
                            print("[Bridge][server] send rpc result method=threads/list id=\(idVal.value) count=\(items.count) bytes=\(jtext.utf8.count)")
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
                            print("[Bridge][server] send rpc result method=\(ACPRPC.sessionNew) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.sessionPrompt:
                        // Echo a tiny streamed agent message back as a demo
                        let params = dict["params"] as? [String: Any]
                        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
                        let sid = ACPSessionId(sidStr)
                        let msg = ACP.Client.SessionUpdate.agentMessageChunk(
                            ACP.Client.ContentChunk(content: .text("OK — processing your request…"))
                        )
                        let note = ACP.Client.SessionNotificationWire(session_id: sid, update: msg)
                        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: note)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc notify method=\(ACPRPC.sessionUpdate) update=agent_message_chunk session_id=\(sid.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                        // Also send AvailableCommandsUpdate and CurrentModeUpdate
                        let cmds = [ACP.Client.AvailableCommand(name: "create_plan", description: "Propose a plan", input: .unstructured(hint: "What should we plan?"))]
                        let ac = ACP.Client.SessionUpdate.availableCommandsUpdate(.init(available_commands: cmds))
                        let acNote = ACP.Client.SessionNotificationWire(session_id: sid, update: ac)
                        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: acNote)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc notify method=\(ACPRPC.sessionUpdate) update=available_commands_update session_id=\(sid.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                        let cm = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: .default_mode))
                        let cmNote = ACP.Client.SessionNotificationWire(session_id: sid, update: cm)
                        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: cmNote)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc notify method=\(ACPRPC.sessionUpdate) update=current_mode_update session_id=\(sid.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                        // Also respond to the request with an empty object
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        struct EmptyResult: Codable {}
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: EmptyResult())), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=\(ACPRPC.sessionPrompt) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.sessionSetMode:
                        // Update current mode and broadcast a current_mode_update
                        let params = dict["params"] as? [String: Any]
                        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
                        let sid = ACPSessionId(sidStr)
                        // Echo a current_mode_update
                        let modeStr = (params?["mode_id"] as? String) ?? ACPSessionModeId.default_mode.rawValue
                        let update = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: ACPSessionModeId(rawValue: modeStr) ?? .default_mode))
                        let note = ACP.Client.SessionNotificationWire(session_id: sid, update: update)
                        if let out = try? JSONEncoder().encode(JSONRPC.Notification(method: ACPRPC.sessionUpdate, params: note)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc notify method=\(ACPRPC.sessionUpdate) update=current_mode_update session_id=\(sid.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                        // Respond with an empty SetSessionModeResponse
                        let idVal: JSONRPC.ID
                        if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                        else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                        else { idVal = JSONRPC.ID("1") }
                        let result = ACP.Agent.SetSessionModeResponse()
                        if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: result)), let jtext = String(data: out, encoding: .utf8) {
                            print("[Bridge][server] send rpc result method=\(ACPRPC.sessionSetMode) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                            client.send(text: jtext)
                        }
                    case ACPRPC.sessionCancel:
                        // Notification: log and optionally send a status note
                        print("[Bridge][server] session/cancel received")
                        // No response required
                    case ACPRPC.fsReadTextFile:
                        struct Req: Codable { let session_id: ACPSessionId; let uri: String }
                        struct Resp: Codable { let text: String }
                        let idVal: JSONRPC.ID = {
                            if let idNum = dict["id"] as? Int { return JSONRPC.ID(String(idNum)) }
                            if let idStr = dict["id"] as? String { return JSONRPC.ID(idStr) }
                            return JSONRPC.ID("1")
                        }()
                        if let p = dict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let req = try? JSONDecoder().decode(Req.self, from: d) {
                            let text: String? = DesktopWebSocketServer.readText(fromURI: req.uri)
                            if let text = text, let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: Resp(text: text))), let jtext = String(data: out, encoding: .utf8) {
                                print("[Bridge][server] send rpc result method=\(ACPRPC.fsReadTextFile) id=\(idVal.value) bytes=\(jtext.utf8.count)")
                                client.send(text: jtext)
                            } else {
                                DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32002, message: "Resource not found: \(req.uri)")
                            }
                        } else {
                            DesktopWebSocketServer.sendJSONRPCError(client: client, id: idVal, code: -32602, message: "Invalid params")
                        }
                    case ACPRPC.fsWriteTextFile:
                        struct Req: Codable { let session_id: ACPSessionId; let uri: String; let text: String }
                        struct Resp: Codable { let ok: Bool }
                        let idVal: JSONRPC.ID = {
                            if let idNum = dict["id"] as? Int { return JSONRPC.ID(String(idNum)) }
                            if let idStr = dict["id"] as? String { return JSONRPC.ID(idStr) }
                            return JSONRPC.ID("1")
                        }()
                        if let p = dict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let req = try? JSONDecoder().decode(Req.self, from: d) {
                            if DesktopWebSocketServer.writeText(toURI: req.uri, text: req.text), let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: Resp(ok: true))), let jtext = String(data: out, encoding: .utf8) {
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
                    default:
                        break
                    }
                }
                return
            }
            print("[Bridge][server] ignoring non JSON-RPC payload")
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
            if let more = try? pipe.fileHandleForReading.readToEnd(), let more = more { data.append(more) }
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
}
#endif
