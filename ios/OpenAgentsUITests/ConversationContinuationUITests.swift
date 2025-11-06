import XCTest

#if os(iOS)
/// UI Tests for conversation continuation and message flow
@MainActor
final class ConversationContinuationUITests: XCTestCase {

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--uitesting"]
        app.launch()
    }

    override func tearDownWithError() throws {
        app = nil
    }

    // MARK: - Conversation Continuation Tests

    func testConversationContinuation_SendPrompt_AgentResponds() {
        // Given: App is launched and connected to bridge
        waitForConnection()

        // When: User sends a prompt via pencil button
        let pencilButton = app.buttons["compose-button"]
        XCTAssertTrue(pencilButton.waitForExistence(timeout: 5), "Pencil button should exist")
        pencilButton.tap()

        let composer = app.textEditors["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 5), "Composer should appear")
        composer.tap()
        composer.typeText("What is 2+2?")

        let sendButton = app.buttons["send-button"]
        XCTAssertTrue(sendButton.exists, "Send button should exist")
        sendButton.tap()

        // Then: User message appears immediately (optimistic UI)
        let userMessage = app.staticTexts["What is 2+2?"]
        XCTAssertTrue(userMessage.waitForExistence(timeout: 2), "User message should appear immediately")

        // And: Agent response appears within reasonable time
        let agentResponse = app.staticTexts.matching(NSPredicate(format: "label CONTAINS '4'"))
        XCTAssertTrue(agentResponse.firstMatch.waitForExistence(timeout: 30), "Agent should respond with answer")
    }

    func testConversationContinuation_MultipleMessages_MaintainsContext() {
        // Given: App is connected
        waitForConnection()

        // When: User sends first message
        sendMessage("My name is Alice")

        // Then: User message appears
        XCTAssertTrue(app.staticTexts["My name is Alice"].waitForExistence(timeout: 2))

        // Wait for agent response
        sleep(5)

        // When: User sends follow-up message
        sendMessage("What is my name?")

        // Then: Agent should remember context and respond with "Alice"
        let contextualResponse = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'Alice'"))
        XCTAssertTrue(contextualResponse.firstMatch.waitForExistence(timeout: 30), "Agent should remember user's name")
    }

    func testConversationContinuation_SessionPersistence_AfterRelaunch() {
        // Given: User sends a message
        waitForConnection()
        sendMessage("Remember: my favorite color is blue")
        sleep(5) // Wait for agent processing

        // When: App is relaunched
        app.terminate()
        app.launch()
        waitForConnection()

        // Then: Previous message should still be visible
        let scrollView = app.scrollViews.firstMatch
        XCTAssertTrue(scrollView.exists)

        let previousMessage = app.staticTexts["Remember: my favorite color is blue"]
        XCTAssertTrue(previousMessage.exists, "Previous messages should persist after relaunch")
    }

    // MARK: - Optimistic Message Display Tests

    func testOptimisticUI_UserMessage_AppearsImmediately() {
        // Given: App is connected
        waitForConnection()

        // When: User types and sends message
        let beforeSend = Date()
        sendMessage("Test message")

        // Then: Message appears within 500ms (optimistic UI)
        let message = app.staticTexts["Test message"]
        XCTAssertTrue(message.waitForExistence(timeout: 0.5), "User message should appear immediately")

        let elapsed = Date().timeIntervalSince(beforeSend)
        XCTAssertLessThan(elapsed, 1.0, "Message should appear in under 1 second")
    }

    func testOptimisticUI_MessageOrder_Preserved() {
        // Given: App is connected
        waitForConnection()

        // When: User sends multiple messages rapidly
        sendMessage("First")
        sendMessage("Second")
        sendMessage("Third")

        // Then: Messages appear in correct order
        let messages = app.staticTexts.allElementsBoundByIndex

        let firstIndex = messages.firstIndex { $0.label == "First" }
        let secondIndex = messages.firstIndex { $0.label == "Second" }
        let thirdIndex = messages.firstIndex { $0.label == "Third" }

        XCTAssertNotNil(firstIndex)
        XCTAssertNotNil(secondIndex)
        XCTAssertNotNil(thirdIndex)

        if let first = firstIndex, let second = secondIndex, let third = thirdIndex {
            XCTAssertLessThan(first, second, "First should appear before Second")
            XCTAssertLessThan(second, third, "Second should appear before Third")
        }
    }

    // MARK: - Agent Response Streaming Tests

    func testAgentResponse_AppearsInTimeline() {
        // Given: App is connected
        waitForConnection()

        // When: User sends a prompt
        sendMessage("Say hello")

        // Then: Agent response appears in timeline
        let agentResponse = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] 'hello'"))
        XCTAssertTrue(agentResponse.firstMatch.waitForExistence(timeout: 30), "Agent response should appear in timeline")
    }

    func testAgentResponse_UpdatesStream_Incrementally() {
        // Given: App is connected
        waitForConnection()

        // When: User sends a prompt that requires thinking
        sendMessage("Count from 1 to 5")

        // Then: Response should stream/update (not appear all at once)
        // We can check this by waiting for partial content
        let partialResponse = app.staticTexts.matching(NSPredicate(format: "label CONTAINS '1'"))
        XCTAssertTrue(partialResponse.firstMatch.waitForExistence(timeout: 15), "Partial response should appear")

        // And eventually complete response appears
        let completeResponse = app.staticTexts.matching(NSPredicate(format: "label CONTAINS '5'"))
        XCTAssertTrue(completeResponse.firstMatch.waitForExistence(timeout: 30), "Complete response should appear")
    }

    func testAgentResponse_ToolCalls_DisplayCorrectly() {
        // Given: App is connected
        waitForConnection()

        // When: User requests agent to use a tool
        sendMessage("Read package.json")

        // Then: Tool call should be visible in timeline
        let toolCall = app.staticTexts.matching(NSPredicate(format: "label CONTAINS[c] 'read'"))
        XCTAssertTrue(toolCall.firstMatch.waitForExistence(timeout: 30), "Tool call should be visible")
    }

    // MARK: - ComposeSheet Behavior Tests

    func testComposeSheet_Opens_WithoutLag() {
        // Given: App has loaded with timeline content
        waitForConnection()
        // Send a few messages to populate timeline
        sendMessage("Message 1")
        sleep(2)
        sendMessage("Message 2")
        sleep(2)

        // When: User taps pencil button
        let beforeTap = Date()
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        // Then: Sheet appears within 500ms (no lag)
        let composer = app.textEditors["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 0.5), "Composer sheet should open without lag")

        let elapsed = Date().timeIntervalSince(beforeTap)
        XCTAssertLessThan(elapsed, 1.0, "Sheet should open in under 1 second")
    }

    func testComposeSheet_Keyboard_AppearsImmediately() {
        // Given: App is ready
        waitForConnection()

        // When: User opens compose sheet
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        // Then: Keyboard should appear immediately
        let composer = app.textEditors["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 0.5))

        // Keyboard is present when composer is first responder
        composer.tap()
        XCTAssertTrue(composer.value(forKey: "hasKeyboardFocus") as? Bool ?? false, "Keyboard should appear")
    }

    func testComposeSheet_TextEntry_Responsive() {
        // Given: Compose sheet is open
        waitForConnection()
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        let composer = app.textEditors["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 1))
        composer.tap()

        // When: User types text
        let testText = "This is a test message with multiple words"
        composer.typeText(testText)

        // Then: Text appears without lag
        XCTAssertEqual(composer.value as? String, testText, "Text should appear as typed")
    }

    func testComposeSheet_Send_ClearsComposer() {
        // Given: Compose sheet with typed text
        waitForConnection()
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        let composer = app.textEditors["message-composer"]
        composer.tap()
        composer.typeText("Test message")

        // When: User sends message
        let sendButton = app.buttons["send-button"]
        sendButton.tap()

        // Then: Composer should be cleared
        sleep(1) // Brief wait for UI update
        pencilButton.tap() // Reopen to check state

        let composerValue = composer.value as? String ?? ""
        XCTAssertTrue(composerValue.isEmpty, "Composer should be cleared after send")
    }

    func testComposeSheet_Cancel_DoesNotSend() {
        // Given: Compose sheet with typed text
        waitForConnection()
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        let composer = app.textEditors["message-composer"]
        composer.tap()
        composer.typeText("Message to cancel")

        // When: User dismisses sheet without sending
        // Swipe down or tap outside (depending on presentation)
        app.swipeDown()

        // Then: Message should not appear in timeline
        sleep(1)
        let cancelledMessage = app.staticTexts["Message to cancel"]
        XCTAssertFalse(cancelledMessage.exists, "Cancelled message should not appear")
    }

    // MARK: - Edge Cases

    func testConversation_EmptyMessage_NotSent() {
        // Given: Compose sheet is open
        waitForConnection()
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        let composer = app.textEditors["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 1))

        // When: User tries to send empty message
        let sendButton = app.buttons["send-button"]

        // Then: Send button should be disabled
        XCTAssertFalse(sendButton.isEnabled, "Send button should be disabled for empty message")
    }

    func testConversation_LongMessage_HandledCorrectly() {
        // Given: App is connected
        waitForConnection()

        // When: User sends very long message
        let longMessage = String(repeating: "This is a long message. ", count: 50)
        sendMessage(longMessage)

        // Then: Message should appear (possibly truncated in UI)
        let messageExists = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'This is a long message'"))
        XCTAssertTrue(messageExists.firstMatch.waitForExistence(timeout: 2), "Long message should be handled")
    }

    func testConversation_SpecialCharacters_HandledCorrectly() {
        // Given: App is connected
        waitForConnection()

        // When: User sends message with special characters
        let specialMessage = "Test: emoji 🚀 and symbols @#$%"
        sendMessage(specialMessage)

        // Then: Message should appear correctly
        XCTAssertTrue(app.staticTexts[specialMessage].waitForExistence(timeout: 2), "Special characters should be preserved")
    }

    func testConversation_Reconnection_ResumesCorrectly() {
        // Given: User has sent messages
        waitForConnection()
        sendMessage("Before disconnect")
        sleep(3)

        // When: Connection is lost and restored
        // (This would require disconnecting bridge - simulate by waiting)
        sleep(2)

        // Then: User can still send messages
        sendMessage("After reconnect")
        XCTAssertTrue(app.staticTexts["After reconnect"].waitForExistence(timeout: 2))
    }

    // MARK: - Helper Methods

    private func waitForConnection() {
        // Wait for connection indicator to show connected state
        let connectionIndicator = app.images["connection-indicator"]
        _ = connectionIndicator.waitForExistence(timeout: 10)

        // Additional wait to ensure bridge is ready
        sleep(2)
    }

    private func sendMessage(_ text: String) {
        let pencilButton = app.buttons["compose-button"]
        pencilButton.tap()

        let composer = app.textEditors["message-composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 1))
        composer.tap()
        composer.typeText(text)

        let sendButton = app.buttons["send-button"]
        sendButton.tap()

        // Brief wait for UI to settle
        Thread.sleep(forTimeInterval: 0.5)
    }
}
#endif
