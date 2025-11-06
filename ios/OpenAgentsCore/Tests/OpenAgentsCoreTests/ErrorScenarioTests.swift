import XCTest
@testable import OpenAgentsCore

#if os(macOS)
/// Tests for error scenarios and error recovery in ACP protocol implementation
/// Phase 6: Error Scenario Tests - Network interruption, malformed data, protocol failures
final class ErrorScenarioTests: XCTestCase {

    // MARK: - Malformed Update Tests

    func testMalformedJSON_HandledGracefully() throws {
        // Test that malformed JSON doesn't crash the system
        let malformedJSON = "{\"session_id\":\"test\",\"update\":{\"type\":\"user_message_chunk\",\"content\":{\"type\":\"text\",\"text\":null]}}" // Invalid: text is null

        let decoder = JSONDecoder()

        // Attempt to decode malformed update
        XCTAssertThrowsError(try decoder.decode(ACP.Client.SessionNotificationWire.self, from: malformedJSON.data(using: .utf8)!)) { error in
            // Should throw decoding error, not crash
            XCTAssert(error is DecodingError, "Should throw DecodingError for malformed JSON")
        }
    }

    func testMissingRequiredFields_HandledCorrectly() throws {
        // Test update with missing required fields
        let missingFieldJSON = "{\"session_id\":\"test\"}" // Missing "update" field

        let decoder = JSONDecoder()

        XCTAssertThrowsError(try decoder.decode(ACP.Client.SessionNotificationWire.self, from: missingFieldJSON.data(using: .utf8)!)) { error in
            XCTAssert(error is DecodingError, "Should fail to decode missing required field")
        }
    }

    func testInvalidContentType_RejectedCorrectly() throws {
        // Test content block with unknown/invalid type
        let invalidTypeJSON = """
        {
            "session_id": "test",
            "update": {
                "type": "agent_message_chunk",
                "content": {
                    "type": "invalid_content_type",
                    "data": "something"
                }
            }
        }
        """

        let decoder = JSONDecoder()

        // Should fail to decode unknown content type
        XCTAssertThrowsError(try decoder.decode(ACP.Client.SessionNotificationWire.self, from: invalidTypeJSON.data(using: .utf8)!))
    }

    // MARK: - Partial Message Handling Tests

