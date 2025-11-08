import Foundation
import Network

#if os(macOS)
// MARK: - Tinyvex WebSocket server (Network.framework)

public actor TinyvexServer {
    public struct Config { public let port: UInt16; public let dbPath: String; public let features: TinyvexFeatures; public init(port: UInt16 = 9099, dbPath: String, features: TinyvexFeatures = .init()) { self.port = port; self.dbPath = dbPath; self.features = features } }

    private let config: Config
    private let db: TinyvexDbLayer
    private var listener: NWListener?
    private var connections: [ObjectIdentifier: TinyvexConnection] = [:]
    private var nextSeq: Int64 = 1

    public init(config: Config) throws {
        self.config = config
        self.db = try TinyvexDbLayer(path: config.dbPath)
    }

    public func start() throws {
        let params = NWParameters(tls: nil)
        let wsOptions = NWProtocolWebSocket.Options()
        wsOptions.autoReplyPing = true
        params.defaultProtocolStack.applicationProtocols = [wsOptions]
        guard let nwPort = NWEndpoint.Port(rawValue: config.port) else {
            throw NSError(domain: "TinyvexServer", code: -1, userInfo: [NSLocalizedDescriptionKey: "Invalid port: \(config.port)"])
        }
        let listener = try NWListener(using: params, on: nwPort)
        listener.newConnectionHandler = { [weak self] conn in
            guard let self else { return }
            Task { await self.accept(connection: conn) }
        }
        listener.start(queue: .main)
        self.listener = listener
    }

    public func stop() {
        listener?.cancel(); listener = nil
        for (_, c) in connections { c.cancel() }
        connections.removeAll()
    }

    private func accept(connection: NWConnection) {
        let handler = TinyvexConnection(connection: connection, server: self)
        connections[ObjectIdentifier(handler)] = handler
        handler.start()
    }

    fileprivate func remove(_ conn: TinyvexConnection) {
        connections.removeValue(forKey: ObjectIdentifier(conn))
    }

    // MARK: - Routing
    fileprivate func handleRequest(id: JSONRPC.ID, method: String, payload: Data, send: @escaping (Data) -> Void) async {
        switch method {
        case TinyvexMethods.connect:
            let now = Int64(Date().timeIntervalSince1970 * 1000)
            let result = TinyvexConnectResult(serverVersion: "0.1.0", nowTs: now, features: config.features, sessionId: nil)
            send(encode(JSONRPC.Response(id: id, result: result)))

        case TinyvexMethods.historySessionUpdates:
            do {
                let req = try JSONDecoder().decode(JSONRPC.Request<TinyvexHistoryParams>.self, from: payload)
                let p = req.params
                let rows = try await db.history(sessionId: p.session_id, sinceSeq: p.since_seq, sinceTs: p.since_ts, limit: p.limit)
                // Return array of ACP.Client.SessionNotificationWire
                let wires = try rows.map { row -> ACP.Client.SessionNotificationWire in
                    let update = try JSONDecoder().decode(ACP.Client.SessionUpdate.self, from: Data(row.update_json.utf8))
                    return .init(session_id: ACPSessionId(p.session_id), update: update)
                }
                send(encode(JSONRPC.Response(id: id, result: wires)))
            } catch {
                sendError(id: id, code: -32602, message: "Invalid params: \(error)", send: send)
            }

        case TinyvexMethods.initialize:
            do {
                let req = try JSONDecoder().decode(JSONRPC.Request<ACP.Agent.InitializeRequest>.self, from: payload)
                let caps = ACP.Agent.AgentCapabilities(load_session: false, prompt_capabilities: .init(), mcp_capabilities: .init())
                let resp = ACP.Agent.InitializeResponse(protocol_version: req.params.protocol_version, agent_capabilities: caps, auth_methods: [], agent_info: .init(name: "Tinyvex", title: "Tinyvex", version: "0.1.0"))
                send(encode(JSONRPC.Response(id: id, result: resp)))
            } catch { sendError(id: id, code: -32602, message: "Invalid params: \(error)", send: send) }

        case TinyvexMethods.sessionNew:
            do {
                _ = try JSONDecoder().decode(JSONRPC.Request<ACP.Agent.SessionNewRequest>.self, from: payload)
                let sid = ACPSessionId(UUID().uuidString)
                send(encode(JSONRPC.Response(id: id, result: ACP.Agent.SessionNewResponse(session_id: sid))))
            } catch { sendError(id: id, code: -32602, message: "Invalid params: \(error)", send: send) }

        case TinyvexMethods.sessionPrompt:
            do {
                let req = try JSONDecoder().decode(JSONRPC.Request<ACP.Agent.SessionPromptRequest>.self, from: payload)
                // Simulated agent: echo back the prompt text as an agent_message_chunk
                let sessionId = req.params.session_id.value
                let now = Int64(Date().timeIntervalSince1970 * 1000)
                let content = ACP.Client.ContentBlock.text(.init(text: "OK: \(req.params.content.count) blocks"))
                let wire = ACP.Client.SessionNotificationWire(session_id: ACPSessionId(sessionId), update: .agentMessageChunk(.init(content: content)))
                let updateJSON = String(data: try JSONEncoder().encode(wire.update), encoding: .utf8) ?? "{}"
                try await db.appendEvent(sessionId: sessionId, seq: nextSeq, ts: now, updateJSON: updateJSON)
                nextSeq += 1
                // Fan-out
                await broadcastACPUpdate(wire)
                struct OK: Codable { let ok: Bool }
                send(encode(JSONRPC.Response(id: id, result: OK(ok: true))))
            } catch { sendError(id: id, code: -32602, message: "Invalid params: \(error)", send: send) }

        default:
            sendError(id: id, code: -32601, message: "Method not found: \(method)", send: send)
        }
    }

    private func encode<R: Encodable>(_ r: JSONRPC.Response<R>) -> Data {
        (try? JSONEncoder().encode(r)) ?? Data("{\"jsonrpc\":\"2.0\"}".utf8)
    }

    private func sendError(id: JSONRPC.ID, code: Int, message: String, send: (Data) -> Void) {
        let err = JSONRPC.ErrorResponse(id: id, error: .init(code: code, message: message, data: nil))
        if let data = try? JSONEncoder().encode(err) { send(data) }
    }
}

