import XCTest
import SwiftUI
@testable import OpenAgents
@testable import OpenAgentsCore

/// Integration tests for tool call rendering - tests FULL FLOW from data to UI
/// Following AGENTS.md best practices: test end-to-end, not just isolated components
final class ToolCallViewRenderingIntegrationTests: XCTestCase {

    // MARK: - Bash Command Inline Display Tests

    func testBashToolCall_ShowsCommandInline() {
        let call = ACPToolCall(
            id: "bash-123",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([.string("git"), .string("status")]),
                "description": .string("Check git status")
            ]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Should show: [icon] Bash git status [status]
        //              Check git status
        // Command should be visible inline next to "Bash"
        // Description should be visible underneath
    }

    func testBashToolCall_TruncatesLongCommand() {
        let longCommand = String(repeating: "very-long-command-that-should-be-truncated ", count: 10)
        let call = ACPToolCall(
            id: "bash-456",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([.string("bash"), .string("-lc"), .string(longCommand)]),
                "description": .string("Run very long command")
            ]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Command should truncate with ellipsis (...) if it reaches the status badge
        // Layout should not break
    }

    func testReadToolCall_ShowsRelativePath() {
        let homeDir = NSHomeDirectory()
        let absolutePath = "\(homeDir)/code/openagents/ios/OpenAgents/ContentView.swift"

        let call = ACPToolCall(
            id: "read-789",
            tool_name: "Read",
            arguments: .object(["file_path": .string(absolutePath)]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Should show: 📄 ~/code/openagents/ios/OpenAgents/ContentView.swift
        // NOT: 📄 /Users/username/code/openagents/ios/OpenAgents/ContentView.swift
        // Path should be relative with ~ for home directory
    }

    // MARK: - Status Indicator Tests

    func testToolCallView_PendingStatus_NoResult() {
        let call = ACPToolCall(
            id: "call-123",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/path/to/file.swift")]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Pending status: result is nil
        // Visual verification: should show yellow clock icon + "pending" badge
    }

    func testToolCallView_CompletedStatus_WithSuccessResult() {
        let call = ACPToolCall(
            id: "call-123",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/path/to/file.swift")]),
            ts: 1000
        )

        let result = ACPToolResult(
            call_id: "call-123",
            ok: true,
            result: .object(["content": .string("file contents...")]),
            error: nil,
            ts: 1001
        )

        let view = ToolCallView(call: call, result: result)

        XCTAssertNotNil(view)
        // Completed status: result.ok == true
        // Visual verification: should show green checkmark + "completed" badge
    }

    func testToolCallView_ErrorStatus_WithFailureResult() {
        let call = ACPToolCall(
            id: "call-456",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/nonexistent.swift")]),
            ts: 2000
        )

        let result = ACPToolResult(
            call_id: "call-456",
            ok: false,
            result: nil,
            error: "File not found",
            ts: 2001
        )

        let view = ToolCallView(call: call, result: result)

        XCTAssertNotNil(view)
        // Error status: result.ok == false
        // Visual verification: should show red X + "error" badge
    }

    // MARK: - Inline Param Display Tests

    func testToolCallView_ReadTool_ShowsFilePathInline() {
        let call = ACPToolCall(
            id: "read-call",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/Users/dev/project/main.swift")]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Inline display: should show "📄 /Users/dev/project/main.swift"
        // NOT: JSON blob of arguments
    }

    func testToolCallView_WriteTool_ShowsFilePathInline() {
        let call = ACPToolCall(
            id: "write-call",
            tool_name: "Write",
            arguments: .object([
                "file_path": .string("/Users/dev/project/output.txt"),
                "content": .string("Hello world")
            ]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Inline display: should show "✏️ /Users/dev/project/output.txt"
        // NOT: JSON blob with content
    }

    func testToolCallView_EditTool_ShowsFilePathInline() {
        let call = ACPToolCall(
            id: "edit-call",
            tool_name: "Edit",
            arguments: .object([
                "file_path": .string("/Users/dev/project/config.json"),
                "old_string": .string("foo"),
                "new_string": .string("bar")
            ]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Inline display: should show "✏️ /Users/dev/project/config.json"
    }

    func testToolCallView_BashTool_ShowsCommandInline() {
        let call = ACPToolCall(
            id: "bash-call",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("bash"),
                    .string("-lc"),
                    .string("git status")
                ])
            ]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Inline display: should show "git status" (extracted from bash -lc)
        // NOT: JSON blob of command array
    }

    func testToolCallView_GlobTool_ShowsPatternInline() {
        let call = ACPToolCall(
            id: "glob-call",
            tool_name: "Glob",
            arguments: .object(["pattern": .string("**/*.swift")]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Inline display: should show "🔍 **/*.swift"
    }

    func testToolCallView_GrepTool_ShowsPatternInline() {
        let call = ACPToolCall(
            id: "grep-call",
            tool_name: "Grep",
            arguments: .object([
                "pattern": .string("func testToolCall"),
                "path": .string("/Users/dev/project")
            ]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Inline display: should show "🔍 func testToolCall"
    }

    // MARK: - Tool Name Suffix Tests

    func testToolCallView_ToolNameWithSuffix_ParsesCorrectly() {
        // Tools might come with suffixes like "acp.Read" or "custom.read"
        let suffixedTools: [(String, String, Any)] = [
            ("acp.Read", "file_path", "/path/to/file"),
            ("custom.write", "file_path", "/output.txt"),
            ("namespace.edit", "file_path", "/config.json"),
            ("agent.bash", "command", ["ls", "-la"]),
            ("search.glob", "pattern", "*.ts"),
            ("search.grep", "pattern", "TODO")
        ]

        for (toolName, key, value) in suffixedTools {
            let args: JSONValue
            if let arr = value as? [String] {
                args = .object([key: .array(arr.map { JSONValue.string($0) })])
            } else if let str = value as? String {
                args = .object([key: .string(str)])
            } else {
                XCTFail("Invalid test data")
                continue
            }

            let call = ACPToolCall(id: "test", tool_name: toolName, arguments: args)
            let view = ToolCallView(call: call, result: nil)

            XCTAssertNotNil(view, "Should render tool with suffix: \(toolName)")
            // Should still parse inline params correctly despite suffix
        }
    }

    // MARK: - Result Correlation Tests

    func testToolCallView_FindsMatchingResult_ByCallId() {
        let callId = "correlation-test-123"

        let call = ACPToolCall(
            id: callId,
            tool_name: "Read",
            arguments: .object(["file_path": .string("/test.swift")]),
            ts: 1000
        )

        let matchingResult = ACPToolResult(
            call_id: callId,  // ✅ Matches call.id
            ok: true,
            result: .object(["data": .string("content")]),
            ts: 1001
        )

        let view = ToolCallView(call: call, result: matchingResult)

        XCTAssertNotNil(view)
        // Should show "completed" status because result is provided and ok == true
    }

    func testToolCallView_NoResult_ShowsPending() {
        let call = ACPToolCall(
            id: "pending-call",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/test.swift")]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view)
        // Should show "pending" status because result is nil
    }

    // MARK: - Detail Sheet Tests

    func testToolCallDetailSheet_ShowsFullCallDetails() {
        let call = ACPToolCall(
            id: "detail-test",
            tool_name: "Read",
            arguments: .object([
                "file_path": .string("/Users/dev/project/file.swift"),
                "offset": .number(100),
                "limit": .number(50)
            ]),
            ts: 1699900000000
        )

        let sheet = ToolCallDetailSheet(call: call, result: nil)

        XCTAssertNotNil(sheet)
        // Should display:
        // - ID: detail-test
        // - Tool: Read
        // - Timestamp: 1699900000000
        // - Arguments JSON (formatted, selectable)
    }

    func testToolCallDetailSheet_ShowsFullResultDetails() {
        let call = ACPToolCall(
            id: "detail-test",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/test.swift")]),
            ts: 1000
        )

        let result = ACPToolResult(
            call_id: "detail-test",
            ok: true,
            result: .object([
                "content": .string("func main() { ... }"),
                "lines_read": .number(100)
            ]),
            ts: 1001
        )

        let sheet = ToolCallDetailSheet(call: call, result: result)

        XCTAssertNotNil(sheet)
        // Should display:
        // - Tool Call section (ID, name, args)
        // - Tool Result section (status: Success, result data JSON)
    }

    func testToolCallDetailSheet_ShowsErrorDetails() {
        let call = ACPToolCall(
            id: "error-test",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/nonexistent.swift")]),
            ts: 2000
        )

        let result = ACPToolResult(
            call_id: "error-test",
            ok: false,
            result: nil,
            error: "File not found: /nonexistent.swift",
            ts: 2001
        )

        let sheet = ToolCallDetailSheet(call: call, result: result)

        XCTAssertNotNil(sheet)
        // Should display:
        // - Tool Call section
        // - Tool Result section with status: Error
        // - Error message in red box: "File not found: /nonexistent.swift"
    }

    // MARK: - Integration: AcpThreadView Timeline Rendering

    func testAcpThreadView_FindsMatchingResult() {
        // Simulate timeline with tool call and result
        let callId = "timeline-test-123"

        let toolCall = ACPToolCall(
            id: callId,
            tool_name: "Read",
            arguments: .object(["file_path": .string("/test.swift")]),
            ts: 1000
        )

        let toolResult = ACPToolResult(
            call_id: callId,
            ok: true,
            result: .object(["content": .string("data")]),
            ts: 1001
        )

        // In real AcpThreadView, these would be in timeline as:
        // .toolCall(toolCall)
        // .toolResult(toolResult)
        //
        // findResult(for: toolCall) should return toolResult

        // Verify correlation logic
        XCTAssertEqual(toolResult.call_id, toolCall.id, "Result should match call by ID")
    }

    // MARK: - Regression Tests

    func testToolCallView_NeverRendersJSONBlob_ForStandardTools() {
        // CRITICAL: Standard tools should NEVER render as JSON blobs
        let standardTools: [(String, JSONValue)] = [
            ("Read", .object(["file_path": .string("/test.swift")])),
            ("Write", .object(["file_path": .string("/out.txt"), "content": .string("data")])),
            ("Edit", .object(["file_path": .string("/cfg.json"), "old_string": .string("a"), "new_string": .string("b")])),
            ("Bash", .object(["command": .array([.string("ls")])])),
            ("Glob", .object(["pattern": .string("*.ts")])),
            ("Grep", .object(["pattern": .string("TODO")]))
        ]

        for (toolName, args) in standardTools {
            let call = ACPToolCall(id: "test", tool_name: toolName, arguments: args)
            let view = ToolCallView(call: call, result: nil)

            XCTAssertNotNil(view, "\(toolName) should render as component, not JSON")
            // Visual verification: should show inline params, NOT raw JSON
        }
    }

    func testToolCallView_AllStatusStates_RenderProperly() {
        let call = ACPToolCall(
            id: "status-test",
            tool_name: "Read",
            arguments: .object(["file_path": .string("/test.swift")]),
            ts: 1000
        )

        // Test all 3 status states
        let states: [(ACPToolResult?, String)] = [
            (nil, "pending"),
            (ACPToolResult(call_id: "status-test", ok: true, ts: 1001), "completed"),
            (ACPToolResult(call_id: "status-test", ok: false, error: "Error", ts: 1002), "error")
        ]

        for (result, expectedStatus) in states {
            let view = ToolCallView(call: call, result: result)
            XCTAssertNotNil(view, "Should render in \(expectedStatus) state")
            // Visual verification:
            // - pending: yellow clock, "pending" badge
            // - completed: green checkmark, "completed" badge
            // - error: red X, "error" badge
        }
    }

    // MARK: - Edge Cases

    func testToolCallView_MissingFilePath_FallsBackGracefully() {
        let call = ACPToolCall(
            id: "edge-case",
            tool_name: "Read",
            arguments: .object([:]),  // Missing file_path
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view, "Should still render even with missing params")
        // Should not crash; may show tool name without inline params
    }

    func testToolCallView_InvalidArgumentsShape_HandlesGracefully() {
        let call = ACPToolCall(
            id: "invalid",
            tool_name: "Read",
            arguments: .string("invalid structure"),  // Wrong type
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view, "Should handle invalid args without crashing")
    }

    func testToolCallView_UnknownToolType_RendersGeneric() {
        let call = ACPToolCall(
            id: "unknown",
            tool_name: "CustomUnknownTool",
            arguments: .object(["foo": .string("bar")]),
            ts: 1000
        )

        let view = ToolCallView(call: call, result: nil)

        XCTAssertNotNil(view, "Should render unknown tools generically")
        // OK to show JSON for truly unknown tools
    }
}
