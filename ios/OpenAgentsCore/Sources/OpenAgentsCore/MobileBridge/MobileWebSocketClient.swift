#if canImport(Foundation)
import Foundation

public protocol MobileWebSocketClientDelegate: AnyObject {
    /// Called when the client successfully connects and authenticates
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient)

    /// Called when the client disconnects or fails to connect
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?)

    /// Called when the client receives a message
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage)
}

public final class MobileWebSocketClient {
    public weak var delegate: MobileWebSocketClientDelegate?

    private var webSocketTask: URLSessionWebSocketTask?
    private let session: URLSession

    private var isConnected: Bool = false

    public init(session: URLSession = .shared) {
        self.session = session
    }

    /// Connect to the WebSocket URL and authenticate with token
    /// - Parameters:
    ///   - url: The WebSocket URL to connect to
    ///   - token: The authentication token to send in Hello message
    public func connect(url: URL, token: String) {
        disconnect()

        let request = URLRequest(url: url)
        let webSocketTask = session.webSocketTask(with: request)
        self.webSocketTask = webSocketTask

        webSocketTask.resume()

        // Send Hello message as text JSON for compatibility with server
        let hello = BridgeMessages.Hello(token: token)
        do {
            let data = try JSONEncoder().encode(hello)
            let text = String(data: data, encoding: .utf8) ?? "{\"token\":\"\(token)\"}"
            let message = URLSessionWebSocketTask.Message.string(text)
            webSocketTask.send(message) { [weak self] error in
                if let error = error {
                    self?.disconnect(error: error)
                    return
                }
                // Wait for HelloAck
                self?.waitForHelloAck(expectedToken: token)
            }
        } catch {
            disconnect(error: error)
        }
    }

    /// Send ping to keep connection alive
    public func sendPing() {
        webSocketTask?.sendPing { [weak self] error in
            if let error = error {
                self?.disconnect(error: error)
            }
        }
    }

    /// Send an envelope with a typed message
    public func send<T: Codable>(type: String, message: T) {
        do {
            let env = try BridgeMessage.envelope(for: message, type: type)
            if let json = try? env.jsonString(prettyPrinted: false) {
                webSocketTask?.send(.string(json)) { _ in }
            } else {
                let data = try JSONEncoder().encode(env)
                webSocketTask?.send(.data(data)) { _ in }
            }
        } catch {
            // ignore
        }
    }

    /// Disconnect the WebSocket connection
    public func disconnect() {
        disconnect(error: nil)
    }

    private func disconnect(error: Error?) {
        isConnected = false
        webSocketTask?.cancel(with: .goingAway, reason: nil)
        webSocketTask = nil
        delegate?.mobileWebSocketClient(self, didDisconnect: error)
    }

    private func waitForHelloAck(expectedToken: String) {
        webSocketTask?.receive { [weak self] result in
            guard let self = self else { return }

            switch result {
            case .failure(let error):
                self.disconnect(error: error)
            case .success(let message):
                switch message {
                case .data(let data):
                    do {
                        let helloAck = try JSONDecoder().decode(BridgeMessages.HelloAck.self, from: data)
                        if helloAck.token == expectedToken || helloAck.token.isEmpty {
                            self.isConnected = true
                            self.delegate?.mobileWebSocketClientDidConnect(self)
                            // Start general receive loop after handshake
                            self.receive()
                        } else {
                            self.disconnect(error: NSError(domain: "MobileWebSocketClient", code: 1, userInfo: [NSLocalizedDescriptionKey: "HelloAck token mismatch"]))
                        }
                    } catch {
                        self.disconnect(error: error)
                    }
                case .string(let text):
                    // Accept text HelloAck as well
                    if let data = text.data(using: .utf8), let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                        if (obj["type"] as? String) == "HelloAck" {
                            self.isConnected = true
                            self.delegate?.mobileWebSocketClientDidConnect(self)
                            self.receive()
                            return
                        }
                        if let token = obj["token"] as? String, token == expectedToken {
                            self.isConnected = true
                            self.delegate?.mobileWebSocketClientDidConnect(self)
                            return
                        }
                    }
                    self.disconnect(error: NSError(domain: "MobileWebSocketClient", code: 2, userInfo: [NSLocalizedDescriptionKey: "Invalid HelloAck message"]))
                @unknown default:
                    self.disconnect(error: NSError(domain: "MobileWebSocketClient", code: 3, userInfo: [NSLocalizedDescriptionKey: "Unknown message type"]))
                }
            }
        }
    }

    private func receive() {
        webSocketTask?.receive { [weak self] result in
            guard let self = self else { return }
            switch result {
            case .failure(let error):
                self.disconnect(error: error)
            case .success(let message):
                switch message {
                case .data(let data):
                    if let env = try? JSONDecoder().decode(BridgeMessage.self, from: data) {
                        self.delegate?.mobileWebSocketClient(self, didReceiveMessage: env)
                    }
                case .string(let text):
                    if let data = text.data(using: .utf8), let env = try? JSONDecoder().decode(BridgeMessage.self, from: data) {
                        self.delegate?.mobileWebSocketClient(self, didReceiveMessage: env)
                    }
                @unknown default:
                    break
                }

                self.receive()
            }
        }
    }
}

#endif
