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

    func testWorkingDirectoryInHandshake() throws {
        let port: UInt16 = 9912
        let srv = DesktopWebSocketServer()
        server = srv

        // Set a test working directory on the server
        let testWorkingDir = URL(fileURLWithPath: "/Users/test/code/myproject")
        srv.workingDirectory = testWorkingDir

        try srv.start(port: port, advertiseService: false)

        let exp = expectation(description: "working directory received")
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
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient, workingDirectory: String?) {
        // If we're testing working directory, verify it was received
        if responseExpectation?.expectationDescription.contains("working directory") == true {
            XCTAssertNotNil(workingDirectory, "Working directory should be present")
            XCTAssertEqual(workingDirectory, "/Users/test/code/myproject", "Working directory should match server value")
            responseExpectation?.fulfill()
        } else {
            // Otherwise run the threads list test
            client.sendJSONRPC(method: "threads/list", params: ThreadsListParams(topK: 5), id: "test-threads-list") { (resp: ThreadsListResult?) in
                if let _ = resp { self.responseExpectation?.fulfill() }
            }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let error = error {
            XCTFail("Unexpected disconnect: \(error)")
        }
    }

    // JSON-RPC response is handled via completion in sendJSONRPC; no-op here.
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCNotification method: String, payload: Data) {}
    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceiveJSONRPCRequest method: String, id: String, params: Data) -> Data? { nil }
}
#endif
