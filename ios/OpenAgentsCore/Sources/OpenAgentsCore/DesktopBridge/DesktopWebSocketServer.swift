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
                let idVal: JSONRPC.ID
                if let idNum = dict["id"] as? Int { idVal = JSONRPC.ID(String(idNum)) }
                else if let idStr = dict["id"] as? String { idVal = JSONRPC.ID(idStr) }
                else { idVal = JSONRPC.ID("1") }
                let resp = ACP.Agent.InitializeResponse(protocol_version: "0.7.0", agent_capabilities: .init(), auth_methods: [], agent_info: ACP.Agent.Implementation(name: "openagents-mac", title: "OpenAgents macOS", version: "0.1.0"), _meta: nil)
                if let out = try? JSONEncoder().encode(JSONRPC.Response(id: idVal, result: resp)), let jtext = String(data: out, encoding: .utf8) {
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
            guard let env = try? WebSocketMessage.Envelope.from(jsonString: text) else {
                print("[Bridge][server] unparseable message: \(text)")
                return
            }
            print("[Bridge][server] envelope type=\(env.type)")
            let etype = env.type.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)
            switch etype {
            case "threads.list.request":
                // Decode request (cap at 10 for lightweight response)
                let req = (try? env.decodedMessage(as: WebSocketMessage.ThreadsListRequest.self)) ?? .init(topK: 10)
                let limit = min(10, max(1, req.topK ?? 10))
                // Single-base, fast top-K scan (lightweight)
                let base = CodexScanner.defaultBaseDir()
                let urls = CodexScanner.listRecentTopN(at: base, topK: limit)
                var items = urls.map { CodexScanner.makeSummary(for: $0, base: base) }
                items.sort { ($0.last_message_ts ?? $0.updated_at) > ($1.last_message_ts ?? $1.updated_at) }
                if items.count > limit { items = Array(items.prefix(limit)) }
                print("[Bridge][server] threads.list.request topK=\(limit) -> count=\(items.count)")
                let resp = WebSocketMessage.ThreadsListResponse(items: items)
                if let out = try? WebSocketMessage.Envelope.envelope(for: resp, type: "threads.list.response").jsonString(prettyPrinted: false) {
                    print("[Bridge][server] sending threads.list.response bytes=\(out.count)")
                    client.send(text: out)
                } else {
                    print("[Bridge][server] failed to encode threads.list.response")
                }
            default:
                print("[Bridge][server] unknown envelope type=\(env.type)")
            }
        }
    }
}
#endif
