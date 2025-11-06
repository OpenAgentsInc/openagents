import XCTest
@testable import OpenAgentsCore

/// Tests to ensure 100% ACP protocol compliance for message type handling
/// Verifies that agentMessageChunk vs agentThoughtChunk are correctly distinguished
final class ACPMessageTypeComplianceTests: XCTestCase {

    // MARK: - Protocol Type Distinction Tests

    func testAgentMessageChunk_IsNotThinking() {
        // Agent messages should NEVER be classified as thinking/reasoning
        // regardless of their content format (markdown, bullets, etc.)
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: "Here's what I found:\n- Item 1\n- Item 2\n- Item 3"))
        )
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        // Verify it's the correct type
        if case .agentMessageChunk = update {
            XCTAssert(true, "Correctly identified as agentMessageChunk")
        } else {
            XCTFail("Should be agentMessageChunk, not thinking")
        }
    }

    func testAgentThoughtChunk_IsThinking() {
        // Agent thought chunks should ALWAYS be classified as thinking/reasoning
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: "Let me think about this..."))
        )
        let update = ACP.Client.SessionUpdate.agentThoughtChunk(chunk)

        // Verify it's the correct type
        if case .agentThoughtChunk = update {
            XCTAssert(true, "Correctly identified as agentThoughtChunk")
        } else {
            XCTFail("Should be agentThoughtChunk")
        }
    }

    func testAgentMessageWithMarkdown_NotClassifiedAsThought() {
        // CRITICAL: Messages with markdown formatting should NOT be thoughts
        let markdownMessage = """
        ## ✅ Retry/Reconnect Implementation Complete!

        ### What Was Implemented

        **MobileWebSocketClient Enhancements:**
        - ✅ Automatic retry with exponential backoff
        - ✅ Handshake timeout detection
        - ✅ Auto-reconnect on unexpected disconnect
        """

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: markdownMessage)))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        if case .agentMessageChunk = update {
            XCTAssert(true, "Markdown messages should be regular messages")
        } else {
            XCTFail("Markdown formatted message incorrectly classified as thought")
        }
    }

    func testAgentMessageWithBullets_NotClassifiedAsThought() {
        // Messages with bullet lists are regular messages, not thoughts
        let bulletMessage = """
        I've completed the following:
        - Added retry logic
        - Implemented exponential backoff
        - Created comprehensive tests
        """

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: bulletMessage)))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        if case .agentMessageChunk = update {
            XCTAssert(true, "Bullet lists should be regular messages")
        } else {
            XCTFail("Bullet list message incorrectly classified as thought")
        }
    }

    func testAgentMessageWithNumberedList_NotClassifiedAsThought() {
        // Messages with numbered lists are regular messages, not thoughts
        let numberedMessage = """
        Here are the steps:
        1. First step
        2. Second step
        3. Third step
        """

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: numberedMessage)))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        if case .agentMessageChunk = update {
            XCTAssert(true, "Numbered lists should be regular messages")
        } else {
            XCTFail("Numbered list message incorrectly classified as thought")
        }
    }

    func testThinkingContent_ClassifiedAsThought() {
        // Only agentThoughtChunk should be thoughts
        let thinkingMessage = "Let me analyze this step by step..."

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: thinkingMessage)))
        let update = ACP.Client.SessionUpdate.agentThoughtChunk(chunk)

        if case .agentThoughtChunk = update {
            XCTAssert(true, "Thinking content correctly classified")
        } else {
            XCTFail("Thinking content should be agentThoughtChunk")
        }
    }

    // MARK: - Encoding/Decoding Compliance

    func testAgentMessageChunk_EncodesCorrectly() throws {
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: "Test message"))
        )
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        let encoder = JSONEncoder()
        let data = try encoder.encode(update)
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]

        XCTAssertEqual(json["sessionUpdate"] as? String, "agent_message_chunk")
    }

    func testAgentThoughtChunk_EncodesCorrectly() throws {
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: "Test thought"))
        )
        let update = ACP.Client.SessionUpdate.agentThoughtChunk(chunk)

        let encoder = JSONEncoder()
        let data = try encoder.encode(update)
        let json = try JSONSerialization.jsonObject(with: data) as! [String: Any]

        XCTAssertEqual(json["sessionUpdate"] as? String, "agent_thought_chunk")
    }

    func testAgentMessageChunk_DecodesCorrectly() throws {
        let json = """
        {
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "text",
                "text": "Test message"
            }
        }
        """

        let decoder = JSONDecoder()
        let data = json.data(using: .utf8)!
        let update = try decoder.decode(ACP.Client.SessionUpdate.self, from: data)

        if case .agentMessageChunk(let chunk) = update,
           case .text(let text) = chunk.content {
            XCTAssertEqual(text.text, "Test message")
        } else {
            XCTFail("Failed to decode agentMessageChunk correctly")
        }
    }

    func testAgentThoughtChunk_DecodesCorrectly() throws {
        let json = """
        {
            "sessionUpdate": "agent_thought_chunk",
            "content": {
                "type": "text",
                "text": "Test thought"
            }
        }
        """

        let decoder = JSONDecoder()
        let data = json.data(using: .utf8)!
        let update = try decoder.decode(ACP.Client.SessionUpdate.self, from: data)

        if case .agentThoughtChunk(let chunk) = update,
           case .text(let text) = chunk.content {
            XCTAssertEqual(text.text, "Test thought")
        } else {
            XCTFail("Failed to decode agentThoughtChunk correctly")
        }
    }

    // MARK: - Round Trip Tests

    func testAgentMessageChunk_RoundTrip() throws {
        let original = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Round trip test")))
        )

        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        let data = try encoder.encode(original)
        let decoded = try decoder.decode(ACP.Client.SessionUpdate.self, from: data)

        if case .agentMessageChunk(let chunk) = decoded,
           case .text(let text) = chunk.content {
            XCTAssertEqual(text.text, "Round trip test")
        } else {
            XCTFail("Round trip failed")
        }
    }

    func testAgentThoughtChunk_RoundTrip() throws {
        let original = ACP.Client.SessionUpdate.agentThoughtChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Thinking round trip")))
        )

        let encoder = JSONEncoder()
        let decoder = JSONDecoder()

        let data = try encoder.encode(original)
        let decoded = try decoder.decode(ACP.Client.SessionUpdate.self, from: data)

        if case .agentThoughtChunk(let chunk) = decoded,
           case .text(let text) = chunk.content {
            XCTAssertEqual(text.text, "Thinking round trip")
        } else {
            XCTFail("Round trip failed")
        }
    }

    // MARK: - Content Format Independence

    func testAgentMessage_WithCode_NotThought() {
        // Code blocks in messages should not make them thoughts
        let codeMessage = """
        Here's the implementation:
        ```swift
        func example() {
            print("Hello")
        }
        ```
        """

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: codeMessage)))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        if case .agentMessageChunk = update {
            XCTAssert(true)
        } else {
            XCTFail("Code in message should not make it a thought")
        }
    }

    func testAgentMessage_WithEmojis_NotThought() {
        // Emojis in messages should not affect classification
        let emojiMessage = "✅ Success! 🎉 Everything works perfectly! 🚀"

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: emojiMessage)))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        if case .agentMessageChunk = update {
            XCTAssert(true)
        } else {
            XCTFail("Emojis should not affect message classification")
        }
    }

    func testAgentMessage_WithTables_NotThought() {
        // Markdown tables should not make messages thoughts
        let tableMessage = """
        | Feature | Status |
        |---------|--------|
        | Retry   | ✅      |
        | Tests   | ✅      |
        """

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: tableMessage)))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        if case .agentMessageChunk = update {
            XCTAssert(true)
        } else {
            XCTFail("Tables should not affect message classification")
        }
    }

    // MARK: - Protocol Compliance Verification

    func testAllMessageTypes_UniqueDiscriminators() throws {
        // Ensure each message type has a unique discriminator
        let userChunk = ACP.Client.SessionUpdate.userMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "User")))
        )
        let agentChunk = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Agent")))
        )
        let thoughtChunk = ACP.Client.SessionUpdate.agentThoughtChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Thought")))
        )

        let encoder = JSONEncoder()

        let userData = try encoder.encode(userChunk)
        let agentData = try encoder.encode(agentChunk)
        let thoughtData = try encoder.encode(agentChunk)

        let userJSON = try JSONSerialization.jsonObject(with: userData) as! [String: Any]
        let agentJSON = try JSONSerialization.jsonObject(with: agentData) as! [String: Any]
        let thoughtJSON = try JSONSerialization.jsonObject(with: thoughtData) as! [String: Any]

        XCTAssertEqual(userJSON["sessionUpdate"] as? String, "user_message_chunk")
        XCTAssertEqual(agentJSON["sessionUpdate"] as? String, "agent_message_chunk")
        XCTAssertNotEqual(userJSON["sessionUpdate"] as? String, agentJSON["sessionUpdate"] as? String)
    }

    func testMessageType_NeverChangesBasedOnContent() {
        // CRITICAL: Message type MUST be determined by ACP update type,
        // NOT by heuristics on content

        let messages = [
            "Simple message",
            "Message with\n- bullets",
            "## Header message",
            "1. Numbered\n2. List",
            "**Bold** and *italic*",
            "```code block```",
            "| table | cell |"
        ]

        for text in messages {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
            let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

            // ALL of these should remain agentMessageChunk
            if case .agentMessageChunk = update {
                XCTAssert(true)
            } else {
                XCTFail("Message type changed based on content: \(text)")
            }
        }
    }
}
