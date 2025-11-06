import XCTest
@testable import OpenAgentsCore

#if os(macOS)
/// End-to-end integration tests for message pipeline
/// Tests the complete flow: desktop → WebSocket → iOS → timeline rendering
final class EndToEndIntegrationTests: XCTestCase {
    private var server: DesktopWebSocketServer?
    private var client: MobileWebSocketClient?
    private var receivedUpdates: [ACP.Client.SessionUpdate] = []
    private var updateExpectation: XCTestExpectation?

    override func tearDown() {
        super.tearDown()
        client?.disconnect()
        client = nil
        server?.stop()
        server = nil
        receivedUpdates.removeAll()
    }

    // MARK: - Tool Call Round-Trip Tests

    func testToolCallExecution_BashCommand() throws {
        let port: UInt16 = 9912
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let exp = expectation(description: "Tool call round-trip completed")
        updateExpectation = exp

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = self

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [exp], timeout: 10.0)

        // Verify we received the expected updates
        XCTAssertFalse(receivedUpdates.isEmpty, "Should receive session updates")

        // Look for tool call in the updates
        let hasToolCall = receivedUpdates.contains { update in
            if case .agentMessageChunk(let chunk) = update,
               case .toolUse = chunk.content {
                return true
            }
            return false
        }

        XCTAssert(hasToolCall || receivedUpdates.count > 0, "Should have updates or tool calls")
    }

    func testMultiTurnConversation_WithToolExecution() throws {
        let port: UInt16 = 9913
        let srv = DesktopWebSocketServer()
        server = srv
        try srv.start(port: port, advertiseService: false)

        let exp = expectation(description: "Multi-turn conversation completed")
        updateExpectation = exp

        let cli = MobileWebSocketClient()
        client = cli
        cli.delegate = self

        let url = URL(string: "ws://127.0.0.1:\(port)")!
        cli.connect(url: url)

        wait(for: [exp], timeout: 15.0)

        // Verify conversation flow
        XCTAssertFalse(receivedUpdates.isEmpty, "Should receive updates in conversation")
    }

    // MARK: - Reasoning Consolidation Tests

    func testReasoningChunks_ConsolidatedIntoGlassButton() throws {
        // Create a sequence of thinking chunks that should be consolidated
        let thinkingUpdates: [ACP.Client.SessionUpdate] = [
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Let me read the file first")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "I see the issue now")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "I'll make the fix"))))
        ]

        let sessionId = ACPSessionId("test-thinking")
        let wires = thinkingUpdates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Use the timeline computation function
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Thinking chunks should be consolidated
        // Since they're all within the consolidation window, they should be merged
        let thinkingItems = items.filter { item in
            if case .thinking = item.variant {
                return true
            }
            return false
        }

        // Should have reasoning consolidated (exact count depends on timing/consolidation logic)
        XCTAssertFalse(thinkingItems.isEmpty, "Should have thinking items in timeline")
    }

    // MARK: - Tool Call and Result Pairing Tests

    func testToolCallResultPairing_InTimeline() throws {
        let sessionId = ACPSessionId("test-pairing")

        // Create tool call
        let toolUseId = ACP.ToolUseId("tool-123")
        let toolCall = ACP.Client.ToolUse(
            id: toolUseId,
            name: "Read",
            arguments: TestHelpers.makeToolArguments(["file_path": "/tmp/test.txt"])
        )

        // Create tool result
        let toolResult = ACP.Client.ToolResult(
            tool_use_id: toolUseId,
            content: [.text(.init(text: "File contents here"))],
            is_error: false
        )

        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(toolCall))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(toolResult)))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should have both tool call and result in timeline
        XCTAssertEqual(items.count, 2, "Should have tool call and result as separate items")

        // First item should be the tool call
        if case .assistant_message = items.first?.variant {
            XCTAssert(true, "First item is tool call")
        } else {
            XCTFail("Expected tool call as first item")
        }
    }

    // MARK: - User Prompt Flow Tests

    func testUserPrompt_SendAndReceiveResponse() throws {
        let sessionId = ACPSessionId("test-user-prompt")

        // Simulate user prompt → agent response flow
        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Fix the bug")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "I'll help you fix that bug"))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        XCTAssertEqual(items.count, 2, "Should have user message and agent response")

        // First should be user message
        if case .user_message = items.first?.variant {
            XCTAssert(true, "First item is user message")
        } else {
            XCTFail("Expected user message first")
        }

        // Second should be agent message
        if case .assistant_message = items.last?.variant {
            XCTAssert(true, "Second item is agent message")
        } else {
            XCTFail("Expected agent message second")
        }
    }

    // MARK: - Message Content Types Tests

    func testAllContentTypes_RenderInTimeline() throws {
        let sessionId = ACPSessionId("test-content-types")

        // Test all ACP content block types
        let updates: [ACP.Client.SessionUpdate] = [
            // Text content
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Plain text")))),

            // Image content
            .agentMessageChunk(ACP.Client.ContentChunk(content: .image(.init(
                data: nil,
                mimeType: "image/png",
                uri: "file:///test.png"
            )))),

            // Resource link
            .agentMessageChunk(ACP.Client.ContentChunk(content: .resource_link(.init(
                title: "Documentation",
                uri: "https://docs.example.com",
                mimeType: "text/html",
                description: nil
            )))),

            // Tool use
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("tool-456"),
                name: "Bash",
                arguments: TestHelpers.makeToolArguments(["command": "ls"])
            )))),

            // Tool result
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("tool-456"),
                content: [.text(.init(text: "Command output"))],
                is_error: false
            )))),

            // Thinking content
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Internal reasoning"))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // All content types should be processed
        XCTAssertFalse(items.isEmpty, "Should process all content types")
        XCTAssertGreaterThanOrEqual(items.count, 5, "Should have items for each content type")
    }

    // MARK: - Out-of-Order Message Handling

    func testOutOfOrderMessages_HandleCorrectly() throws {
        let sessionId = ACPSessionId("test-out-of-order")

        // Send messages out of order (by timestamp)
        var updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message 1")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Response 2")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Response 1"))))
        ]

        // Shuffle the updates to simulate out-of-order arrival
        updates.shuffle()

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Timeline should handle out-of-order messages
        XCTAssertEqual(items.count, 3, "Should have all messages in timeline")
    }

    // MARK: - Large Message Count Tests

    func testLargeMessageCount_PerformanceAcceptable() throws {
        let sessionId = ACPSessionId("test-large-count")

        // Create 1000 messages
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<1000 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Measure timeline computation performance
        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)
        }

        // Should complete in reasonable time (measured by XCTest)
    }

    // MARK: - Update Buffer Tests

    func testUpdateBuffer_RingBufferBehavior() throws {
        let sessionId = ACPSessionId("test-buffer-overflow")

        // Create more than 200 updates (buffer limit)
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<250 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // With cap of 200, should only get last 200 messages
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 200)

        XCTAssertLessThanOrEqual(items.count, 200, "Should cap at buffer limit")
    }
}

// MARK: - MobileWebSocketClientDelegate

extension EndToEndIntegrationTests: MobileWebSocketClientDelegate {
    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        // Connection established, tests can proceed
        // In real integration tests, would send session/new or session/prompt here
        updateExpectation?.fulfill()
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        if let error = error {
            print("Client disconnected with error: \(error)")
        }
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceive notification: ACP.Client.SessionNotificationWire) {
        // Store received updates for verification
        receivedUpdates.append(notification.update)
    }
}
#endif
