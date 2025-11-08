import XCTest
@testable import OpenAgentsCore

#if os(macOS)
final class ExtCapabilitiesGatingTests: XCTestCase {
    var server: DesktopWebSocketServer!

    override func tearDown() {
        server?.stop()
        server = nil
        super.tearDown()
    }

    func testOrchestrateExploreStartRejectedWhenExtDisabled() throws {
        server = DesktopWebSocketServer()
        // Explicitly disable ext support
        server.overrideExtOrchestrateExplore = false
        try server.start(port: 9979, advertiseService: false)

        let client = MobileWebSocketClient()
        let delegate = MockMobileWebSocketClientDelegate()
        client.delegate = delegate
        client.connect(url: URL(string: "ws://127.0.0.1:9979")!)

        // Wait for connect
        let connectExpectation = expectation(description: "connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if delegate.didConnect { connectExpectation.fulfill() }
        }
        wait(for: [connectExpectation], timeout: 3.0)

        let params = OrchestrateExploreStartRequest(root: FileManager.default.currentDirectoryPath)
        let responseExpectation = expectation(description: "orchestrate rejected")
        var response: OrchestrateExploreStartResponse?
        client.sendJSONRPC(method: ACPRPC.orchestrateExploreStart, params: params, id: "orch-1") { (result: OrchestrateExploreStartResponse?) in
            response = result
            responseExpectation.fulfill()
        }
        wait(for: [responseExpectation], timeout: 5.0)
        // When gated off, server returns JSON-RPC error; client completion receives nil
        XCTAssertNil(response)

        client.disconnect()
    }

    func testOrchestrateExploreStartAllowedWhenExtEnabled() throws {
        server = DesktopWebSocketServer()
        // Explicitly enable ext support regardless of platform availability
        server.overrideExtOrchestrateExplore = true
        try server.start(port: 9978, advertiseService: false)

        let client = MobileWebSocketClient()
        let delegate = MockMobileWebSocketClientDelegate()
        client.delegate = delegate
        client.connect(url: URL(string: "ws://127.0.0.1:9978")!)

        // Wait for connect
        let connectExpectation = expectation(description: "connected")
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
            if delegate.didConnect { connectExpectation.fulfill() }
        }
        wait(for: [connectExpectation], timeout: 3.0)

        let params = OrchestrateExploreStartRequest(root: FileManager.default.currentDirectoryPath)
        let responseExpectation = expectation(description: "orchestrate response")
        var response: OrchestrateExploreStartResponse?
        client.sendJSONRPC(method: ACPRPC.orchestrateExploreStart, params: params, id: "orch-2") { (result: OrchestrateExploreStartResponse?) in
            response = result
            responseExpectation.fulfill()
        }
        wait(for: [responseExpectation], timeout: 5.0)
        XCTAssertNotNil(response)
        XCTAssertFalse(response?.session_id.isEmpty ?? true)

        client.disconnect()
    }
}
#endif

