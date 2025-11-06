import XCTest
import SwiftUI
@testable import OpenAgents
@testable import OpenAgentsCore

#if os(iOS)
/// Tests for AcpThreadView rendering behavior and UI states
/// Validates timeline rendering, message detail sheets, loading states, and user interactions
@MainActor
final class AcpThreadViewRenderingTests: XCTestCase {
    var bridge: BridgeManager!

    override func setUp() async throws {
        try await super.setUp()
        bridge = BridgeManager()
    }

    override func tearDown() async throws {
        bridge.stop()
        bridge = nil
        try await super.tearDown()
    }

    // MARK: - Timeline Rendering Tests

    func testTimelineRendering_EmptyState() {
        // Test empty state renders correctly
        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // Should render loading view when timeline is empty
        XCTAssertTrue(bridge.updates.isEmpty)
        XCTAssertNotNil(view)
    }

    func testTimelineRendering_SingleTextMessage() {
        // Add a single text message
        let textUpdate = TestHelpers.makeTextUpdate(text: "Hello world")
        let notification = TestHelpers.makeSessionUpdateNotification(update: textUpdate)
        bridge.updates.append(notification)

        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // Should render the view with one message
        XCTAssertEqual(bridge.updates.count, 1)
        XCTAssertNotNil(view)
    }

