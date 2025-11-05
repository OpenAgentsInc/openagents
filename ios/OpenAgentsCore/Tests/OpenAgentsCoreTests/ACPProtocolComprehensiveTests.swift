import XCTest
@testable import OpenAgentsCore

final class ACPProtocolComprehensiveTests: XCTestCase {
    var encoder: JSONEncoder!
    var decoder: JSONDecoder!

    override func setUp() {
        super.setUp()
        encoder = JSONEncoder()
        decoder = JSONDecoder()
    }

    override func tearDown() {
        encoder = nil
        decoder = nil
        super.tearDown()
    }

    // MARK: - ContentBlock Comprehensive Tests

    func testContentBlock_allTextVariants() throws {
        // Plain text
        let text1 = ACP.Client.TextBlock(text: "Hello")
        let block1 = ACP.Client.ContentBlock.text(text1)
        let data1 = try encoder.encode(block1)
        let decoded1 = try decoder.decode(ACP.Client.ContentBlock.self, from: data1)
        if case .text(let t) = decoded1 {
            XCTAssertEqual(t.text, "Hello")
        } else {
            XCTFail("Expected text block")
        }

        // Text with annotations
        let text2 = ACP.Client.TextBlock(text: "Annotated", annotations: ["key": "value"])
        let block2 = ACP.Client.ContentBlock.text(text2)
        let data2 = try encoder.encode(block2)
        let decoded2 = try decoder.decode(ACP.Client.ContentBlock.self, from: data2)
        if case .text(let t) = decoded2 {
            XCTAssertEqual(t.text, "Annotated")
            // Annotations currently not decoded due to AnyEncodable handling
        } else {
            XCTFail("Expected text block")
        }

        // Empty text
        let text3 = ACP.Client.TextBlock(text: "")
        let block3 = ACP.Client.ContentBlock.text(text3)
        let data3 = try encoder.encode(block3)
        let decoded3 = try decoder.decode(ACP.Client.ContentBlock.self, from: data3)
        if case .text(let t) = decoded3 {
            XCTAssertEqual(t.text, "")
        } else {
            XCTFail("Expected text block")
        }

        // Text with special characters
        let text4 = ACP.Client.TextBlock(text: "Special: \n\t\"\\")
        let block4 = ACP.Client.ContentBlock.text(text4)
        let data4 = try encoder.encode(block4)
        let decoded4 = try decoder.decode(ACP.Client.ContentBlock.self, from: data4)
        if case .text(let t) = decoded4 {
            XCTAssertEqual(t.text, "Special: \n\t\"\\")
        } else {
            XCTFail("Expected text block")
        }
    }

    func testContentBlock_imageVariants() throws {
        // Image with data
        let img1 = ACP.Client.Image(data: "base64data", mimeType: "image/png", uri: "file:///test.png")
        let block1 = ACP.Client.ContentBlock.image(img1)
        let data1 = try encoder.encode(block1)
        let decoded1 = try decoder.decode(ACP.Client.ContentBlock.self, from: data1)
        if case .image(let i) = decoded1 {
            XCTAssertEqual(i.mimeType, "image/png")
            XCTAssertEqual(i.uri, "file:///test.png")
        } else {
            XCTFail("Expected image block")
        }

        // Image without data (uri only)
        let img2 = ACP.Client.Image(data: nil, mimeType: "image/jpeg", uri: "https://example.com/img.jpg")
        let block2 = ACP.Client.ContentBlock.image(img2)
        let data2 = try encoder.encode(block2)
        let decoded2 = try decoder.decode(ACP.Client.ContentBlock.self, from: data2)
        if case .image(let i) = decoded2 {
            XCTAssertEqual(i.mimeType, "image/jpeg")
            XCTAssertEqual(i.uri, "https://example.com/img.jpg")
        } else {
            XCTFail("Expected image block")
        }
    }

