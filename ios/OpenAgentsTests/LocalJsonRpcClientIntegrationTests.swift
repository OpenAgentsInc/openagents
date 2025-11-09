import XCTest
import Combine
@testable import OpenAgents
import OpenAgentsCore

final class LocalJsonRpcClientIntegrationTests: XCTestCase {
    var cancellables: Set<AnyCancellable> = []

    func testSessionNewSetModePublishesUpdate() throws {
        #if os(macOS)
        let server = DesktopWebSocketServer()
        // Use a temporary Tinyvex DB path
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("LocalJsonRpcClientTests-")
            .appendingPathComponent(UUID().uuidString).appendingPathExtension("sqlite")
        server.setTinyvexDb(path: tmp.path)

        let rpc = LocalJsonRpcClient(server: server)

        let expectUpdate = expectation(description: "Expect currentModeUpdate published")
        var receivedMode: ACPSessionModeId?

        server.notificationPublisher
            .sink { evt in
                guard evt.method == ACPRPC.sessionUpdate else { return }
                if let note = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: evt.payload) {
                    if case .currentModeUpdate(let cm) = note.update {
                        receivedMode = cm.current_mode_id
                        expectUpdate.fulfill()
                    }
                }
            }
            .store(in: &cancellables)

        var newSessionId: ACPSessionId?
        let newExpectation = expectation(description: "session/new response")
        rpc.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: "test-new") { (resp: ACP.Agent.SessionNewResponse?) in
            newSessionId = resp?.session_id
            newExpectation.fulfill()
        }
        wait(for: [newExpectation], timeout: 2.0)
        XCTAssertNotNil(newSessionId)

        struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
        let setModeExpectation = expectation(description: "session/set_mode response")
        rpc.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: newSessionId!, mode_id: .codex), id: "test-set-mode") { (_: ACP.Agent.SetSessionModeResponse?) in
            setModeExpectation.fulfill()
        }

        wait(for: [setModeExpectation, expectUpdate], timeout: 3.0)
        XCTAssertEqual(receivedMode, .codex)

        // History endpoints should be callable (may return empty if no persisted timeline yet)
        let recentExpectation = expectation(description: "recent sessions fetched")
        struct EmptyParams: Codable {}
        rpc.sendJSONRPC(method: "tinyvex/history.recentSessions", params: EmptyParams(), id: "test-recent") { (items: [RecentSession]?) in
            // No assert on contents; success path should decode (even if empty)
            XCTAssertNotNil(items)
            recentExpectation.fulfill()
        }
        wait(for: [recentExpectation], timeout: 2.0)
        #else
        throw XCTSkip("LocalJsonRpcClient integration is macOS-only")
        #endif
    }
}

