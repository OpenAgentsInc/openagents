import XCTest
@testable import OpenAgents
@testable import OpenAgentsCore

#if os(iOS)

final class ACPTimelineViewModelTests: XCTestCase {
    @MainActor
    func testMessageTransform_UserAndAgent() async throws {
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        let sid = ACPSessionId("test-session")
        bridge.currentSessionId = sid

        let userChunk = ACP.Client.ContentChunk(content: .text(.init(text: "Hello")))
        let agentChunk = ACP.Client.ContentChunk(content: .text(.init(text: "Hi there!")))

        let uWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .userMessageChunk(userChunk))
        let aWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .agentMessageChunk(agentChunk))

        bridge.updates = [uWire, aWire]

        // Allow Combine delivery on main loop
        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        XCTAssertEqual(vm.items.count, 2)
        guard vm.items.count == 2 else { return }

        if case let .message(role1, text1, _) = vm.items[0] {
            switch role1 { case .user: break; default: XCTFail("First should be user") }
            XCTAssertEqual(text1, "Hello")
        } else { XCTFail("First should be message") }

        if case let .message(role2, text2, _) = vm.items[1] {
            switch role2 { case .assistant: break; default: XCTFail("Second should be assistant") }
            XCTAssertEqual(text2, "Hi there!")
        } else { XCTFail("Second should be message") }
    }
}

#endif

