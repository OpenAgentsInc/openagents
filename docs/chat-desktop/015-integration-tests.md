# Issue #15: Write Integration Tests for Chat Flow and Session Management

## Phase
Phase 5: Testing & Documentation

## Priority
High - Quality assurance

## Description
Write comprehensive integration tests for the complete chat flow and session management to ensure reliability and prevent regressions.

## Current State
- Some unit tests exist for core components
- No integration tests for macOS chat flow
- Manual testing only for end-to-end scenarios
- No automated test coverage for session management

## Target State
- Integration tests for complete chat flow (send → receive → render)
- Session management tests (create, switch, delete, persist)
- Bridge communication tests (WebSocket, JSON-RPC)
- UI rendering tests for messages, tool calls, plans
- 70%+ test coverage for new macOS code
- Tests run in CI/CD pipeline

## Acceptance Criteria
- [ ] Integration test suite for chat flow
- [ ] Session management test suite
- [ ] Bridge communication test suite
- [ ] UI rendering snapshot tests
- [ ] Tests pass on macOS simulator and device
- [ ] Coverage report shows 70%+ for new code
- [ ] Tests run fast (< 30 seconds total)
- [ ] No flaky tests (tests are deterministic)

## Technical Details

### Test Structure
```swift
// Option A: app target UI tests (new target)
// ios/OpenAgentsAppTests/ChatFlowIntegrationTests.swift
import XCTest
@testable import OpenAgents

@MainActor
final class ChatFlowIntegrationTests: XCTestCase {
    var bridgeManager: BridgeManager!
    var tinyvexManager: TinyvexManager!
    var mockDesktopBridge: MockDesktopBridge!

    override func setUp() async throws {
        // Set up test environment
        tinyvexManager = TinyvexManager(isTest: true)
        bridgeManager = BridgeManager()
        // For macOS, wire a local dispatcher or mock RPC to feed TimelineStore
        bridgeManager.wireConnection(MockConnectionManager())
    }

    override func tearDown() async throws {
        // Clean up test data
        tinyvexManager.deleteAllData()
        bridgeManager = nil
        mockDesktopBridge = nil
    }

    // MARK: - Chat Flow Tests

    func testSendMessageCreatesNewSessionIfNoneExists() async throws {
        // Given: No active session
        XCTAssertNil(bridgeManager.currentSessionId)
        XCTAssertTrue(bridgeManager.updates.isEmpty)

        // When: User sends a message
        bridgeManager.sendPrompt(text: "Hello, agent!")

        // Then: New session created and message added
        XCTAssertNotNil(bridgeManager.currentSessionId)
        XCTAssertEqual(bridgeManager.updates.count, 1)
        XCTAssertEqual(bridgeManager.updates.first?.role, .user)

        if case .text(let text) = bridgeManager.updates.first?.content {
            XCTAssertEqual(text.text, "Hello, agent!")
        } else {
            XCTFail("Expected text content")
        }
    }

    func testReceiveAgentResponseAppendsToUpdates() async throws {
        // Given: Active session with user message
        bridgeManager.sendPrompt(text: "Test message")
        let initialCount = bridgeManager.updates.count

        // When: Agent responds
        let agentResponse = ACP.Client.SessionNotificationWire(
            role: .assistant,
            content: .text(ACP.Client.TextContent(text: "Agent response")),
            sessionId: bridgeManager.currentSessionId
        )
        // Simulate update flowing through TimelineStore
        bridgeManager.timeline.applySessionUpdatePayload(try JSONEncoder().encode(agentResponse))

        // Then: Response appended
        XCTAssertEqual(bridgeManager.updates.count, initialCount + 1)
        XCTAssertEqual(bridgeManager.updates.last?.role, .assistant)
    }

    func testToolCallUpdatesStatus() async throws {
        // Given: Active session
        bridgeManager.startNewSession()

        // When: Tool call received
        let toolUse = ACP.Client.ToolUse(
            id: "tool-123",
            name: "read_file",
            input: ["path": "/test.txt"]
        )
        let toolUpdate = ACP.Client.SessionNotificationWire(
            role: .assistant,
            content: .toolUse(toolUse),
            sessionId: bridgeManager.currentSessionId
        )
        bridgeManager.handleSessionUpdate(toolUpdate)

        // Then: Tool call tracked
        XCTAssertEqual(bridgeManager.toolCallNames["tool-123"], "read_file")
        XCTAssertTrue(bridgeManager.updates.contains(where: { update in
            if case .toolUse(let tool) = update.content {
                return tool.id == "tool-123"
            }
            return false
        }))
    }

    func testToolResultStoresOutput() async throws {
        // Given: Active session with tool call
        bridgeManager.startNewSession()
        let toolUse = ACP.Client.ToolUse(id: "tool-123", name: "read_file", input: [:])
        bridgeManager.handleSessionUpdate(ACP.Client.SessionNotificationWire(
            role: .assistant,
            content: .toolUse(toolUse),
            sessionId: bridgeManager.currentSessionId
        ))

        // When: Tool result received
        let result = ACP.Client.ToolResult(
            toolUseId: "tool-123",
            content: [
                ACP.Client.ToolResultContent(
                    type: "json",
                    text: nil,
                    json: #"{"data": "test"}"#
                )
            ],
            isError: false
        )
        bridgeManager.handleSessionUpdate(ACP.Client.SessionNotificationWire(
            role: .assistant,
            content: .toolResult(result),
            sessionId: bridgeManager.currentSessionId
        ))

        // Then: Output stored
        XCTAssertEqual(bridgeManager.outputJSONByCallId["tool-123"], #"{"data": "test"}"#)
    }

    func testErrorMessageDisplaysInChat() async throws {
        // Given: Active session
        try await bridgeManager.startNewSession()

        // When: Error occurs
        let errorUpdate = ACP.Client.SessionNotificationWire(
            role: .assistant,
            content: .error(ACP.Client.ErrorContent(
                message: "Connection failed",
                code: "BRIDGE_ERROR"
            )),
            sessionId: bridgeManager.currentSessionId
        )
        bridgeManager.handleSessionUpdate(errorUpdate)

        // Then: Error in updates
        XCTAssertTrue(bridgeManager.updates.contains(where: { update in
            if case .error = update.content {
                return true
            }
            return false
        }))
    }

    // MARK: - Session Management Tests

    func testNewSessionCreatesEmptySession() async throws {
        // When: New session created
        try await bridgeManager.startNewSession()

        // Then: Session exists with no messages
        XCTAssertNotNil(bridgeManager.currentSessionId)
        XCTAssertTrue(bridgeManager.updates.isEmpty)
        XCTAssertEqual(bridgeManager.threads.count, 1)
        XCTAssertEqual(bridgeManager.threads.first?.title, "New Chat")
    }

    func testSwitchSessionLoadsCorrectHistory() async throws {
        // Given: Two sessions with different messages
        try await bridgeManager.startNewSession()
        let session1Id = bridgeManager.currentSessionId!
        bridgeManager.sendPrompt(text: "Message in session 1")

        bridgeManager.startNewSession()
        let session2Id = bridgeManager.currentSessionId!
        bridgeManager.sendPrompt(text: "Message in session 2")

        // When: Switch back to session 1
        bridgeManager.loadSession(session1Id)

        // Then: Correct history loaded
        XCTAssertEqual(bridgeManager.currentSessionId, session1Id)
        XCTAssertEqual(bridgeManager.updates.count, 1)
        if case .text(let text) = bridgeManager.updates.first?.content {
            XCTAssertEqual(text.text, "Message in session 1")
        }
    }

    func testDeleteSessionRemovesFromThreads() async throws {
        // Given: Active session
        bridgeManager.startNewSession()
        let sessionId = bridgeManager.currentSessionId!

        // When: Session deleted
        await bridgeManager.deleteSession(sessionId)

        // Then: Removed from threads
        XCTAssertFalse(bridgeManager.threads.contains(where: { $0.id == sessionId }))
        XCTAssertNotEqual(bridgeManager.currentSessionId, sessionId)
    }

    func testSessionPersistsAcrossReloads() async throws {
        // Given: Session with messages
        try await bridgeManager.startNewSession()
        let sessionId = bridgeManager.currentSessionId!
        bridgeManager.sendPrompt(text: "Persisted message")

        // Save to database
        bridgeManager.updates.forEach { update in
            tinyvexManager.saveUpdate(update, for: sessionId)
        }

        // When: Load all sessions in fresh manager
        let newManager = BridgeManager()
        newManager.tinyvexManager = tinyvexManager
        newManager.loadAllSessions()

        // Then: Session and messages loaded
        XCTAssertTrue(newManager.threads.contains(where: { $0.id == sessionId }))
        newManager.loadSession(sessionId)
        XCTAssertEqual(newManager.updates.count, 1)
    }

    // MARK: - Bridge Communication Tests

    func testBridgeConnectsSuccessfully() async throws {
        // When: Bridge connects
        mockDesktopBridge.simulateConnection()

        // Then: Status updated
        await bridgeManager.handleConnectionStatusChange(.connected)
        XCTAssertEqual(bridgeManager.status, .connected)
    }

    func testBridgeDisconnectClearsState() async throws {
        // Given: Connected bridge
        mockDesktopBridge.simulateConnection()
        await bridgeManager.handleConnectionStatusChange(.connected)

        // When: Bridge disconnects
        mockDesktopBridge.simulateDisconnection()
        await bridgeManager.handleConnectionStatusChange(.disconnected)

        // Then: Status updated
        XCTAssertEqual(bridgeManager.status, .disconnected)
    }

    func testJSONRPCMessageParsing() throws {
        // Given: JSON-RPC message
        let jsonString = """
        {
            "role": "assistant",
            "content": {
                "type": "text",
                "text": "Test response"
            },
            "sessionId": "test-session"
        }
        """

        // When: Parsing message
        let data = jsonString.data(using: .utf8)!
        let update = try JSONDecoder().decode(
            ACP.Client.SessionNotificationWire.self,
            from: data
        )

        // Then: Correctly parsed
        XCTAssertEqual(update.role, .assistant)
        if case .text(let text) = update.content {
            XCTAssertEqual(text.text, "Test response")
        } else {
            XCTFail("Expected text content")
        }
    }
}

// MARK: - Mock Desktop Bridge
class MockDesktopBridge {
    var isConnected = false
    var sentMessages: [String] = []

    func simulateConnection() {
        isConnected = true
    }

    func simulateDisconnection() {
        isConnected = false
    }

    func sendPrompt(sessionId: String, prompt: String, agent: String?) async throws {
        guard isConnected else {
            throw BridgeError.notConnected
        }
        sentMessages.append(prompt)
    }

    func createSession(workingDirectory: String?) async throws -> (sessionId: String) {
        guard isConnected else {
            throw BridgeError.notConnected
        }
        return (sessionId: UUID().uuidString)
    }
}
```

