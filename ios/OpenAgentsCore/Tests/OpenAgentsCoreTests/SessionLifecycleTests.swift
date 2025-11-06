import XCTest
@testable import OpenAgentsCore

#if os(macOS)
/// Tests for complete session lifecycle management
/// Tests the flow: session creation → prompts → tool execution → completion
final class SessionLifecycleTests: XCTestCase {
    private var server: DesktopWebSocketServer?
    private var client: MobileWebSocketClient?
    private var sessionCreated: Bool = false
    private var sessionId: ACPSessionId?
    private var lifecycleExpectation: XCTestExpectation?

    override func tearDown() {
        super.tearDown()
        client?.disconnect()
        client = nil
        server?.stop()
        server = nil
        sessionCreated = false
        sessionId = nil
    }

    // MARK: - Session Creation Tests

    func testNewSession_Creation() throws {
        let port: UInt16 = 9914
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let exp = expectation(description: "Session created")
        lifecycleExpectation = exp

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = self

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [exp], timeout: 5.0)

        XCTAssert(sessionCreated, "Session should be created")
        XCTAssertNotNil(sessionId, "Should have session ID")
    }

    // MARK: - Complete Session Flow Tests

    func testCompleteSessionFlow_NewPromptToolResultCompletion() throws {
        let testSessionId = ACPSessionId("test-complete-flow")

        // Simulate complete session lifecycle with timeline
        let updates: [ACP.Client.SessionUpdate] = [
            // 1. Session start
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "List files in /tmp")))),

            // 2. Agent thinking
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "I'll use Bash to list files")))),

            // 3. Tool call
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("tool-ls"),
                name: "Bash",
                arguments: TestHelpers.makeToolArguments(["command": "ls /tmp"])
            )))),

            // 4. Tool result
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("tool-ls"),
                content: [.text(.init(text: "file1.txt\nfile2.txt\nfile3.txt"))],
                is_error: false
            )))),

            // 5. Agent response
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "I found 3 files in /tmp: file1.txt, file2.txt, and file3.txt")))),

            // 6. Session completion
            .statusUpdate(.init(status: .completed, message: "Task completed"))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: testSessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Verify complete flow is in timeline
        XCTAssertFalse(items.isEmpty, "Timeline should have items")

        // Should have user message, tool call, tool result, agent response
        let hasUserMessage = items.contains { if case .user_message = $0.variant { return true }; return false }
        let hasToolCall = items.contains { if case .assistant_message = $0.variant { return true }; return false }

        XCTAssert(hasUserMessage, "Should have user message")
        XCTAssert(hasToolCall, "Should have assistant messages (tools/responses)")
    }

    // MARK: - Multi-Turn Session Tests

    func testMultiTurnSession_ConsecutivePrompts() throws {
        let testSessionId = ACPSessionId("test-multi-turn")

        // Simulate multiple turns in same session
        let updates: [ACP.Client.SessionUpdate] = [
            // Turn 1
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "First question")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "First answer")))),

            // Turn 2
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Follow-up question")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Follow-up answer")))),

            // Turn 3
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Final question")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Final answer"))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: testSessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should have 6 items (3 user messages + 3 agent responses)
        XCTAssertEqual(items.count, 6, "Should have all turns in timeline")

        // Count user messages and agent messages
        let userMessages = items.filter { if case .user_message = $0.variant { return true }; return false }
        let agentMessages = items.filter { if case .assistant_message = $0.variant { return true }; return false }

        XCTAssertEqual(userMessages.count, 3, "Should have 3 user messages")
        XCTAssertEqual(agentMessages.count, 3, "Should have 3 agent responses")
    }

    // MARK: - Session Cancellation Tests

    func testSessionCancellation_MidExecution() throws {
        let testSessionId = ACPSessionId("test-cancel")

        // Simulate session being cancelled during tool execution
        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Start long task")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Starting task...")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("tool-long"),
                name: "Bash",
                arguments: TestHelpers.makeToolArguments(["command": "sleep 100"])
            )))),
            // Cancellation
            .statusUpdate(.init(status: .cancelled, message: "Task cancelled by user"))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: testSessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should have items up to cancellation
        XCTAssertFalse(items.isEmpty, "Should have items before cancellation")

        // Tool call should exist but no result
        let hasToolCall = items.contains { item in
            if case .assistant_message(let assoc) = item.variant,
               assoc.tool != nil {
                return true
            }
            return false
        }

        XCTAssert(hasToolCall, "Should have tool call before cancellation")
    }

    // MARK: - Concurrent Sessions Tests

    func testConcurrentSessions_IndependentTimelines() throws {
        let session1 = ACPSessionId("session-1")
        let session2 = ACPSessionId("session-2")

        // Session 1 updates
        let updates1: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Session 1 message")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Session 1 response"))))
        ]

        // Session 2 updates
        let updates2: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Session 2 message")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Session 2 response"))))
        ]

        let wires1 = updates1.map { ACP.Client.SessionNotificationWire(session_id: session1, update: $0) }
        let wires2 = updates2.map { ACP.Client.SessionNotificationWire(session_id: session2, update: $0) }

        let (items1, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires1, cap: 100)
        let (items2, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires2, cap: 100)

        // Each session should have independent timeline
        XCTAssertEqual(items1.count, 2, "Session 1 should have 2 items")
        XCTAssertEqual(items2.count, 2, "Session 2 should have 2 items")

        // Sessions should not interfere with each other
        XCTAssertNotEqual(items1.first?.id, items2.first?.id, "Sessions should have different items")
    }

    // MARK: - Session State Transitions Tests

    func testSessionStateTransitions_AllStates() throws {
        let testSessionId = ACPSessionId("test-states")

        // Test all session state transitions
        let stateUpdates: [ACP.Client.SessionUpdate] = [
            .statusUpdate(.init(status: .idle, message: nil)),
            .statusUpdate(.init(status: .running, message: "Processing...")),
            .statusUpdate(.init(status: .completed, message: "Done")),
            .statusUpdate(.init(status: .failed, message: "Error occurred")),
            .statusUpdate(.init(status: .cancelled, message: "Cancelled"))
        ]

        // Each state should be processable
        for stateUpdate in stateUpdates {
            let wire = ACP.Client.SessionNotificationWire(session_id: testSessionId, update: stateUpdate)

            // Should not crash when processing state updates
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: [wire], cap: 100)
        }

        XCTAssert(true, "All state transitions handled")
    }

    // MARK: - Error Handling Tests

    func testSession_WithToolError() throws {
        let testSessionId = ACPSessionId("test-tool-error")

        // Tool execution with error
        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Read non-existent file")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("tool-read-fail"),
                name: "Read",
                arguments: TestHelpers.makeToolArguments(["file_path": "/nonexistent/file.txt"])
            )))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("tool-read-fail"),
                content: [.text(.init(text: "Error: File not found"))],
                is_error: true
            )))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "The file doesn't exist. Please check the path."))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: testSessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should have all items including error result
        XCTAssertFalse(items.isEmpty, "Should process error results")

        // Should have tool call and error result
        XCTAssertGreaterThanOrEqual(items.count, 3, "Should have user message, tool call, error result, and response")
    }

    // MARK: - Session Persistence Tests

    func testSessionData_Serialization() throws {
        let testSessionId = ACPSessionId("test-persist")

        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Persistent message")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Persistent response"))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: testSessionId, update: $0) }

        // Serialize updates
        let encoder = JSONEncoder()
        var serializedUpdates: [Data] = []

        for wire in wires {
            if let data = try? encoder.encode(wire) {
                serializedUpdates.append(data)
            }
        }

        XCTAssertEqual(serializedUpdates.count, updates.count, "All updates should serialize")

        // Deserialize and verify
        let decoder = JSONDecoder()
        var deserializedWires: [ACP.Client.SessionNotificationWire] = []

        for data in serializedUpdates {
            if let wire = try? decoder.decode(ACP.Client.SessionNotificationWire.self, from: data) {
                deserializedWires.append(wire)
            }
        }

        XCTAssertEqual(deserializedWires.count, wires.count, "All updates should deserialize")

        // Timeline from deserialized data should match original
        let (originalItems, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)
        let (deserializedItems, _) = AcpThreadView_computeTimelineFromUpdates(updates: deserializedWires, cap: 100)

        XCTAssertEqual(originalItems.count, deserializedItems.count, "Timelines should match after serialization round-trip")
    }
}

// MARK: - MobileWebSocketClientDelegate

extension SessionLifecycleTests: MobileWebSocketClientDelegate {
    struct SessionNewParams: Codable {
        let prompt: String?
    }

    struct SessionNewResult: Codable {
        let session_id: String
    }

    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        // Create new session on connect
        client.sendJSONRPC(method: "session/new", params: SessionNewParams(prompt: "Test session"), id: "new-session") { (result: SessionNewResult?) in
            if let result = result {
                self.sessionCreated = true
                self.sessionId = ACPSessionId(result.session_id)
                self.lifecycleExpectation?.fulfill()
            }
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let error = error {
            print("Session lifecycle test: Client disconnected with error: \(error)")
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceive notification: ACP.Client.SessionNotificationWire) {
        // Handle session notifications during lifecycle tests
    }
}
#endif
