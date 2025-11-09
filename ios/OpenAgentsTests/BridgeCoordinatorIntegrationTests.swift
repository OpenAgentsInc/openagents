import XCTest
import Combine
@testable import OpenAgents
@testable import OpenAgentsCore

#if os(iOS)
@MainActor
final class BridgeCoordinatorIntegrationTests: XCTestCase {
    final class FakeConnectionManager: ConnectionManaging {
        let statusSubject = PassthroughSubject<BridgeManager.Status, Never>()
        let logSubject = PassthroughSubject<String, Never>()
        let wdSubject = CurrentValueSubject<String?, Never>(nil)
        let notifySubject = PassthroughSubject<(method: String, payload: Data), Never>()

        var statusPublisher: AnyPublisher<BridgeManager.Status, Never> { statusSubject.eraseToAnyPublisher() }
        var logPublisher: AnyPublisher<String, Never> { logSubject.eraseToAnyPublisher() }
        var workingDirectoryPublisher: AnyPublisher<String?, Never> { wdSubject.eraseToAnyPublisher() }
        var notificationPublisher: AnyPublisher<(method: String, payload: Data), Never> { notifySubject.eraseToAnyPublisher() }

        var rpcClient: JSONRPCSending? { nil }

        func start() {}
        func stop() {}
        func performManualConnect(host: String, port: Int) {}
    }

    func testSessionUpdateFlowsIntoBridgeManagerUpdates() throws {
        let bridge = BridgeManager()
        let fake = FakeConnectionManager()
        bridge.wireConnection(fake)

        // Construct a simple agent message chunk update
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "hello")))
        let note = ACP.Client.SessionNotificationWire(session_id: ACPSessionId("s1"), update: .agentMessageChunk(chunk))
        let payload = try JSONEncoder().encode(note)

        // Emit the notification
        fake.notifySubject.send((ACPRPC.sessionUpdate, payload))

        // Assert BridgeManager mirrored state updated
        XCTAssertEqual(bridge.updates.count, 1)
        if case .agentMessageChunk(let mu) = bridge.updates[0].update,
           case .text(let t) = mu.content {
            XCTAssertEqual(t.text, "hello")
        } else { XCTFail("Expected agentMessageChunk") }
    }
}
#endif