Option B: extend existing Core integration tests
- Add scenarios to `DesktopWebSocketServerComprehensiveTests.swift` and `BridgeServerClientTests.swift` that cover `session/new`, `session/prompt`, `session/update` flows and Tinyvex history queries.

### UI Rendering Tests
```swift
// ios/OpenAgentsTests/macOS/ChatUIRenderingTests.swift
import XCTest
import SwiftUI
import ViewInspector
@testable import OpenAgents

@MainActor
final class ChatUIRenderingTests: XCTestCase {
    func testMessageBubbleRendersUserMessage() throws {
        // Given: User message
        let update = ACP.Client.SessionNotificationWire(
            role: .user,
            content: .text(ACP.Client.TextContent(text: "Test message")),
            sessionId: "test"
        )

        // When: Rendering bubble
        let view = MessageBubbleView(
            update: update,
            bridgeManager: BridgeManager()
        )

        // Then: Contains message text
        let text = try view.inspect().find(text: "Test message")
        XCTAssertNotNil(text)
    }

    func testToolCallViewRendersCorrectly() throws {
        // Given: Tool call
        let toolUse = ACP.Client.ToolUse(
            id: "tool-123",
            name: "read_file",
            input: ["path": "/test.txt"]
        )

        // When: Rendering tool call view
        let bridgeManager = BridgeManager()
        bridgeManager.toolCallNames["tool-123"] = "read_file"

        let view = ToolCallView(
            callId: "tool-123",
            toolName: "read_file",
            status: .running,
            percentage: nil,
            bridgeManager: bridgeManager
        )

        // Then: Displays tool name
        let text = try view.inspect().find(text: "read_file")
        XCTAssertNotNil(text)
    }
}
```

