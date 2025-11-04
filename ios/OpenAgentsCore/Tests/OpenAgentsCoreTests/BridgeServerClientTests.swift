import XCTest
@testable import OpenAgentsCore

#if os(macOS)
final class BridgeServerClientTests: XCTestCase {
    private var server: DesktopWebSocketServer?
    private var client: MobileWebSocketClient?
    private var responseExpectation: XCTestExpectation?

    override func tearDown() {
        super.tearDown()
        client?.disconnect()
        client = nil
        server?.stop()
        server = nil
    }

    func testThreadsListRoundTrip() throws {
        let port: UInt16 = 9911
        let token = "unit-test-token"
        let srv = DesktopWebSocketServer(token: token)
        server = srv
        try srv.start(port: port, advertiseService: false)

        let exp = expectation(description: "threads.list.response received")
        responseExpectation = exp

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = self

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url, token: token)

        wait(for: [exp], timeout: 5.0)
    }
}

extension BridgeServerClientTests: MobileWebSocketClientDelegate {
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        let request = BridgeMessages.ThreadsListRequest(topK: 5)
        client.send(type: "threads.list.request", message: request)
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let error = error {
            XCTFail("Unexpected disconnect: \(error)")
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveMessage message: BridgeMessage) {
        if message.type == "threads.list.response" {
            responseExpectation?.fulfill()
        }
    }
}
#endif
