import XCTest
import Combine
@testable import OpenAgents
@testable import OpenAgentsCore

/// Integration tests for sending prompts and verifying user messages appear in timeline
@MainActor
final class PromptSendingIntegrationTests: XCTestCase {
    var bridge: BridgeManager!
    var cancellables: Set<AnyCancellable>!

    override func setUp() async throws {
        try await super.setUp()
        bridge = BridgeManager()
        cancellables = []
    }

    override func tearDown() async throws {
        bridge.stop()
        bridge = nil
        cancellables = nil
        try await super.tearDown()
    }

    // MARK: - Optimistic UI Tests

    func testSendPrompt_AddsUserMessageImmediately() {
        bridge.currentSessionId = ACPSessionId("test-session")
        let initialCount = bridge.updates.count

        bridge.sendPrompt(text: "Hello, agent!")

        // User message should be added immediately
        XCTAssertEqual(bridge.updates.count, initialCount + 1)

        // Verify it's a user message
        if let lastUpdate = bridge.updates.last {
            if case .userMessageChunk(let chunk) = lastUpdate.update,
               case .text(let content) = chunk.content {
                XCTAssertEqual(content.text, "Hello, agent!")
            } else {
                XCTFail("Last update should be a user message chunk")
            }
        } else {
            XCTFail("Should have at least one update")
        }
    }

    func testSendPrompt_FiresObjectWillChange() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let expectation = expectation(description: "objectWillChange fires")
        let cancellable = bridge.objectWillChange.sink { _ in
            expectation.fulfill()
        }

        bridge.sendPrompt(text: "Test message")

