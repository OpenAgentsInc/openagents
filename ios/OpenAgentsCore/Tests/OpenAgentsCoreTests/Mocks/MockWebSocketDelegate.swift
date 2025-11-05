import Foundation
@testable import OpenAgentsCore

/// Mock delegate for MobileWebSocketClient that records all callbacks
final class MockMobileWebSocketClientDelegate: MobileWebSocketClientDelegate {
    var didConnect = false
    var didDisconnect = false
    var disconnectError: Error?
    var receivedMessages: [BridgeMessage] = []
    var receivedNotifications: [(method: String, payload: Data)] = []
    var receivedRequests: [(method: String, id: String, params: Data)] = []

    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        didConnect = true
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        didDisconnect = true
        disconnectError = error
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        receivedMessages.append(message)
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data) {
        receivedNotifications.append((method, payload))
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCRequest method: String, id: String, params: Data) -> Data? {
        receivedRequests.append((method, id, params))
        return nil
    }

    func reset() {
        didConnect = false
        didDisconnect = false
        disconnectError = nil
        receivedMessages = []
        receivedNotifications = []
        receivedRequests = []
    }
}

#if os(macOS)
/// Mock delegate for DesktopWebSocketServer that records all callbacks
final class MockDesktopWebSocketServerDelegate: DesktopWebSocketServerDelegate {
    var acceptedClients: [DesktopWebSocketServer.Client] = []
    var handshakeResults: [(client: DesktopWebSocketServer.Client, success: Bool)] = []
    var disconnectedClients: [(client: DesktopWebSocketServer.Client, reason: NWError?)] = []

    func webSocketServer(_ server: DesktopWebSocketServer, didAccept client: DesktopWebSocketServer.Client) {
        acceptedClients.append(client)
    }

    func webSocketServer(_ server: DesktopWebSocketServer, didCompleteHandshakeFor client: DesktopWebSocketServer.Client, success: Bool) {
        handshakeResults.append((client, success))
    }

    func webSocketServer(_ server: DesktopWebSocketServer, didDisconnect client: DesktopWebSocketServer.Client, reason: NWError?) {
        disconnectedClients.append((client, reason))
    }

    func reset() {
        acceptedClients = []
        handshakeResults = []
        disconnectedClients = []
    }
}
#endif