// MARK: - Connection wrapper

final class TinyvexConnection {
    private let connection: NWConnection
    private weak var serverRef: TinyvexServer?
    private let queue = DispatchQueue(label: "tvx.conn")

    init(connection: NWConnection, server: TinyvexServer) {
        self.connection = connection
        self.serverRef = server
    }

    func start() {
        connection.stateUpdateHandler = { [weak self] state in
            if case .failed = state { self?.cancel() }
            if case .cancelled = state { self?.cancel() }
        }
        connection.start(queue: queue)
        receive()
    }

    func cancel() {
        connection.cancel()
        if let server = serverRef { Task { await server.remove(self) } }
    }

    private func receive() {
        connection.receiveMessage { [weak self] (data, ctx, isComplete, err) in
            guard let self else { return }
            if let err { self.cancel(); return }
            if let data, data.count > 0 { self.handle(data: data) }
            self.receive()
        }
    }

    private func handle(data: Data) {
        // Attempt to decode a generic request to get method + id
        do {
            let obj = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            guard let method = obj?["method"] as? String else { return }
            if let idAny = obj?["id"] {
                let id: JSONRPC.ID
                if let i = idAny as? Int { id = JSONRPC.ID(String(i)) }
                else if let s = idAny as? String { id = JSONRPC.ID(s) }
                else { id = JSONRPC.ID(String(describing: idAny)) }
                Task { [weak self] in
                    guard let self, let server = self.serverRef else { return }
                    await server.handleRequest(id: id, method: method, payload: data, send: { [weak self] out in self?.send(out) })
                }
            } else {
                // Notification: currently only ACP session/update from server, ignore others from client
            }
        } catch {
            // Ignore malformed
        }
    }

    func send(_ data: Data) { connection.send(content: data, completion: .contentProcessed { _ in }) }
}

extension TinyvexServer {
    fileprivate func broadcastACPUpdate(_ wire: ACP.Client.SessionNotificationWire) async {
        // Build notification envelope
        struct Notif: Encodable { let jsonrpc = "2.0"; let method = TinyvexMethods.sessionUpdate; let params: ACP.Client.SessionNotificationWire }
        let notif = Notif(params: wire)
        guard let payload = try? JSONEncoder().encode(notif) else { return }
        for (_, c) in connections { c.send(payload) }
    }
}
#endif
