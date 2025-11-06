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
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let exp = expectation(description: "threads.list.response received")
        responseExpectation = exp

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = self

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [exp], timeout: 5.0)
    }
}

extension BridgeServerClientTests: MobileWebSocketClientDelegate {
    struct ThreadsListParams: Codable { let topK: Int? }
    struct ThreadsListResult: Codable { let items: [ThreadSummary] }
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        client.sendJSONRPC(method: "threads/list", params: ThreadsListParams(topK: 5), id: "test-threads-list") { (resp: ThreadsListResult?) in
            if let _ = resp { self.responseExpectation?.fulfill() }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let error = error {
            XCTFail("Unexpected disconnect: \(error)")
        }
    }

    // JSON-RPC response is handled via completion in sendJSONRPC; no-op here.
}
#endif