### Performance Tests
```swift
// ios/OpenAgentsTests/macOS/ChatPerformanceTests.swift
import XCTest
@testable import OpenAgents

final class ChatPerformanceTests: XCTestCase {
    func testLargeMessageListRenders() throws {
        // Measure time to render 1000 messages
        let bridgeManager = BridgeManager()

        // Add 1000 messages
        for i in 0..<1000 {
            bridgeManager.updates.append(ACP.Client.SessionNotificationWire(
                role: i % 2 == 0 ? .user : .assistant,
                content: .text(ACP.Client.TextContent(text: "Message \(i)")),
                sessionId: "test"
            ))
        }

        // Measure rendering (should be < 100ms)
        measure {
            let view = ChatAreaView()
                .environmentObject(bridgeManager)
            _ = view.body
        }
    }

    func testSessionSwitchingPerformance() async throws {
        // Measure time to switch between sessions
        let bridgeManager = BridgeManager()
        let tinyvexManager = TinyvexManager(isTest: true)

        // Create 10 sessions with 100 messages each
        for sessionIndex in 0..<10 {
            try await bridgeManager.startNewSession()
            for i in 0..<100 {
                bridgeManager.updates.append(ACP.Client.SessionNotificationWire(
                    role: .user,
                    content: .text(ACP.Client.TextContent(text: "Message \(i)")),
                    sessionId: bridgeManager.currentSessionId
                ))
            }
        }

        // Measure switching (should be < 50ms)
        measure {
            bridgeManager.loadSession(bridgeManager.threads[0].id)
        }
    }
}
```

### Test Coverage Configuration
```swift
// Enable code coverage in scheme
// Edit Scheme > Test > Options > Code Coverage ✅
// Gather coverage for targets: OpenAgents, OpenAgentsCore
```

## Dependencies
- All implementation issues (#1-#14)

## Blocked By
- All implementation issues

## Blocks
None - Testing is final validation

## Estimated Complexity
High (6-8 hours)

## Testing Requirements
- [ ] All tests pass on macOS simulator
- [ ] All tests pass on macOS device
- [ ] Tests run in < 30 seconds
- [ ] No flaky tests (run 10 times, all pass)
- [ ] Coverage report generated
- [ ] Coverage > 70% for new macOS code
- [ ] Performance tests meet targets

## References
- Existing tests: `ios/OpenAgentsTests/`
- XCTest: https://developer.apple.com/documentation/xctest
- ViewInspector: https://github.com/nalexn/ViewInspector
- Swift Testing best practices
