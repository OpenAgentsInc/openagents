import XCTest
@testable import OpenAgentsCore

final class ShellCommandFormatterTests: XCTestCase {

    // MARK: - format() tests

    func testFormat_bashLoginCommand() {
        // bash -lc "ls -la"
        let call = ACPToolCall(
            id: "test1",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("bash"),
                    .string("-lc"),
                    .string("ls -la")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "ls -la", "Should extract command from bash -lc wrapper")
    }

    func testFormat_genericCommand() {
        // Simple command array without bash -lc
        let call = ACPToolCall(
            id: "test2",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("git"),
                    .string("status")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "git status", "Should join command parts")
    }

    func testFormat_commandWithWhitespace() {
        // Command with arguments containing whitespace
        let call = ACPToolCall(
            id: "test3",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("echo"),
                    .string("hello world")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "echo \"hello world\"", "Should quote arguments with whitespace")
    }

    func testFormat_nonShellTool() {
        // Non-shell tool should return nil
        let call = ACPToolCall(
            id: "test4",
            tool_name: "Read",
            arguments: .object([
                "file_path": .string("/path/to/file")
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertNil(result, "Should return nil for non-shell tools")
    }

    func testFormat_shellVariant() {
        // Test with "Shell" tool name
        let call = ACPToolCall(
            id: "test5",
            tool_name: "Shell",
            arguments: .object([
                "command": .array([
                    .string("pwd")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "pwd", "Should handle 'Shell' tool name")
    }

    func testFormat_customShellSuffix() {
        // Test with custom.shell naming
        let call = ACPToolCall(
            id: "test6",
            tool_name: "custom.shell",
            arguments: .object([
                "command": .array([
                    .string("ls")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "ls", "Should handle .shell suffix")
    }

    // MARK: - parseCommandArray() tests

    func testParseCommandArray_directArray() {
        let args = JSONValue.object([
            "command": .array([
                .string("git"),
                .string("status")
            ])
        ])

        let result = ShellCommandFormatter.parseCommandArray(from: args)
        XCTAssertEqual(result, ["git", "status"], "Should parse direct array")
    }

    func testParseCommandArray_withNumbers() {
        let args = JSONValue.object([
            "command": .array([
                .string("timeout"),
                .number(30),
                .string("npm"),
                .string("test")
            ])
        ])

        let result = ShellCommandFormatter.parseCommandArray(from: args)
        XCTAssertEqual(result, ["timeout", "30.0", "npm", "test"], "Should convert numbers to strings")
    }

    func testParseCommandArray_withBooleans() {
        let args = JSONValue.object([
            "command": .array([
                .string("test"),
                .bool(true),
                .string("and"),
                .bool(false)
            ])
        ])

        let result = ShellCommandFormatter.parseCommandArray(from: args)
        XCTAssertEqual(result, ["test", "true", "and", "false"], "Should convert booleans to strings")
    }

    func testParseCommandArray_nestedJSONString() {
        // Arguments wrapped in JSON string (common pattern)
        let jsonStr = """
        {
            "command": ["bash", "-lc", "ls -la"]
        }
        """

        let args = JSONValue.object([
            "arguments": .string(jsonStr)
        ])

        let result = ShellCommandFormatter.parseCommandArray(from: args)
        XCTAssertEqual(result, ["bash", "-lc", "ls -la"], "Should parse nested JSON string")
    }

    func testParseCommandArray_topLevelJSONString() {
        // Top-level JSON string
        let jsonStr = """
        {
            "command": ["git", "push", "origin", "main"]
        }
        """

        let result = ShellCommandFormatter.parseCommandArray(from: .string(jsonStr))
        XCTAssertEqual(result, ["git", "push", "origin", "main"], "Should parse top-level JSON string")
    }

    func testParseCommandArray_missingCommand() {
        let args = JSONValue.object([
            "file_path": .string("/path/to/file")
        ])

        let result = ShellCommandFormatter.parseCommandArray(from: args)
        XCTAssertNil(result, "Should return nil when command is missing")
    }

    func testParseCommandArray_emptyArray() {
        let args = JSONValue.object([
            "command": .array([])
        ])

        let result = ShellCommandFormatter.parseCommandArray(from: args)
        XCTAssertEqual(result, [], "Should handle empty array")
    }

    // MARK: - Integration tests

    func testFormat_realWorldBashLoginCommand() {
        // Real example from Claude Code
        let call = ACPToolCall(
            id: "call_123",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("bash"),
                    .string("-lc"),
                    .string("cd /Users/test && xcodebuild -workspace App.xcworkspace -scheme App build")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(
            result,
            "cd /Users/test && xcodebuild -workspace App.xcworkspace -scheme App build",
            "Should extract complex command from bash -lc"
        )
    }

    func testFormat_gitCommand() {
        let call = ACPToolCall(
            id: "call_456",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("git"),
                    .string("commit"),
                    .string("-m"),
                    .string("Add feature")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "git commit -m \"Add feature\"", "Should quote commit message")
    }

    func testFormat_commandWithTab() {
        let call = ACPToolCall(
            id: "call_789",
            tool_name: "Bash",
            arguments: .object([
                "command": .array([
                    .string("echo"),
                    .string("hello\tworld")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "echo \"hello\tworld\"", "Should quote arguments with tabs")
    }

    func testFormat_caseInsensitive() {
        // Test that tool name matching is case-insensitive
        let call = ACPToolCall(
            id: "call_999",
            tool_name: "BASH",
            arguments: .object([
                "command": .array([
                    .string("pwd")
                ])
            ])
        )

        let result = ShellCommandFormatter.format(call: call)
        XCTAssertEqual(result, "pwd", "Should handle uppercase tool names")
    }
}