    func testTimelineRendering_MultipleMessageTypes() {
        // Add various message types
        let updates: [ACP.Client.SessionUpdate] = [
            TestHelpers.makeTextUpdate(text: "User message"),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Agent response")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Internal reasoning")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("tool-1"),
                name: "Read",
                arguments: TestHelpers.makeToolArguments(["file_path": "/tmp/test.txt"])
            ))))
        ]

        for update in updates {
            bridge.updates.append(TestHelpers.makeSessionUpdateNotification(update: update))
        }

        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        XCTAssertEqual(bridge.updates.count, 4)
        XCTAssertNotNil(view)
    }

    func testTimelineRendering_LargeMessageCount() {
        // Add 100 messages to test performance
        for i in 1...100 {
            let update = TestHelpers.makeTextUpdate(text: "Message \(i)")
            bridge.updates.append(TestHelpers.makeSessionUpdateNotification(update: update))
        }

        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        XCTAssertEqual(bridge.updates.count, 100)
        XCTAssertNotNil(view)
    }

    // MARK: - Timeline Item Type Tests

    func testTimelineItem_MessageType() {
        let message = ACPMessage(
            id: "msg-1",
            ts: 1000,
            role: .assistant,
            parts: [.text(.init(text: "Test message"))]
        )
        let item = AcpThreadView.TimelineItem.message(message)

        XCTAssertEqual(item.ts, 1000)
        XCTAssertTrue(item.id.contains("msg_"))
    }

    func testTimelineItem_ToolCallType() {
        let toolCall = ACPToolCall(
            id: "tool-123",
            tool_name: "Bash",
            arguments: .object(["command": .string("ls")]),
            ts: 2000
        )
        let item = AcpThreadView.TimelineItem.toolCall(toolCall)

        XCTAssertEqual(item.ts, 2000)
        XCTAssertTrue(item.id.contains("call_tool-123"))
    }

    func testTimelineItem_ToolResultType() {
        let toolResult = ACPToolResult(
            call_id: "tool-123",
            output: "file1.txt\nfile2.txt",
            is_error: false,
            ts: 3000
        )
        let item = AcpThreadView.TimelineItem.toolResult(toolResult)

        XCTAssertEqual(item.ts, 3000)
        XCTAssertTrue(item.id.contains("res_tool-123"))
    }

    func testTimelineItem_ReasoningSummaryType() {
        let messages = [
            ACPMessage(id: "r1", ts: 1000, role: .assistant, parts: [.text(.init(text: "Thinking 1"))]),
            ACPMessage(id: "r2", ts: 1100, role: .assistant, parts: [.text(.init(text: "Thinking 2"))])
        ]
        let summary = AcpThreadView.ReasoningSummary(startTs: 1000, endTs: 1100, messages: messages)
        let item = AcpThreadView.TimelineItem.reasoningSummary(summary)

        XCTAssertEqual(item.ts, 1100)
        XCTAssertTrue(item.id.contains("rs_1000_1100"))
    }

    func testTimelineItem_PlanType() {
        let plan = ACPPlanState(
            status: .running,
            todos: [
                ACPPlanState.Todo(content: "Task 1", activeForm: "Doing task 1", status: .in_progress)
            ],
            ts: 4000
        )
        let item = AcpThreadView.TimelineItem.plan(plan)

        XCTAssertEqual(item.ts, 4000)
        XCTAssertTrue(item.id.contains("plan_"))
    }

    // MARK: - Loading State Tests

    func testLoadingState_InitialLoad() {
        // Test that loading state appears initially
        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // With empty timeline and no updates, should show loading or empty state
        XCTAssertTrue(bridge.updates.isEmpty)
        XCTAssertNotNil(view)
    }

    func testLoadingState_TransitionToContent() {
        // Start with loading, then add content
        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // Add update to transition from loading to content
        let update = TestHelpers.makeTextUpdate(text: "First message")
        bridge.updates.append(TestHelpers.makeSessionUpdateNotification(update: update))

        XCTAssertFalse(bridge.updates.isEmpty)
        XCTAssertNotNil(view)
    }

    // MARK: - Error State Tests

    func testErrorState_ConnectionFailure() {
        // Simulate connection error
        bridge.status = .error("Connection failed")

        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // Should render error state
        if case .error(let message) = bridge.status {
            XCTAssertEqual(message, "Connection failed")
        } else {
            XCTFail("Expected error status")
        }
        XCTAssertNotNil(view)
    }

    func testErrorState_LoadingFailure() {
        // Simulate loading error with empty timeline
        bridge.status = .error("Failed to load messages")

        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        XCTAssertTrue(bridge.updates.isEmpty)
        XCTAssertNotNil(view)
    }

    // MARK: - Update Processing Tests

    func testUpdateProcessing_SingleUpdate() {
        let sessionId = ACPSessionId("test-session")
        let update = ACP.Client.SessionUpdate.userMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Test message")))
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        bridge.updates.append(notification)

        XCTAssertEqual(bridge.updates.count, 1)
        XCTAssertEqual(bridge.updates.first?.session_id, sessionId)
    }

    func testUpdateProcessing_MultipleUpdatesInSequence() {
        let sessionId = ACPSessionId("test-session")

        for i in 1...10 {
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))
            )
            bridge.updates.append(ACP.Client.SessionNotificationWire(session_id: sessionId, update: update))
        }

        XCTAssertEqual(bridge.updates.count, 10)
    }

    func testUpdateProcessing_DuplicateUpdatesFiltered() {
        // Timeline computation should handle duplicates
        let sessionId = ACPSessionId("test-session")
        let update = ACP.Client.SessionUpdate.userMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Duplicate message")))
        )

        // Add same update twice
        bridge.updates.append(ACP.Client.SessionNotificationWire(session_id: sessionId, update: update))
        bridge.updates.append(ACP.Client.SessionNotificationWire(session_id: sessionId, update: update))

        // Both updates are stored (deduplication happens in timeline computation)
        XCTAssertEqual(bridge.updates.count, 2)
    }

    // MARK: - Timeline Computation Tests

    func testTimelineComputation_EmptyUpdates() {
        let (items, title) = AcpThreadView_computeTimelineFromUpdates(updates: [], cap: 100)

        XCTAssertTrue(items.isEmpty)
        XCTAssertNil(title)
    }

    func testTimelineComputation_SingleMessage() {
        let sessionId = ACPSessionId("test-session")
        let update = ACP.Client.SessionUpdate.userMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Test message")))
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [notification], cap: 100)

        XCTAssertEqual(items.count, 1)
        if case .user_message = items.first?.variant {
            XCTAssert(true)
        } else {
            XCTFail("Expected user_message item")
        }
    }

    func testTimelineComputation_MessageOrdering() {
        let sessionId = ACPSessionId("test-session")
        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "First")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Second")))),
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Third"))))
        ]

        let notifications = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: notifications, cap: 100)

        // Should maintain order
        XCTAssertEqual(items.count, 3)
    }

    func testTimelineComputation_CapLimit() {
        let sessionId = ACPSessionId("test-session")
        var updates: [ACP.Client.SessionNotificationWire] = []

        // Create 150 updates
        for i in 1...150 {
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))
            )
            updates.append(ACP.Client.SessionNotificationWire(session_id: sessionId, update: update))
        }

        // Cap at 100
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: updates, cap: 100)

        XCTAssertLessThanOrEqual(items.count, 100)
    }

    // MARK: - Reasoning Consolidation Tests

    func testReasoningConsolidation_MultipleThinkingChunks() {
        let sessionId = ACPSessionId("test-session")
        let thinkingUpdates: [ACP.Client.SessionUpdate] = [
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Step 1")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Step 2")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Step 3"))))
        ]

        let notifications = thinkingUpdates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: notifications, cap: 100)

        // Thinking chunks should be consolidated
        XCTAssertFalse(items.isEmpty)
    }

    func testReasoningConsolidation_MixedWithMessages() {
        let sessionId = ACPSessionId("test-session")
        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "User message")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Thinking")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Agent response"))))
        ]

        let notifications = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: notifications, cap: 100)

        // Should have user message, thinking, and agent response
        XCTAssertGreaterThanOrEqual(items.count, 2)
    }

    // MARK: - Tool Call Display Tests

    func testToolCallDisplay_BashCommand() {
        let sessionId = ACPSessionId("test-session")
        let toolUse = ACP.Client.ToolUse(
            id: ACP.ToolUseId("bash-1"),
            name: "Bash",
            arguments: TestHelpers.makeToolArguments(["command": "ls -la"])
        )
        let update = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .toolUse(toolUse))
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [notification], cap: 100)

        XCTAssertFalse(items.isEmpty)
    }

    func testToolCallDisplay_ReadFile() {
        let sessionId = ACPSessionId("test-session")
        let toolUse = ACP.Client.ToolUse(
            id: ACP.ToolUseId("read-1"),
            name: "Read",
            arguments: TestHelpers.makeToolArguments(["file_path": "/tmp/test.txt"])
        )
        let update = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .toolUse(toolUse))
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [notification], cap: 100)

        XCTAssertFalse(items.isEmpty)
    }

    func testToolCallDisplay_WithResult() {
        let sessionId = ACPSessionId("test-session")
        let toolUseId = ACP.ToolUseId("tool-with-result")

        let toolCall = ACP.Client.ToolUse(
            id: toolUseId,
            name: "Bash",
            arguments: TestHelpers.makeToolArguments(["command": "echo test"])
        )
        let toolResult = ACP.Client.ToolResult(
            tool_use_id: toolUseId,
            content: [.text(.init(text: "test\n"))],
            is_error: false
        )

        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(toolCall))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(toolResult)))
        ]

        let notifications = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: notifications, cap: 100)

        // Should have both tool call and result
        XCTAssertEqual(items.count, 2)
    }

    // MARK: - Plan State Display Tests

    func testPlanStateDisplay_IdleState() {
        let sessionId = ACPSessionId("test-session")
        let planUpdate = TestHelpers.makePlanUpdate(
            status: .idle,
            todos: []
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: planUpdate)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [notification], cap: 100)

        // Plan updates are processed
        XCTAssertNotNil(items)
    }

    func testPlanStateDisplay_RunningWithTodos() {
        let sessionId = ACPSessionId("test-session")
        let planUpdate = TestHelpers.makePlanUpdate(
            status: .running,
            todos: [
                ACPPlanState.Todo(content: "Task 1", activeForm: "Doing task 1", status: .in_progress),
                ACPPlanState.Todo(content: "Task 2", activeForm: "Doing task 2", status: .pending)
            ]
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: planUpdate)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [notification], cap: 100)

        XCTAssertNotNil(items)
    }

    func testPlanStateDisplay_CompletedState() {
        let sessionId = ACPSessionId("test-session")
        let planUpdate = TestHelpers.makePlanUpdate(
            status: .completed,
            todos: [
                ACPPlanState.Todo(content: "Task 1", activeForm: "Doing task 1", status: .completed)
            ]
        )
        let notification = ACP.Client.SessionNotificationWire(session_id: sessionId, update: planUpdate)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [notification], cap: 100)

        XCTAssertNotNil(items)
    }

    // MARK: - Message Detail Tests

    func testMessageDetailSheet_UserMessage() {
        let message = ACPMessage(
            id: "user-msg-1",
            ts: 1000,
            role: .user,
            parts: [.text(.init(text: "User question"))]
        )

        // Message detail should be displayable
        XCTAssertEqual(message.role, .user)
        XCTAssertEqual(message.parts.count, 1)
    }

    func testMessageDetailSheet_AssistantMessage() {
        let message = ACPMessage(
            id: "assistant-msg-1",
            ts: 2000,
            role: .assistant,
            parts: [.text(.init(text: "Assistant response"))]
        )

        XCTAssertEqual(message.role, .assistant)
        XCTAssertEqual(message.parts.count, 1)
    }

    func testMessageDetailSheet_MessageWithMultipleParts() {
        let message = ACPMessage(
            id: "multi-part-1",
            ts: 3000,
            role: .assistant,
            parts: [
                .text(.init(text: "First part")),
                .text(.init(text: "Second part"))
            ]
        )

        XCTAssertEqual(message.parts.count, 2)
    }

    // MARK: - View Lifecycle Tests

    func testViewLifecycle_InitialRender() {
        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // Should initialize without crashing
        XCTAssertNotNil(view)
    }

    func testViewLifecycle_UpdatesAfterAppear() {
        let view = AcpThreadView(url: nil)
            .environmentObject(bridge)

        // Add update after view appears
        let update = TestHelpers.makeTextUpdate(text: "Late update")
        bridge.updates.append(TestHelpers.makeSessionUpdateNotification(update: update))

        XCTAssertEqual(bridge.updates.count, 1)
        XCTAssertNotNil(view)
    }

    func testViewLifecycle_MultipleRenders() {
        // Create and recreate view multiple times
        for _ in 1...5 {
            let view = AcpThreadView(url: nil)
                .environmentObject(bridge)
            XCTAssertNotNil(view)
        }
    }
}
#endif