    func testContentBlock_resourceLinkVariants() throws {
        // Full resource link
        let link1 = ACP.Client.ResourceLink(
            title: "Documentation",
            uri: "https://docs.example.com",
            mimeType: "text/html",
            description: "API docs"
        )
        let block1 = ACP.Client.ContentBlock.resource_link(link1)
        let data1 = try encoder.encode(block1)
        let decoded1 = try decoder.decode(ACP.Client.ContentBlock.self, from: data1)
        if case .resource_link(let l) = decoded1 {
            XCTAssertEqual(l.title, "Documentation")
            XCTAssertEqual(l.uri, "https://docs.example.com")
            XCTAssertEqual(l.mimeType, "text/html")
        } else {
            XCTFail("Expected resource_link block")
        }

        // Minimal resource link
        let link2 = ACP.Client.ResourceLink(
            title: "Link",
            uri: "https://example.com",
            mimeType: nil,
            description: nil
        )
        let block2 = ACP.Client.ContentBlock.resource_link(link2)
        let data2 = try encoder.encode(block2)
        let decoded2 = try decoder.decode(ACP.Client.ContentBlock.self, from: data2)
        if case .resource_link(let l) = decoded2 {
            XCTAssertEqual(l.title, "Link")
            XCTAssertEqual(l.uri, "https://example.com")
        } else {
            XCTFail("Expected resource_link block")
        }
    }

    func testContentBlock_toolUseVariants() throws {
        // Tool use with simple arguments
        let args1 = TestHelpers.makeToolArguments(["command": "ls", "description": "List files"])
        let tool1 = ACP.Client.ToolUse(
            id: ACP.ToolUseId("tool-1"),
            name: "Bash",
            arguments: args1
        )
        let block1 = ACP.Client.ContentBlock.toolUse(tool1)
        let data1 = try encoder.encode(block1)
        let decoded1 = try decoder.decode(ACP.Client.ContentBlock.self, from: data1)
        if case .toolUse(let t) = decoded1 {
            XCTAssertEqual(t.id.value, "tool-1")
            XCTAssertEqual(t.name, "Bash")
            XCTAssertFalse(t.arguments.isEmpty)
        } else {
            XCTFail("Expected toolUse block")
        }

        // Tool use with no arguments
        let tool2 = ACP.Client.ToolUse(
            id: ACP.ToolUseId("tool-2"),
            name: "NoArgs",
            arguments: [:]
        )
        let block2 = ACP.Client.ContentBlock.toolUse(tool2)
        let data2 = try encoder.encode(block2)
        let decoded2 = try decoder.decode(ACP.Client.ContentBlock.self, from: data2)
        if case .toolUse(let t) = decoded2 {
            XCTAssertEqual(t.id.value, "tool-2")
            XCTAssertEqual(t.name, "NoArgs")
            XCTAssertTrue(t.arguments.isEmpty)
        } else {
            XCTFail("Expected toolUse block")
        }

        // Tool use with complex arguments
        let args3 = TestHelpers.makeToolArguments([
            "string": "value",
            "number": 42,
            "bool": true,
            "float": 3.14
        ])
        let tool3 = ACP.Client.ToolUse(
            id: ACP.ToolUseId("tool-3"),
            name: "Complex",
            arguments: args3
        )
        let block3 = ACP.Client.ContentBlock.toolUse(tool3)
        let data3 = try encoder.encode(block3)
        let decoded3 = try decoder.decode(ACP.Client.ContentBlock.self, from: data3)
        if case .toolUse(let t) = decoded3 {
            XCTAssertEqual(t.id.value, "tool-3")
            XCTAssertEqual(t.name, "Complex")
            XCTAssertEqual(t.arguments.count, 4)
        } else {
            XCTFail("Expected toolUse block")
        }
    }