        wait(for: [expectation], timeout: 1.0)
        cancellable.cancel()
    }

    func testSendPrompt_WithoutSession_CreatesOptimisticMessage() {
        // No session yet
        XCTAssertNil(bridge.currentSessionId)

        bridge.sendPrompt(text: "First message")

        // Should still create optimistic user message
        XCTAssertEqual(bridge.updates.count, 1)

        if let update = bridge.updates.first {
            if case .userMessageChunk(let chunk) = update.update,
               case .text(let content) = chunk.content {
                XCTAssertEqual(content.text, "First message")
            } else {
                XCTFail("Should be a user message chunk")
            }
        }
    }

    func testMultiplePrompts_AllAppearInOrder() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let messages = ["First", "Second", "Third"]

        for msg in messages {
            bridge.sendPrompt(text: msg)
        }

        XCTAssertEqual(bridge.updates.count, 3)

        // Verify order
        for (index, expectedText) in messages.enumerated() {
            if case .userMessageChunk(let chunk) = bridge.updates[index].update,
               case .text(let content) = chunk.content {
                XCTAssertEqual(content.text, expectedText)
            } else {
                XCTFail("Update \(index) should be user message")
            }
        }
    }

    func testSendPrompt_SpecialCharacters_PreservedCorrectly() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let specialText = "Test with \"quotes\", 'apostrophes', and symbols: @#$%^&*()"
        bridge.sendPrompt(text: specialText)

        if let lastUpdate = bridge.updates.last,
           case .userMessageChunk(let chunk) = lastUpdate.update,
           case .text(let content) = chunk.content {
            XCTAssertEqual(content.text, specialText)
        } else {
            XCTFail("Should preserve special characters")
        }
    }

    func testSendPrompt_Multiline_PreservedCorrectly() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let multilineText = """
        Line 1
        Line 2
        Line 3
        """
        bridge.sendPrompt(text: multilineText)

        if let lastUpdate = bridge.updates.last,
           case .userMessageChunk(let chunk) = lastUpdate.update,
           case .text(let content) = chunk.content {
            XCTAssertEqual(content.text, multilineText)
        } else {
            XCTFail("Should preserve multiline text")
        }
    }

    func testSendPrompt_EmptyText_StillCreatesUpdate() {
        bridge.currentSessionId = ACPSessionId("test-session")

        bridge.sendPrompt(text: "")

        // Even empty text should create an update (UI may filter it)
        XCTAssertEqual(bridge.updates.count, 1)
    }

    func testSendPrompt_VeryLongText_HandledCorrectly() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let longText = String(repeating: "a", count: 50000)
        bridge.sendPrompt(text: longText)

        if let lastUpdate = bridge.updates.last,
           case .userMessageChunk(let chunk) = lastUpdate.update,
           case .text(let content) = chunk.content {
            XCTAssertEqual(content.text.count, 50000)
        } else {
            XCTFail("Should handle very long text")
        }
    }

    func testSendPrompt_Unicode_PreservedCorrectly() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let unicodeText = "Hello 👋 世界 🌍 مرحبا"
        bridge.sendPrompt(text: unicodeText)

        if let lastUpdate = bridge.updates.last,
           case .userMessageChunk(let chunk) = lastUpdate.update,
           case .text(let content) = chunk.content {
            XCTAssertEqual(content.text, unicodeText)
        } else {
            XCTFail("Should preserve unicode")
        }
    }

    // MARK: - Ring Buffer Integration

    func testSendPrompt_AtCapacity_MaintainsRingBuffer() {
        bridge.currentSessionId = ACPSessionId("test-session")

        // Fill to capacity
        for i in 1...200 {
            let update = TestHelpers.makeSessionUpdateNotification(
                update: TestHelpers.makeTextUpdate(text: "fill \(i)")
            )
            bridge.updates.append(update)
        }

        XCTAssertEqual(bridge.updates.count, 200)

        // Send new prompt
        bridge.sendPrompt(text: "New message at capacity")

        // Should maintain capacity
        XCTAssertEqual(bridge.updates.count, 200)

        // Should have the new message as the last update
        if let lastUpdate = bridge.updates.last,
           case .userMessageChunk(let chunk) = lastUpdate.update,
           case .text(let content) = chunk.content {
            XCTAssertEqual(content.text, "New message at capacity")
        } else {
            XCTFail("Last update should be the new user message")
        }
    }

    // MARK: - Observer Integration

    func testSendPrompt_TriggersUIObservers() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let expectation = expectation(description: "UI observer fires")
        var receivedUpdate: ACP.Client.SessionNotificationWire?

        // Simulate AcpThreadView observer
        let cancellable = bridge.objectWillChange.sink { [weak bridge] _ in
            receivedUpdate = bridge?.updates.last
            expectation.fulfill()
        }

        bridge.sendPrompt(text: "Observer test")

        wait(for: [expectation], timeout: 1.0)

        XCTAssertNotNil(receivedUpdate)
        if let update = receivedUpdate,
           case .userMessageChunk(let chunk) = update.update,
           case .text(let content) = chunk.content {
            XCTAssertEqual(content.text, "Observer test")
        }

        cancellable.cancel()
    }

    // MARK: - Error Cases

    func testSendPrompt_NoClient_DoesNotCrash() {
        // Don't start connection, so client is nil
        XCTAssertNil(bridge.currentSessionId)

        // Should not crash even with no client
        bridge.sendPrompt(text: "No client test")

        // Should not have added update without client
        XCTAssertEqual(bridge.updates.count, 0)
    }

    // MARK: - Timing Tests

    func testSendPrompt_ImmediateAvailability() {
        bridge.currentSessionId = ACPSessionId("test-session")

        let beforeCount = bridge.updates.count

        bridge.sendPrompt(text: "Immediate")

        // Should be available synchronously
        XCTAssertEqual(bridge.updates.count, beforeCount + 1)

        // No need to wait for async completion
        if let lastUpdate = bridge.updates.last,
           case .userMessageChunk = lastUpdate.update {
            XCTAssert(true, "Update available immediately")
        } else {
            XCTFail("Update should be available immediately")
        }
    }

    func testRapidFirePrompts_AllCaptured() {
        bridge.currentSessionId = ACPSessionId("test-session")

        // Send 20 prompts in rapid succession
        for i in 1...20 {
            bridge.sendPrompt(text: "Rapid \(i)")
        }

        // All should be captured
        XCTAssertEqual(bridge.updates.count, 20)

        // Verify they're all user messages
        let userMessageCount = bridge.updates.filter { update in
            if case .userMessageChunk = update.update {
                return true
            }
            return false
        }.count

        XCTAssertEqual(userMessageCount, 20)
    }

    // MARK: - Session ID Handling

    func testSendPrompt_PendingSessionId_UsedForOptimisticUpdate() {
        // No session yet
        XCTAssertNil(bridge.currentSessionId)

        bridge.sendPrompt(text: "Pending session")

        // Should have created update with "pending" session
        if let update = bridge.updates.first {
            XCTAssertEqual(update.session_id.value, "pending")
        } else {
            XCTFail("Should have created optimistic update")
        }
    }

    func testSendPrompt_ExistingSessionId_UsedForOptimisticUpdate() {
        let testSessionId = ACPSessionId("test-session-123")
        bridge.currentSessionId = testSessionId

        bridge.sendPrompt(text: "Existing session")

        if let update = bridge.updates.first {
            XCTAssertEqual(update.session_id, testSessionId)
        } else {
            XCTFail("Should have created optimistic update")
        }
    }
}
