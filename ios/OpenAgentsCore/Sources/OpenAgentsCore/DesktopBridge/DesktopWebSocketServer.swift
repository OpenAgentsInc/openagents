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
            connection.send(content: data, contentContext: context, isComplete: true, completion: .contentProcessed { _ in })
        }

        /// Send a Ping frame
        public func sendPing() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .ping)
            let context = NWConnection.ContentContext(identifier: "pingContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { _ in })
        }

        /// Send a Pong frame
        public func sendPong() {
            let metadata = NWProtocolWebSocket.Metadata(opcode: .pong)
            let context = NWConnection.ContentContext(identifier: "pongContext", metadata: [metadata])
            connection.send(content: nil, contentContext: context, isComplete: true, completion: .contentProcessed { _ in })
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
                listener?.service = NWListener.Service(name: serviceName ?? Host.current().localizedName ?? "OpenAgents", type: serviceType)
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
        delegate?.webSocketServer(self, didAccept: client)

        connection.stateUpdateHandler = { [weak self, weak client] state in
            guard let self = self, let client = client else { return }
            switch state {
            case .ready:
                self.receiveNextMessage(client)
            case .failed(_), .cancelled:
                self.clients.remove(client)
                self.delegate?.webSocketServer(self, didDisconnect: client, reason: nil)
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
                self.delegate?.webSocketServer(self, didDisconnect: client, reason: error)
                client.connection.cancel()
                return
            }

            guard let ctx = context else {
                // No context means connection closed
                self.clients.remove(client)
                self.delegate?.webSocketServer(self, didDisconnect: client, reason: nil)
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
            // Accept either {"type":"Hello","token":"..."} or {"token":"..."}
            guard let data = text.data(using: .utf8),
                  let json = try? JSONSerialization.jsonObject(with: data, options: []) as? [String: Any] else {
                delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: false)
                client.close(); clients.remove(client); return
            }
            let type = (json["type"] as? String) ?? ""
            let receivedToken = (json["token"] as? String) ?? ""
            guard type == "Hello" || (!receivedToken.isEmpty) else {
                delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: false)
                client.close(); clients.remove(client); return
            }
            if receivedToken == token {
                print("[Bridge][server] Hello received; token ok")
                client.isHandshakeComplete = true
                // Reply with HelloAck; include token for clients expecting it
                let ackObj: [String: Any] = ["type": "HelloAck", "token": token]
                if let ackData = try? JSONSerialization.data(withJSONObject: ackObj, options: []),
                   let ackText = String(data: ackData, encoding: .utf8) {
                    client.send(text: ackText)
                }
                delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: true)
            } else {
                print("[Bridge][server] Hello token mismatch")
                delegate?.webSocketServer(self, didCompleteHandshakeFor: client, success: false)
                client.close(); clients.remove(client)
            }
        } else {
            // Handle envelopes after handshake
            if let env = try? WebSocketMessage.Envelope.from(jsonString: text) {
                switch env.type {
                case "threads.list.request":
                    // Decode request
                    let req = (try? env.decodedMessage(as: WebSocketMessage.ThreadsListRequest.self)) ?? .init(topK: 20)
                    let items = CodexDiscovery.loadAllSummaries(maxFilesPerBase: 1000, maxResults: req.topK ?? 20)
                    print("[Bridge][server] threads.list.request topK=\(req.topK ?? 20) -> count=\(items.count)")
                    let resp = WebSocketMessage.ThreadsListResponse(items: items)
                    if let out = try? WebSocketMessage.Envelope.envelope(for: resp, type: "threads.list.response").jsonString(prettyPrinted: false) {
                        client.send(text: out)
                    }
                default:
                    print("[Bridge][server] unknown envelope type=\(env.type)")
                    break
                }
            }
        }
    }
}
#endif
