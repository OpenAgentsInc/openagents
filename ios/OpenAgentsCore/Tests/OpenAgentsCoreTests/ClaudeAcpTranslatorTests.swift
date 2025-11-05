import XCTest
@testable import OpenAgentsCore

final class ClaudeAcpTranslatorTests: XCTestCase {

    // MARK: - User Messages

    func testTranslateUserMessage() throws {
        let lines = [
            """
            {"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/Users/test","sessionId":"test-123","version":"2.0.33","gitBranch":"main","type":"user","message":{"role":"user","content":"Hello, assistant!"},"uuid":"msg-1","timestamp":1730841812515}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.id, "test-123")
        XCTAssertEqual(thread.source, "claude-code")
        XCTAssertEqual(thread.events.count, 1)

        let event = thread.events[0]
        XCTAssertNotNil(event.message)
        XCTAssertEqual(event.message?.role, .user)
        XCTAssertEqual(event.message?.parts.count, 1)

        if case let .text(txt) = event.message?.parts[0] {
            XCTAssertEqual(txt.text, "Hello, assistant!")
        } else {
            XCTFail("Expected text content part")
        }
    }

    func testExtractTitleFromFirstUserMessage() throws {
        let lines = [
            """
            {"type":"user","message":{"role":"user","content":"This is a very long user message that should be truncated to create a reasonable title for the thread"},"uuid":"msg-1","timestamp":1730841812515,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertTrue(thread.title.contains("This is a very long user message"))
        XCTAssertTrue(thread.title.count <= 65) // Should be truncated with "..."
    }

    // MARK: - Assistant Messages

    func testTranslateAssistantTextMessage() throws {
        let lines = [
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"I can help you with that."}]},"uuid":"msg-2","timestamp":1730841815000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1)

        let event = thread.events[0]
        XCTAssertNotNil(event.message)
        XCTAssertEqual(event.message?.role, .assistant)

