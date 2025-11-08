import XCTest
@testable import OpenAgentsCore

#if os(macOS)
final class BridgeServerAdditionalTests: XCTestCase {
    var server: DesktopWebSocketServer!

    override func tearDown() {
        server?.stop()
        server = nil
        super.tearDown()
    }

    func testSessionSetModeEmitsCurrentModeUpdate() throws {
        server = DesktopWebSocketServer()
        let port: UInt16 = 9977
        try server.start(port: port, advertiseService: false)

        let client = MobileWebSocketClient()
        let delegate = MockMobileWebSocketClientDelegate()
        client.delegate = delegate
        client.connect(url: URL(string: "ws://127.0.0.1:\(port)")!)

        // Wait for connect
        let connectExpectation = expectation(description: "connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if delegate.didConnect { connectExpectation.fulfill() }
        }
        wait(for: [connectExpectation], timeout: 3.0)

        // Use explicit session id for assertion
        let sid = ACPSessionId("session-for-mode-test")
        struct Params: Codable { let session_id: ACPSessionId; let mode_id: String }

        let responseExpectation = expectation(description: "set_mode response")
        client.sendJSONRPC(method: ACPRPC.sessionSetMode, params: Params(session_id: sid, mode_id: ACPSessionModeId.claude_code.rawValue), id: "set-mode-1") { (resp: ACP.Agent.SetSessionModeResponse?) in
            XCTAssertNotNil(resp)
            responseExpectation.fulfill()
        }
        wait(for: [responseExpectation], timeout: 5.0)

        // Wait briefly for update notification
        let updateExpectation = expectation(description: "current_mode_update notification")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            let updates = delegate.receivedNotifications.filter { $0.method == ACPRPC.sessionUpdate }
            if !updates.isEmpty { updateExpectation.fulfill() }
        }
        wait(for: [updateExpectation], timeout: 3.0)

        // At least one session/update should have been broadcast
        let updates = delegate.receivedNotifications.filter { $0.method == ACPRPC.sessionUpdate }
        XCTAssertFalse(updates.isEmpty)
        client.disconnect()
    }

    func testSessionPromptEmptyPromptReturnsError() throws {
        server = DesktopWebSocketServer()
        let port: UInt16 = 9976
        try server.start(port: port, advertiseService: false)

        let client = MobileWebSocketClient()
        let delegate = MockMobileWebSocketClientDelegate()
        client.delegate = delegate
        client.connect(url: URL(string: "ws://127.0.0.1:\(port)")!)

        // Wait for connect
        let connectExpectation = expectation(description: "connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if delegate.didConnect { connectExpectation.fulfill() }
        }
        wait(for: [connectExpectation], timeout: 3.0)

        // Build request with empty content -> should error
        let req = ACP.Agent.SessionPromptRequest(session_id: ACPSessionId("s-empty"), content: [])
        let responseExpectation = expectation(description: "prompt error")
        client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "prompt-empty-1") { (resp: [String:String]?) in
            XCTAssertNil(resp, "Expected error (nil result) for empty prompt")
            responseExpectation.fulfill()
        }
        wait(for: [responseExpectation], timeout: 5.0)
        client.disconnect()
    }

    func testSessionPromptInvalidParamsReturnsError() throws {
        server = DesktopWebSocketServer()
        let port: UInt16 = 9975
        try server.start(port: port, advertiseService: false)

        let client = MobileWebSocketClient()
        let delegate = MockMobileWebSocketClientDelegate()
        client.delegate = delegate
        client.connect(url: URL(string: "ws://127.0.0.1:\(port)")!)

        // Wait for connect
        let connectExpectation = expectation(description: "connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if delegate.didConnect { connectExpectation.fulfill() }
        }
        wait(for: [connectExpectation], timeout: 3.0)

        struct BadParams: Codable { let foo: String }
        let responseExpectation = expectation(description: "invalid params error")
        client.sendJSONRPC(method: ACPRPC.sessionPrompt, params: BadParams(foo: "bar"), id: "prompt-bad-1") { (resp: [String:String]?) in
            XCTAssertNil(resp, "Expected error (nil result) for invalid params")
            responseExpectation.fulfill()
        }
        wait(for: [responseExpectation], timeout: 5.0)
        client.disconnect()
    }
}
// Mock extension for delegate signature bridging
extension MockMobileWebSocketClientDelegate {
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient, workingDirectory: String?) {
        self.didConnect = true
    }
}

#endif