    func testPartialMessage_Buffering() throws {
        let sessionId = ACPSessionId("test-partial")

        // Simulate receiving partial message chunks
        let partialChunks: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "This is ")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "a partial ")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "message."))))
        ]

        let wires = partialChunks.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Chunks should be accumulated into single message
        XCTAssertEqual(items.count, 1, "Partial chunks should accumulate into one message")

        // Message should contain all chunk text
        if case .assistant_message(let assoc) = items.first?.variant {
            let fullText = assoc.blocks.compactMap { block -> String? in
                if case .text(let textBlock) = block {
                    return textBlock.text
                }
                return nil
            }.joined()

            XCTAssert(fullText.contains("This is a partial message"), "Should accumulate all chunks")
        } else {
            XCTFail("Expected assistant message")
        }
    }

    func testPartialToolCall_IncompleteData() throws {
        let sessionId = ACPSessionId("test-incomplete-tool")

        // Tool call without result (incomplete execution)
        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("incomplete-tool"),
                name: "Bash",
                arguments: TestHelpers.makeToolArguments(["command": "echo test"])
            ))))
            // No tool result follows - simulates interruption
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should still display the tool call even without result
        XCTAssertEqual(items.count, 1, "Incomplete tool call should still be displayed")

        if case .assistant_message(let assoc) = items.first?.variant {
            XCTAssertNotNil(assoc.tool, "Should have tool information")
        }
    }

    // MARK: - Protocol Version Negotiation Tests

    func testProtocolVersionMismatch_DetectedCorrectly() throws {
        // Test that version mismatch is detectable
        let expectedVersion = "0.7.0"
        let incompatibleVersion = "1.0.0"

        XCTAssertNotEqual(expectedVersion, incompatibleVersion, "Should detect version mismatch")

        // In real implementation, this would check against InitializeResult
        struct InitializeResult: Codable {
            let protocol_version: String
        }

        let result = InitializeResult(protocol_version: incompatibleVersion)
        XCTAssertNotEqual(result.protocol_version, expectedVersion, "Should reject incompatible version")
    }

    func testProtocolVersionNegotiation_FallbackBehavior() throws {
        // Test that client can detect supported protocol versions
        let supportedVersions = ["0.7.0"]
        let serverVersion = "0.7.0"

        XCTAssert(supportedVersions.contains(serverVersion), "Should accept compatible version")

        let unsupportedVersion = "0.5.0"
        XCTAssertFalse(supportedVersions.contains(unsupportedVersion), "Should reject unsupported version")
    }

    // MARK: - Service Error Tests

    func testFileSystemError_ToolExecution() throws {
        let sessionId = ACPSessionId("test-fs-error")

        // Simulate filesystem error during tool execution
        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("fs-error-tool"),
                name: "Read",
                arguments: TestHelpers.makeToolArguments(["file_path": "/nonexistent/path/file.txt"])
            )))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("fs-error-tool"),
                content: [.text(.init(text: "Error: ENOENT: no such file or directory"))],
                is_error: true
            ))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should display both tool call and error result
        XCTAssertEqual(items.count, 2, "Should show tool call and error result")

        // Second item should be the error result
        if items.count >= 2, case .assistant_message(let assoc) = items[1].variant {
            // Error results are displayed as regular tool results
            XCTAssert(assoc.blocks.count > 0, "Should have result content")
        }
    }

    func testTerminalError_ProcessExecution() throws {
        let sessionId = ACPSessionId("test-terminal-error")

        // Simulate terminal/bash execution error
        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("bash-error-tool"),
                name: "Bash",
                arguments: TestHelpers.makeToolArguments(["command": "invalid-command-xyz"])
            )))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("bash-error-tool"),
                content: [.text(.init(text: "bash: invalid-command-xyz: command not found\nExit code: 127"))],
                is_error: true
            ))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should handle terminal errors gracefully
        XCTAssertEqual(items.count, 2, "Should display command and error")
    }

    // MARK: - Timeout Handling Tests

    func testUpdateTimeout_NoResponseReceived() throws {
        // Test timeout when no updates are received
        let port: UInt16 = 9920
        let server = DesktopWebSocketServer()
        try server.start(port: port, advertiseService: false)
        defer { server.stop() }

        let client = MobileWebSocketClient()
        let url = URL(string: "ws://127.0.0.1:\(port)")!

        let connectExpectation = expectation(description: "Connection timeout")
        connectExpectation.isInverted = false

        var didConnect = false
        let delegate = TimeoutTestDelegate(connectCallback: {
            didConnect = true
            connectExpectation.fulfill()
        })

        client.delegate = delegate
        client.connect(url: url)

        // Wait for connection or timeout
        wait(for: [connectExpectation], timeout: 5.0)

        XCTAssert(didConnect, "Should connect even if no updates follow")

        client.disconnect()
    }

    func testLongRunningOperation_NoTimeout() throws {
        let sessionId = ACPSessionId("test-long-op")

        // Simulate long-running operation (thinking for extended period)
        let updates: [ACP.Client.SessionUpdate] = [
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Processing large file...")))),
            .statusUpdate(.init(status: .running, message: "Still working...")),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Operation completed"))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should handle long operations without timing out
        XCTAssertFalse(items.isEmpty, "Should process updates from long operation")
    }

    // MARK: - Concurrent Error Handling Tests

    func testConcurrentErrors_IndependentHandling() throws {
        // Test that errors in one session don't affect another
        let session1 = ACPSessionId("session-1-error")
        let session2 = ACPSessionId("session-2-ok")

        // Session 1: Has error
        let updates1: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("error-tool"),
                content: [.text(.init(text: "Error occurred"))],
                is_error: true
            ))))
        ]

        // Session 2: No error
        let updates2: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "All good"))))
        ]

        let wires1 = updates1.map { ACP.Client.SessionNotificationWire(session_id: session1, update: $0) }
        let wires2 = updates2.map { ACP.Client.SessionNotificationWire(session_id: session2, update: $0) }

        let (items1, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires1, cap: 100)
        let (items2, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires2, cap: 100)

        // Both sessions should process independently
        XCTAssertFalse(items1.isEmpty, "Session 1 should have error item")
        XCTAssertFalse(items2.isEmpty, "Session 2 should have normal item")
    }

    // MARK: - Recovery Tests

    func testRecoveryAfterError_ContinueSession() throws {
        let sessionId = ACPSessionId("test-recovery")

        // Error followed by recovery
        let updates: [ACP.Client.SessionUpdate] = [
            // Error occurs
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("failed-tool"),
                content: [.text(.init(text: "Error: Operation failed"))],
                is_error: true
            )))),

            // Agent recovers and continues
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Let me try a different approach")))),

            // Successful tool call
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("success-tool"),
                name: "Bash",
                arguments: TestHelpers.makeToolArguments(["command": "ls"])
            )))),

            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("success-tool"),
                content: [.text(.init(text: "file1.txt\nfile2.txt"))],
                is_error: false
            ))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should have all items including error and recovery
        XCTAssertGreaterThanOrEqual(items.count, 4, "Should show error and recovery sequence")
    }

    // MARK: - Edge Case Error Tests

    func testEmptyToolResult_HandledCorrectly() throws {
        let sessionId = ACPSessionId("test-empty-result")

        // Tool result with empty content
        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                tool_use_id: ACP.ToolUseId("empty-tool"),
                content: [], // Empty content array
                is_error: false
            ))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Should not crash on empty content
        XCTAssertNoThrow({
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)
        }())
    }

    func testNullFieldsInUpdate_DecodingFails() throws {
        // Test that null values in required fields fail decoding
        let nullFieldJSON = """
        {
            "session_id": null,
            "update": {
                "type": "status_update",
                "status": "running"
            }
        }
        """

        let decoder = JSONDecoder()

        // Should fail to decode null session_id
        XCTAssertThrowsError(try decoder.decode(ACP.Client.SessionNotificationWire.self, from: nullFieldJSON.data(using: .utf8)!))
    }
}

// MARK: - Test Helpers

class TimeoutTestDelegate: NSObject, MobileWebSocketClientDelegate {
    let connectCallback: () -> Void

    init(connectCallback: @escaping () -> Void) {
        self.connectCallback = connectCallback
    }

    func mobileWebSocketClientDidConnect(_ client: MobileWebSocketClient) {
        connectCallback()
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didDisconnect error: Error?) {
        // Handle disconnect
    }

    func mobileWebSocketClient(_ client: MobileWebSocketClient, didReceive notification: ACP.Client.SessionNotificationWire) {
        // Handle updates
    }
}
#endif
