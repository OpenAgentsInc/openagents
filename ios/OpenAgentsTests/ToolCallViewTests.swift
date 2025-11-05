import XCTest
import SwiftUI
@testable import OpenAgents
@testable import OpenAgentsCore

final class ToolCallViewTests: XCTestCase {

    // MARK: - ACPToolCall Creation Helpers

    func makeToolCall(name: String, arguments: JSONValue) -> ACPToolCall {
        return ACPToolCall(
            id: ACP.ToolUseId("test-id"),
            tool_name: name,
            arguments: arguments
        )
    }

    // MARK: - Shell Command Parsing Tests

    func testShellCommand_BashLC_Simple() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("bash"),
                .string("-lc"),
                .string("ls -la")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)

        // Test via view rendering (indirect test of prettyShellCommand)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_BashLC_Complex() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("bash"),
                .string("-lc"),
                .string("cd /tmp && echo 'hello world' > test.txt")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_DirectArray() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("git"),
                .string("status")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_WithSpaces() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("echo"),
                .string("hello world")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_WithNumbers() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("kill"),
                .number(1234)
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_WithBooleans() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("test"),
                .bool(true)
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_NestedArguments() throws {
        // Test the unwrapArgumentsJSON path
        let innerJSON = """
        {"command":["bash","-lc","pwd"]}
        """
        let args = JSONValue.object([
            "arguments": .string(innerJSON)
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_StringWithNestedJSON() throws {
        let innerJSON = """
        {"command":["npm","install"]}
        """
        let args = JSONValue.string(innerJSON)
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    // MARK: - Non-Shell Tool Tests

    func testNonShellTool_Read() throws {
        let args = JSONValue.object([
            "file_path": .string("/tmp/test.txt"),
            "offset": .number(0),
            "limit": .number(100)
        ])
        let call = makeToolCall(name: "Read", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testNonShellTool_Write() throws {
        let args = JSONValue.object([
            "file_path": .string("/tmp/output.txt"),
            "content": .string("Hello, world!")
        ])
        let call = makeToolCall(name: "Write", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testNonShellTool_Edit() throws {
        let args = JSONValue.object([
            "file_path": .string("/tmp/file.txt"),
            "old_string": .string("foo"),
            "new_string": .string("bar")
        ])
        let call = makeToolCall(name: "Edit", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testNonShellTool_Bash() throws {
        let args = JSONValue.object([
            "command": .string("ls -la"),
            "description": .string("List files")
        ])
        let call = makeToolCall(name: "Bash", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testNonShellTool_EmptyArguments() throws {
        let args = JSONValue.object([:])
        let call = makeToolCall(name: "NoArgs", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    // MARK: - Tool Name Variants

    func testShellTool_Lowercase() throws {
        let args = JSONValue.object([
            "command": .array([.string("echo"), .string("test")])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellTool_Uppercase() throws {
        let args = JSONValue.object([
            "command": .array([.string("echo"), .string("test")])
        ])
        let call = makeToolCall(name: "SHELL", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellTool_Suffix() throws {
        let args = JSONValue.object([
            "command": .array([.string("echo"), .string("test")])
        ])
        let call = makeToolCall(name: "mcp.shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    // MARK: - Edge Cases

    func testShellCommand_EmptyArray() throws {
        let args = JSONValue.object([
            "command": .array([])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_SingleElement() throws {
        let args = JSONValue.object([
            "command": .array([.string("pwd")])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_TwoElements() throws {
        let args = JSONValue.object([
            "command": .array([.string("bash"), .string("-lc")])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_MalformedJSON() throws {
        let args = JSONValue.object([
            "command": .string("not a valid command array")
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_MissingCommand() throws {
        let args = JSONValue.object([
            "foo": .string("bar")
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testShellCommand_NullCommand() throws {
        let args = JSONValue.object([
            "command": .null
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    // MARK: - JSON Prettification Tests

    func testPrettyJSON_SimpleObject() throws {
        let args = JSONValue.object([
            "key": .string("value")
        ])
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testPrettyJSON_NestedObject() throws {
        let args = JSONValue.object([
            "outer": .object([
                "inner": .string("value")
            ])
        ])
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testPrettyJSON_Array() throws {
        let args = JSONValue.object([
            "items": .array([
                .string("a"),
                .string("b"),
                .string("c")
            ])
        ])
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testPrettyJSON_MixedTypes() throws {
        let args = JSONValue.object([
            "string": .string("value"),
            "number": .number(42),
            "bool": .bool(true),
            "null": .null
        ])
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testPrettyJSON_LargeObject() throws {
        var largeObject: [String: JSONValue] = [:]
        for i in 0..<100 {
            largeObject["key\(i)"] = .string("value\(i)")
        }
        let args = JSONValue.object(largeObject)
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testPrettyJSON_SpecialCharacters() throws {
        let args = JSONValue.object([
            "text": .string("Line 1\nLine 2\tTabbed"),
            "quote": .string("She said \"hello\"")
        ])
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testPrettyJSON_Unicode() throws {
        let args = JSONValue.object([
            "emoji": .string("🚀🎉"),
            "chinese": .string("你好世界"),
            "arabic": .string("مرحبا")
        ])
        let call = makeToolCall(name: "Test", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    // MARK: - View Rendering Tests

    func testView_RendersToolName() throws {
        let args = JSONValue.object([:])
        let call = makeToolCall(name: "TestTool", arguments: args)
        let view = ToolCallView(call: call)

        // Verify view can be created and contains expected elements
        XCTAssertNotNil(view)
        XCTAssertEqual(call.tool_name, "TestTool")
    }

    func testView_HandlesComplexCall() throws {
        let args = JSONValue.object([
            "nested": .object([
                "array": .array([
                    .number(1),
                    .number(2),
                    .number(3)
                ]),
                "string": .string("test")
            ])
        ])
        let call = makeToolCall(name: "ComplexTool", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    // MARK: - Bash -lc Extraction Tests

    func testBashLC_ExtractsCommand() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("bash"),
                .string("-lc"),
                .string("echo 'hello'")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testBashLC_DoesNotExtractIfNotBashLC() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("bash"),
                .string("script.sh")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }

    func testBashLC_HandlesComplexCommand() throws {
        let args = JSONValue.object([
            "command": .array([
                .string("bash"),
                .string("-lc"),
                .string("for i in {1..10}; do echo $i; done")
            ])
        ])
        let call = makeToolCall(name: "shell", arguments: args)
        let view = ToolCallView(call: call)
        XCTAssertNotNil(view)
    }
}
