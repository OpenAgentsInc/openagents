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

    @MainActor
    func testFiltersWarmupAsFirstMessage() async throws {
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        let sid = ACPSessionId("test-session")
        bridge.currentSessionId = sid

        // First update is a provider warmup artifact
        let warm = ACP.Client.ContentChunk(content: .text(.init(text: "Warmup")))
        let wWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .userMessageChunk(warm))

        // Then a real user message
        let user = ACP.Client.ContentChunk(content: .text(.init(text: "Real question")))
        let uWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .userMessageChunk(user))

        bridge.updates = [wWire, uWire]

        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        // Warmup should be filtered; only the real question remains
        XCTAssertEqual(vm.items.count, 1)
        if case let .message(_, text, _) = vm.items.first {
            XCTAssertEqual(text, "Real question")
        } else {
            XCTFail("Expected one message item")
        }
    }

    @MainActor
    func testNoLeakFromOtherSessionsWhenNoCurrentSession() async throws {
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        // No current session yet (nil). Only 'pending' items should render.
        let otherSid = ACPSessionId("other")
        let pendingSid = ACPSessionId("pending")

        let other = ACP.Client.SessionNotificationWire(
            session_id: otherSid,
            update: .agentMessageChunk(.init(content: .text(.init(text: "From other session"))))
        )
        let pending = ACP.Client.SessionNotificationWire(
            session_id: pendingSid,
            update: .userMessageChunk(.init(content: .text(.init(text: "Local echo"))))
        )

        bridge.updates = [other, pending]

        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        // Only the 'pending' echo should be visible
        XCTAssertEqual(vm.items.count, 1)
        if case let .message(role, text, _) = vm.items.first {
            switch role { case .user: break; default: XCTFail("Expected user echo") }
            XCTAssertEqual(text, "Local echo")
        } else { XCTFail("Expected one message item") }
    }

    @MainActor
    func testToolCallRendering() async throws {
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        let sid = ACPSessionId("test-session")
        bridge.currentSessionId = sid

        // Create a tool call
        let toolCall = ACP.Client.ACPToolCallWire(
            call_id: "call_123",
            name: "Read",
            arguments: ["file_path": AnyEncodable("/test/file.txt")],
            _meta: nil
        )
        let callWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCall(toolCall))

        // Create a tool call update (completed)
        let toolUpdate = ACP.Client.ACPToolCallUpdateWire(
            call_id: "call_123",
            status: .completed,
            output: AnyEncodable("File contents here"),
            error: nil
        )
        let updateWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCallUpdate(toolUpdate))

        bridge.updates = [callWire, updateWire]

        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        // Should have tool call + tool result
        XCTAssertEqual(vm.items.count, 2)
        guard vm.items.count == 2 else { return }

        // First item should be tool call
        if case let .toolCall(call) = vm.items[0] {
            XCTAssertEqual(call.id, "call_123")
            XCTAssertEqual(call.tool_name, "Read")
        } else {
            XCTFail("First item should be tool call")
        }

        // Second item should be tool result
        if case let .toolResult(result) = vm.items[1] {
            XCTAssertEqual(result.call_id, "call_123")
            XCTAssertTrue(result.ok)
        } else {
            XCTFail("Second item should be tool result")
        }
    }

    @MainActor
    func testToolCallFiltersOutTodoWrite() async throws {
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        let sid = ACPSessionId("test-session")
        bridge.currentSessionId = sid

        // Create a TodoWrite tool call (should be filtered)
        let todoCall = ACP.Client.ACPToolCallWire(
            call_id: "call_todo",
            name: "TodoWrite",
            arguments: ["todos": AnyEncodable([["content": "test", "status": "pending"]])],
            _meta: nil
        )
        let todoWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCall(todoCall))

        // Create a regular tool call (should appear)
        let readCall = ACP.Client.ACPToolCallWire(
            call_id: "call_read",
            name: "Read",
            arguments: ["file_path": AnyEncodable("/test.txt")],
            _meta: nil
        )
        let readWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCall(readCall))

        bridge.updates = [todoWire, readWire]

        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        // Only the Read call should appear (TodoWrite filtered)
        XCTAssertEqual(vm.items.count, 1)
        if case let .toolCall(call) = vm.items.first {
            XCTAssertEqual(call.tool_name, "Read")
        } else {
            XCTFail("Expected one Read tool call")
        }
    }

    @MainActor
    func testOrphanedToolUpdateDoesNotCrash() async throws {
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        let sid = ACPSessionId("test-session")
        bridge.currentSessionId = sid

        // Create only a tool update without the initial call
        let toolUpdate = ACP.Client.ACPToolCallUpdateWire(
            call_id: "orphan_123",
            status: .completed,
            output: AnyEncodable("Some output"),
            error: nil
        )
        let updateWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCallUpdate(toolUpdate))

        bridge.updates = [updateWire]

        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        // Should have the orphaned result (won't crash, but won't find matching call)
        XCTAssertEqual(vm.items.count, 1)
        if case let .toolResult(result) = vm.items.first {
            XCTAssertEqual(result.call_id, "orphan_123")
        } else {
            XCTFail("Expected orphaned tool result")
        }
    }

    @MainActor
    func testRegressionIssue1432_ToolCallPatternMatches() async throws {
        // Regression test for GitHub issue #1432
        // Verifies that .toolCall events from bridge are properly matched in switch statement
        let bridge = BridgeManager()
        let vm = ACPTimelineViewModel()
        vm.attach(bridge: bridge)

        let sid = ACPSessionId("test-session-1432")
        bridge.currentSessionId = sid

        // Create tool call exactly as bridge receives it
        let toolCall = ACP.Client.ACPToolCallWire(
            call_id: "item_1",
            name: "Bash",
            arguments: ["command": AnyEncodable("ls -la")],
            _meta: nil
        )
        let callWire = ACP.Client.SessionNotificationWire(session_id: sid, update: .toolCall(toolCall))

        bridge.updates = [callWire]

        let exp = expectation(description: "items updated")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { exp.fulfill() }
        await fulfillment(of: [exp], timeout: 1.0)

        // CRITICAL: The .toolCall case MUST match
        XCTAssertEqual(vm.items.count, 1, "Tool call should be processed and added to items")
        if case let .toolCall(call) = vm.items.first {
            XCTAssertEqual(call.id, "item_1")
            XCTAssertEqual(call.tool_name, "Bash")
        } else {
            XCTFail("Expected tool call item (regression: .toolCall case not matching)")
        }
    }
}

#endif