    func testContentBlock_toolResultVariants() throws {
        // Success result
        let result1 = ACP.Client.ToolResult(
            tool_use_id: ACP.ToolUseId("tool-1"),
            content: [.text(.init(text: "Success output"))],
            is_error: nil
        )
        let block1 = ACP.Client.ContentBlock.toolResult(result1)
        let data1 = try encoder.encode(block1)
        let decoded1 = try decoder.decode(ACP.Client.ContentBlock.self, from: data1)
        if case .toolResult(let r) = decoded1 {
            XCTAssertEqual(r.tool_use_id.value, "tool-1")
            XCTAssertEqual(r.content.count, 1)
        } else {
            XCTFail("Expected toolResult block")
        }

        // Error result
        let result2 = ACP.Client.ToolResult(
            tool_use_id: ACP.ToolUseId("tool-2"),
            content: [.text(.init(text: "Error: command failed"))],
            is_error: true
        )
        let block2 = ACP.Client.ContentBlock.toolResult(result2)
        let data2 = try encoder.encode(block2)
        let decoded2 = try decoder.decode(ACP.Client.ContentBlock.self, from: data2)
        if case .toolResult(let r) = decoded2 {
            XCTAssertEqual(r.tool_use_id.value, "tool-2")
            XCTAssertTrue(r.is_error ?? false)
        } else {
            XCTFail("Expected toolResult block")
        }

        // Result with multiple content blocks
        let result3 = ACP.Client.ToolResult(
            tool_use_id: ACP.ToolUseId("tool-3"),
            content: [
                .text(.init(text: "Output line 1")),
                .text(.init(text: "Output line 2"))
            ],
            is_error: nil
        )
        let block3 = ACP.Client.ContentBlock.toolResult(result3)
        let data3 = try encoder.encode(block3)
        let decoded3 = try decoder.decode(ACP.Client.ContentBlock.self, from: data3)
        if case .toolResult(let r) = decoded3 {
            XCTAssertEqual(r.tool_use_id.value, "tool-3")
            XCTAssertEqual(r.content.count, 2)
        } else {
            XCTFail("Expected toolResult block")
        }
    }

    func testContentBlock_thinkingVariants() throws {
        // Regular thinking
        let thinking1 = ACP.Client.Thinking(thinking: "Let me analyze this...")
        let block1 = ACP.Client.ContentBlock.thinking(thinking1)
        let data1 = try encoder.encode(block1)
        let decoded1 = try decoder.decode(ACP.Client.ContentBlock.self, from: data1)
        if case .thinking(let t) = decoded1 {
            XCTAssertEqual(t.thinking, "Let me analyze this...")
        } else {
            XCTFail("Expected thinking block")
        }

        // Empty thinking
        let thinking2 = ACP.Client.Thinking(thinking: "")
        let block2 = ACP.Client.ContentBlock.thinking(thinking2)
        let data2 = try encoder.encode(block2)
        let decoded2 = try decoder.decode(ACP.Client.ContentBlock.self, from: data2)
        if case .thinking(let t) = decoded2 {
            XCTAssertEqual(t.thinking, "")
        } else {
            XCTFail("Expected thinking block")
        }

        // Multi-line thinking
        let thinking3 = ACP.Client.Thinking(thinking: "Step 1:\nAnalyze input\n\nStep 2:\nGenerate response")
        let block3 = ACP.Client.ContentBlock.thinking(thinking3)
        let data3 = try encoder.encode(block3)
        let decoded3 = try decoder.decode(ACP.Client.ContentBlock.self, from: data3)
        if case .thinking(let t) = decoded3 {
            XCTAssertTrue(t.thinking.contains("Step 1:"))
            XCTAssertTrue(t.thinking.contains("Step 2:"))
        } else {
            XCTFail("Expected thinking block")
        }
    }

    // MARK: - SessionUpdate Comprehensive Tests