        if case let .text(txt) = event.message?.parts[0] {
            XCTAssertEqual(txt.text, "I can help you with that.")
        } else {
            XCTFail("Expected text content part")
        }
    }

    func testTranslateAssistantThinking() throws {
        let lines = [
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me think about this problem...","signature":"..."}]},"uuid":"msg-2","timestamp":1730841815000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1)

        let event = thread.events[0]
        XCTAssertNotNil(event.message)
        XCTAssertEqual(event.message?.role, .assistant)

        if case let .text(txt) = event.message?.parts[0] {
            XCTAssertEqual(txt.text, "Let me think about this problem...")
        } else {
            XCTFail("Expected text content part")
        }
    }

    func testTranslateMultipleContentBlocks() throws {
        let lines = [
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me analyze..."},{"type":"text","text":"Here's my response."}]},"uuid":"msg-2","timestamp":1730841815000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 2) // One for thinking, one for text
    }

    // MARK: - Tool Calls

    func testTranslateToolCall() throws {
        let lines = [
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_123","name":"Bash","input":{"command":"ls -la","description":"List files"}}]},"uuid":"msg-3","timestamp":1730841820000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1)

        let event = thread.events[0]
        XCTAssertNotNil(event.tool_call)
        XCTAssertEqual(event.tool_call?.id, "toolu_123")
        XCTAssertEqual(event.tool_call?.tool_name, "Bash")

        // Check arguments
        if case let .object(args) = event.tool_call?.arguments {
            XCTAssertEqual(args.count, 2)
            if case let .string(cmd) = args["command"] {
                XCTAssertEqual(cmd, "ls -la")
            } else {
                XCTFail("Expected command argument")
            }
        } else {
            XCTFail("Expected object arguments")
        }
    }

    // MARK: - Tool Results

    func testTranslateToolResultSuccess() throws {
        let lines = [
            """
            {"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_123","content":"total 48\\ndrwxr-xr-x  6 user  staff  192 Nov  5 10:00 .","is_error":false}]},"uuid":"msg-4","timestamp":1730841825000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1)

        let event = thread.events[0]
        XCTAssertNotNil(event.tool_result)
        XCTAssertEqual(event.tool_result?.call_id, "toolu_123")
        XCTAssertTrue(event.tool_result?.ok ?? false)
        XCTAssertNil(event.tool_result?.error)

        if case let .string(output) = event.tool_result?.result {
            XCTAssertTrue(output.contains("total 48"))
        } else {
            XCTFail("Expected string result")
        }
    }

    func testTranslateToolResultError() throws {
        let lines = [
            """
            {"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_456","content":"Error: command not found","is_error":true}]},"uuid":"msg-5","timestamp":1730841830000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1)

        let event = thread.events[0]
        XCTAssertNotNil(event.tool_result)
        XCTAssertEqual(event.tool_result?.call_id, "toolu_456")
        XCTAssertFalse(event.tool_result?.ok ?? true)
        XCTAssertEqual(event.tool_result?.error, "Error: command not found")
    }

    // MARK: - Full Conversation

    func testTranslateFullConversation() throws {
        let lines = [
            """
            {"type":"user","message":{"role":"user","content":"Run ls command"},"uuid":"msg-1","timestamp":1730841812515,"sessionId":"test-123"}
            """,
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"User wants to run ls"}]},"uuid":"msg-2","timestamp":1730841815000,"sessionId":"test-123"}
            """,
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"I'll run that for you."}]},"uuid":"msg-3","timestamp":1730841816000,"sessionId":"test-123"}
            """,
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_789","name":"Bash","input":{"command":"ls","description":"List files"}}]},"uuid":"msg-4","timestamp":1730841817000,"sessionId":"test-123"}
            """,
            """
            {"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_789","content":"file1.txt\\nfile2.txt","is_error":false}]},"uuid":"msg-5","timestamp":1730841820000,"sessionId":"test-123"}
            """,
            """
            {"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Here are the files in the directory."}]},"uuid":"msg-6","timestamp":1730841822000,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.id, "test-123")
        XCTAssertEqual(thread.source, "claude-code")
        XCTAssertEqual(thread.events.count, 6)

        // Verify sequence: user msg, thinking, assistant msg, tool call, tool result, assistant msg
        XCTAssertNotNil(thread.events[0].message)
        XCTAssertEqual(thread.events[0].message?.role, .user)

        XCTAssertNotNil(thread.events[1].message) // thinking
        XCTAssertEqual(thread.events[1].message?.role, .assistant)

        XCTAssertNotNil(thread.events[2].message)
        XCTAssertEqual(thread.events[2].message?.role, .assistant)

        XCTAssertNotNil(thread.events[3].tool_call)
        XCTAssertEqual(thread.events[3].tool_call?.tool_name, "Bash")

        XCTAssertNotNil(thread.events[4].tool_result)
        XCTAssertTrue(thread.events[4].tool_result?.ok ?? false)

        XCTAssertNotNil(thread.events[5].message)
        XCTAssertEqual(thread.events[5].message?.role, .assistant)
    }

    // MARK: - Edge Cases

    func testSkipFileHistorySnapshot() throws {
        let lines = [
            """
            {"type":"file-history-snapshot","messageId":"snap-1","snapshot":{"messageId":"snap-1","trackedFileBackups":{},"timestamp":"2025-11-05T19:43:32.520Z"},"isSnapshotUpdate":false}
            """,
            """
            {"type":"user","message":{"role":"user","content":"Hello"},"uuid":"msg-1","timestamp":1730841812515,"sessionId":"test-123"}
            """
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1) // Only user message, snapshot skipped
        XCTAssertNotNil(thread.events[0].message)
    }

    func testHandleMalformedJSON() throws {
        let lines = [
            "not valid json",
            """
            {"type":"user","message":{"role":"user","content":"Valid message"},"uuid":"msg-1","timestamp":1730841812515,"sessionId":"test-123"}
            """,
            "{incomplete json"
        ]

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertEqual(thread.events.count, 1) // Only the valid line parsed
    }

    func testEmptyLines() throws {
        let lines: [String] = []

        let thread = ClaudeAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))

        XCTAssertNotNil(thread.id)
        XCTAssertEqual(thread.events.count, 0)
    }
}
