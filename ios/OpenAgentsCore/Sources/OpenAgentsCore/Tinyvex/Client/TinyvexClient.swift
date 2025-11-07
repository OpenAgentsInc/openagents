import Foundation

public final class TinyvexClient: NSObject {
    public enum ConnectionState { case disconnected, connecting, connected }

    private var url: URL
    private var session: URLSession!
    private var ws: URLSessionWebSocketTask?
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    public init(endpoint: URL) {
        self.url = endpoint
        super.init()
        let cfg = URLSessionConfiguration.default
        self.session = URLSession(configuration: cfg, delegate: nil, delegateQueue: nil)
    }

    public func connect() {
        let task = session.webSocketTask(with: url)
        self.ws = task
        task.resume()
        receive()
    }

    public func disconnect() { ws?.cancel() }

    // MARK: - ACP lifecycle
    public func initialize(_ req: ACP.Agent.InitializeRequest, id: JSONRPCId = .int(1)) async throws -> ACP.Agent.InitializeResponse {
        try await rpc(method: TinyvexMethods.initialize, params: req, id: id)
    }
    public func sessionNew(_ req: ACP.Agent.SessionNewRequest, id: JSONRPCId = .int(2)) async throws -> ACP.Agent.SessionNewResponse {
        try await rpc(method: TinyvexMethods.sessionNew, params: req, id: id)
    }
    public func sessionPrompt(_ req: ACP.Agent.SessionPromptRequest, id: JSONRPCId = .int(3)) async throws { let _: Empty = try await rpc(method: TinyvexMethods.sessionPrompt, params: req, id: id) }
    public func sessionCancel(sessionId: ACPSessionId) async { _ = try? await rpc(method: TinyvexMethods.sessionCancel, params: ["session_id": sessionId.value], id: .int(4)) as Empty }

    struct Empty: Codable {}

    // MARK: - Session updates stream (simple closure callback)
    public func onSessionUpdate(_ handler: @escaping (ACP.Client.SessionNotificationWire) -> Void) {
        // naive listener: rely on shared receive loop and parse notifications
        self._updateHandler = handler
    }

    private var _updateHandler: ((ACP.Client.SessionNotificationWire) -> Void)?

    private func receive() {
        ws?.receive { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure:
                return
            case .success(let msg):
                switch msg {
                case .string(let s): self.handleMessage(Data(s.utf8))
                case .data(let d): self.handleMessage(d)
                @unknown default: break
                }
                self.receive()
            }
        }
    }

    private func handleMessage(_ data: Data) {
        if let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let method = obj["method"] as? String, method == TinyvexMethods.sessionUpdate {
            if let params = try? decoder.decode(ACP.Client.SessionNotificationWire.self, from: (try? JSONSerialization.data(withJSONObject: obj["params"] ?? [:])) ?? Data("{}".utf8)) {
                _updateHandler?(params)
            }
        }
    }

    // MARK: - RPC helper
    private func rpc<Req: Encodable, Res: Decodable>(method: String, params: Req, id: JSONRPCId) async throws -> Res {
        struct ReqWrap<P: Encodable>: Encodable { let jsonrpc = "2.0"; let id: JSONRPCId; let method: String; let params: P }
        let payload = try encoder.encode(ReqWrap(id: id, method: method, params: params))
        try await wsSend(payload)
        // naive: wait for first response with same id
        let (data, _) = try await wsRecv()
        // Attempt to parse as result
        let root = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        if let err = root?["error"] as? [String: Any] { throw TinyvexError.server(code: String(describing: err["code"] ?? -1), message: String(describing: err["message"] ?? "error")) }
        if let resData = try? JSONSerialization.data(withJSONObject: root?["result"] ?? [:]) {
            return try decoder.decode(Res.self, from: resData)
        }
        throw TinyvexError.decoding("Missing result")
    }

    private func wsSend(_ data: Data) async throws {
        try await ws?.send(.data(data))
    }

    private func wsRecv() async throws -> (Data, String?) {
        guard let ws else { throw TinyvexError.transport("no socket") }
        let msg = try await ws.receive()
        switch msg {
        case .data(let d): return (d, nil)
        case .string(let s): return (Data(s.utf8), nil)
        @unknown default: return (Data(), nil)
        }
    }
}