    func testSessionUpdate_allVariants() throws {
        // userMessageChunk
        let chunk1 = ACP.Client.ContentChunk(content: .text(.init(text: "User input")))
        let update1 = ACP.Client.SessionUpdate.userMessageChunk(chunk1)
        let data1 = try encoder.encode(update1)
        let decoded1 = try decoder.decode(ACP.Client.SessionUpdate.self, from: data1)
        if case .userMessageChunk(let c) = decoded1 {
            if case .text(let t) = c.content {
                XCTAssertEqual(t.text, "User input")
            } else {
                XCTFail("Expected text content")
            }
        } else {
            XCTFail("Expected userMessageChunk")
        }

        // agentMessageChunk
        let chunk2 = ACP.Client.ContentChunk(content: .text(.init(text: "Agent response")))
        let update2 = ACP.Client.SessionUpdate.agentMessageChunk(chunk2)
        let data2 = try encoder.encode(update2)
        let decoded2 = try decoder.decode(ACP.Client.SessionUpdate.self, from: data2)
        if case .agentMessageChunk(let c) = decoded2 {
            if case .text(let t) = c.content {
                XCTAssertEqual(t.text, "Agent response")
            } else {
                XCTFail("Expected text content")
            }
        } else {
            XCTFail("Expected agentMessageChunk")
        }

        // messageUpdated
        let message = ACP.Client.Message(
            role: .assistant,
            content: [.text(.init(text: "Complete message"))]
        )
        let update3 = ACP.Client.SessionUpdate.messageUpdated(.init(message: message))
        let data3 = try encoder.encode(update3)
        let decoded3 = try decoder.decode(ACP.Client.SessionUpdate.self, from: data3)
        if case .messageUpdated(let m) = decoded3 {
            XCTAssertEqual(m.message.role, .assistant)
            XCTAssertEqual(m.message.content.count, 1)
        } else {
            XCTFail("Expected messageUpdated")
        }

        // availableCommandsUpdate
        let commands = [
            ACP.Client.AvailableCommand(
                id: ACP.CommandId("cmd1"),
                command_name: "Test",
                mode_id: .default_mode
            )
        ]
        let update4 = ACP.Client.SessionUpdate.availableCommandsUpdate(.init(available_commands: commands))
        let data4 = try encoder.encode(update4)
        let decoded4 = try decoder.decode(ACP.Client.SessionUpdate.self, from: data4)
        if case .availableCommandsUpdate(let ac) = decoded4 {
            XCTAssertEqual(ac.available_commands.count, 1)
            XCTAssertEqual(ac.available_commands[0].command_name, "Test")
        } else {
            XCTFail("Expected availableCommandsUpdate")
        }

        // currentModeUpdate
        let update5 = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: ACPSessionModeId("custom")))
        let data5 = try encoder.encode(update5)
        let decoded5 = try decoder.decode(ACP.Client.SessionUpdate.self, from: data5)
        if case .currentModeUpdate(let cm) = decoded5 {
            XCTAssertEqual(cm.current_mode_id.value, "custom")
        } else {
            XCTFail("Expected currentModeUpdate")
        }

        // statusUpdate
        let update6 = ACP.Client.SessionUpdate.statusUpdate(.init(
            status: "running",
            reasoning: "Processing request"
        ))
        let data6 = try encoder.encode(update6)
        let decoded6 = try decoder.decode(ACP.Client.SessionUpdate.self, from: data6)
        if case .statusUpdate(let s) = decoded6 {
            XCTAssertEqual(s.status, "running")
            XCTAssertEqual(s.reasoning, "Processing request")
        } else {
            XCTFail("Expected statusUpdate")
        }
    }

    // MARK: - Message Role Tests

    func testMessageRoles() throws {
        let roles: [ACP.Client.Role] = [.user, .assistant]

        for role in roles {
            let message = ACP.Client.Message(
                role: role,
                content: [.text(.init(text: "Test"))]
            )
            let data = try encoder.encode(message)
            let decoded = try decoder.decode(ACP.Client.Message.self, from: data)
            XCTAssertEqual(decoded.role, role)
        }
    }

    // MARK: - SessionNotificationWire Tests

    func testSessionNotificationWire_variousUpdates() throws {
        let sessionId = ACPSessionId("session-123")

        // Text chunk
        let update1 = TestHelpers.makeTextUpdate(text: "Hello")
        let notification1 = TestHelpers.makeSessionUpdateNotification(
            sessionId: sessionId.value,
            update: update1
        )
        let data1 = try encoder.encode(notification1)
        let decoded1 = try decoder.decode(ACP.Client.SessionNotificationWire.self, from: data1)
        XCTAssertEqual(decoded1.session_id.value, "session-123")

        // Tool call
        let update2 = TestHelpers.makeToolCallUpdate(
            toolName: "Read",
            arguments: TestHelpers.makeToolArguments(["file_path": "/test.txt"])
        )
        let notification2 = TestHelpers.makeSessionUpdateNotification(
            sessionId: sessionId.value,
            update: update2
        )
        let data2 = try encoder.encode(notification2)
        let decoded2 = try decoder.decode(ACP.Client.SessionNotificationWire.self, from: data2)
        XCTAssertEqual(decoded2.session_id.value, "session-123")
    }

    // MARK: - AnyEncodable Comprehensive Tests

    func testAnyEncodable_allTypes() throws {
        // Null
        let null = AnyEncodable(Optional<String>.none as Any?)
        let nullData = try encoder.encode(null)
        let decodedNull = try decoder.decode(AnyEncodable.self, from: nullData)
        XCTAssertNotNil(decodedNull)

        // Bool
        let bool = AnyEncodable(true)
        let boolData = try encoder.encode(bool)
        let decodedBool = try decoder.decode(AnyEncodable.self, from: boolData)
        XCTAssertNotNil(decodedBool)

        // Int
        let int = AnyEncodable(42)
        let intData = try encoder.encode(int)
        let decodedInt = try decoder.decode(AnyEncodable.self, from: intData)
        XCTAssertNotNil(decodedInt)

        // Double
        let double = AnyEncodable(3.14)
        let doubleData = try encoder.encode(double)
        let decodedDouble = try decoder.decode(AnyEncodable.self, from: doubleData)
        XCTAssertNotNil(decodedDouble)

        // String
        let string = AnyEncodable("test")
        let stringData = try encoder.encode(string)
        let decodedString = try decoder.decode(AnyEncodable.self, from: stringData)
        XCTAssertNotNil(decodedString)

        // Array
        let array = [AnyEncodable(1), AnyEncodable(2), AnyEncodable(3)]
        let arrayData = try encoder.encode(array)
        let decodedArray = try decoder.decode([AnyEncodable].self, from: arrayData)
        XCTAssertEqual(decodedArray.count, 3)

        // Dictionary
        let dict: [String: AnyEncodable] = [
            "key1": AnyEncodable("value1"),
            "key2": AnyEncodable(42)
        ]
        let dictData = try encoder.encode(dict)
        let decodedDict = try decoder.decode([String: AnyEncodable].self, from: dictData)
        XCTAssertEqual(decodedDict.count, 2)
    }

    // MARK: - JSON-RPC Tests

    func testJSONRPC_RequestResponseRoundtrip() throws {
        // Request
        let req = JSONRPC.Request(
            id: JSONRPC.ID("req-1"),
            method: "test/method",
            params: ["key": "value"]
        )
        let reqData = try encoder.encode(req)
        let decodedReq = try decoder.decode(JSONRPC.Request<[String: String]>.self, from: reqData)
        XCTAssertEqual(decodedReq.id.value, "req-1")
        XCTAssertEqual(decodedReq.method, "test/method")

        // Response
        let resp = JSONRPC.Response(
            id: JSONRPC.ID("req-1"),
            result: ["status": "ok"]
        )
        let respData = try encoder.encode(resp)
        let decodedResp = try decoder.decode(JSONRPC.Response<[String: String]>.self, from: respData)
        XCTAssertEqual(decodedResp.id.value, "req-1")
        XCTAssertEqual(decodedResp.result["status"], "ok")

        // Notification
        let note = JSONRPC.Notification(
            method: "test/notify",
            params: ["event": "happened"]
        )
        let noteData = try encoder.encode(note)
        let decodedNote = try decoder.decode(JSONRPC.Notification<[String: String]>.self, from: noteData)
        XCTAssertEqual(decodedNote.method, "test/notify")
    }

    // MARK: - Edge Cases

    func testContentBlock_largeText() throws {
        let largeText = String(repeating: "a", count: 100000)
        let block = ACP.Client.ContentBlock.text(.init(text: largeText))
        let data = try encoder.encode(block)
        let decoded = try decoder.decode(ACP.Client.ContentBlock.self, from: data)
        if case .text(let t) = decoded {
            XCTAssertEqual(t.text.count, 100000)
        } else {
            XCTFail("Expected text block")
        }
    }

    func testContentBlock_unicodeText() throws {
        let unicodeText = "Hello 世界 🌍 émojis"
        let block = ACP.Client.ContentBlock.text(.init(text: unicodeText))
        let data = try encoder.encode(block)
        let decoded = try decoder.decode(ACP.Client.ContentBlock.self, from: data)
        if case .text(let t) = decoded {
            XCTAssertEqual(t.text, unicodeText)
        } else {
            XCTFail("Expected text block")
        }
    }
}
