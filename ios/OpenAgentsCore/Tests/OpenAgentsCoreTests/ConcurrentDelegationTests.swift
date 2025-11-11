#if os(macOS)
import XCTest
@testable import OpenAgentsCore

final class ConcurrentDelegationTests: XCTestCase {
    var tempDbPath: String!
    var server: DesktopWebSocketServer!
    var tinyvexDb: TinyvexDbLayer!

    override func setUp() async throws {
        try await super.setUp()

        // Create temp database
        let tempDir = FileManager.default.temporaryDirectory
        let dbFile = tempDir.appendingPathComponent("test-delegation-\(UUID().uuidString).db")
        tempDbPath = dbFile.path
        tinyvexDb = try TinyvexDbLayer(path: tempDbPath)

        // Create server with Tinyvex
        server = DesktopWebSocketServer()
        server.setTinyvexDb(path: tempDbPath)
    }

    override func tearDown() async throws {
        server = nil
        tinyvexDb = nil
        if let path = tempDbPath {
            try? FileManager.default.removeItem(atPath: path)
        }
        tempDbPath = nil
        try await super.tearDown()
    }

    // MARK: - Session Mapping Tests

    func testRegisterSessionMapping_CreatesMapping() {
        // Given
        let parentId = ACPSessionId("parent-123")
        let subId = ACPSessionId("sub-456")

        // When
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        // Then - verify mapping exists by sending update to sub-session
        let expectation = expectation(description: "update forwarded to parent")
        var receivedSessionId: ACPSessionId?

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload) {
                receivedSessionId = notification.session_id
                expectation.fulfill()
            }
        }
        defer { sub.cancel() }

        Task {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
            await server.sendSessionUpdate(sessionId: subId, update: .userMessageChunk(chunk))
        }

        wait(for: [expectation], timeout: 2.0)
        XCTAssertEqual(receivedSessionId, parentId, "Update should be forwarded to parent session")
    }

    func testUnregisterSessionMapping_RemovesMapping() {
        // Given
        let parentId = ACPSessionId("parent-789")
        let subId = ACPSessionId("sub-abc")
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        // When
        server.unregisterSessionMapping(subSessionId: subId)

        // Then - updates should go to sub-session, not parent
        let expectation = expectation(description: "update sent to sub-session")
        var receivedSessionId: ACPSessionId?

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload) {
                receivedSessionId = notification.session_id
                expectation.fulfill()
            }
        }
        defer { sub.cancel() }

        Task {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
            await server.sendSessionUpdate(sessionId: subId, update: .userMessageChunk(chunk))
        }

        wait(for: [expectation], timeout: 2.0)
        XCTAssertEqual(receivedSessionId, subId, "After unregistering, update should go to sub-session directly")
    }

    func testMultipleSessionMappings_IndependentForwarding() async throws {
        // Given
        let parent1 = ACPSessionId("parent-1")
        let parent2 = ACPSessionId("parent-2")
        let sub1 = ACPSessionId("sub-1")
        let sub2 = ACPSessionId("sub-2")

        server.registerSessionMapping(subSessionId: sub1, parentSessionId: parent1)
        server.registerSessionMapping(subSessionId: sub2, parentSessionId: parent2)

        // When/Then - verify each sub-session forwards to correct parent
        let exp1 = expectation(description: "sub1 forwards to parent1")
        let exp2 = expectation(description: "sub2 forwards to parent2")
        var updates: [ACPSessionId] = []

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload) {
                updates.append(notification.session_id)
                if notification.session_id == parent1 { exp1.fulfill() }
                if notification.session_id == parent2 { exp2.fulfill() }
            }
        }
        defer { sub.cancel() }

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
        await server.sendSessionUpdate(sessionId: sub1, update: .userMessageChunk(chunk))
        await server.sendSessionUpdate(sessionId: sub2, update: .agentMessageChunk(chunk))

        await fulfillment(of: [exp1, exp2], timeout: 2.0)
        XCTAssertTrue(updates.contains(parent1))
        XCTAssertTrue(updates.contains(parent2))
        XCTAssertFalse(updates.contains(sub1))
        XCTAssertFalse(updates.contains(sub2))
    }

    // MARK: - Update Forwarding Tests

    func testUpdateForwarding_PreservesUpdateContent() async throws {
        // Given
        let parentId = ACPSessionId("parent-content-test")
        let subId = ACPSessionId("sub-content-test")
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        let expectation = expectation(description: "content preserved")
        var receivedText: String?

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               case .agentMessageChunk(let chunk) = notification.update,
               case .text(let textContent) = chunk.content {
                receivedText = textContent.text
                expectation.fulfill()
            }
        }
        defer { sub.cancel() }

        // When
        let originalText = "Codex processing subfolder listing..."
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: originalText)))
        await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(chunk))

        // Then
        await fulfillment(of: [expectation], timeout: 2.0)
        XCTAssertEqual(receivedText, originalText, "Content should be preserved during forwarding")
    }

    func testUpdateForwarding_MultipleUpdateTypes() async throws {
        // Given
        let parentId = ACPSessionId("parent-types-test")
        let subId = ACPSessionId("sub-types-test")
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        let userMsgExp = expectation(description: "user message forwarded")
        let agentMsgExp = expectation(description: "agent message forwarded")
        let toolCallExp = expectation(description: "tool call forwarded")

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload) {
                switch notification.update {
                case .userMessageChunk: userMsgExp.fulfill()
                case .agentMessageChunk: agentMsgExp.fulfill()
                case .toolCall: toolCallExp.fulfill()
                default: break
                }
            }
        }
        defer { sub.cancel() }

        // When - send different update types
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
        await server.sendSessionUpdate(sessionId: subId, update: .userMessageChunk(chunk))
        await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(chunk))

        let toolCall = ACPToolCallWire(call_id: "tc-123", name: "bash", arguments: nil)
        await server.sendSessionUpdate(sessionId: subId, update: .toolCall(toolCall))

        // Then
        await fulfillment(of: [userMsgExp, agentMsgExp, toolCallExp], timeout: 2.0)
    }

    // MARK: - Concurrent Delegation Tests

    #if canImport(FoundationModels)
    @available(macOS 26.0, *)
    func testConcurrentDelegations_CreateSeparateSubSessions() async throws {
        // Given
        let parentSessionId = ACPSessionId("fm-parent-session")
        guard let hub = server.updateHub else {
            XCTFail("UpdateHub not initialized")
            return
        }

        var createdSubSessions: Set<String> = []
        let sessionCreationExp = expectation(description: "sub-sessions created")
        sessionCreationExp.expectedFulfillmentCount = 3

        // Track mode updates to detect sub-session creation
        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               case .currentModeUpdate(let modeUpdate) = notification.update,
               modeUpdate.current_mode_id == .codex,
               notification.session_id != parentSessionId {
                createdSubSessions.insert(notification.session_id.value)
                sessionCreationExp.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - create 3 concurrent delegations
        let tool1 = OpenAgentsLocalProvider.FMTool_DelegateRun(
            sessionId: parentSessionId,
            updateHub: hub,
            workspaceRoot: FileManager.default.currentDirectoryPath,
            server: server
        )
        let tool2 = OpenAgentsLocalProvider.FMTool_DelegateRun(
            sessionId: parentSessionId,
            updateHub: hub,
            workspaceRoot: FileManager.default.currentDirectoryPath,
            server: server
        )
        let tool3 = OpenAgentsLocalProvider.FMTool_DelegateRun(
            sessionId: parentSessionId,
            updateHub: hub,
            workspaceRoot: FileManager.default.currentDirectoryPath,
            server: server
        )

        async let result1 = tool1.call(arguments: .init(user_prompt: "list files", provider: "codex", description: nil))
        async let result2 = tool2.call(arguments: .init(user_prompt: "analyze code", provider: "codex", description: nil))
        async let result3 = tool3.call(arguments: .init(user_prompt: "refactor module", provider: "codex", description: nil))

        _ = try await [result1, result2, result3]

        // Then - verify 3 unique sub-sessions created
        await fulfillment(of: [sessionCreationExp], timeout: 3.0)
        XCTAssertEqual(createdSubSessions.count, 3, "Should create 3 unique sub-sessions for concurrent delegations")
    }

    @available(macOS 26.0, *)
    func testConcurrentDelegations_AllUpdatesForwardToParent() async throws {
        // Given
        let parentSessionId = ACPSessionId("fm-concurrent-parent")
        guard let hub = server.updateHub else {
            XCTFail("UpdateHub not initialized")
            return
        }

        var parentUpdates: [ACP.Client.SessionUpdate] = []
        let updatesExp = expectation(description: "updates forwarded to parent")
        updatesExp.expectedFulfillmentCount = 6 // 3 tool calls + 3 mode updates minimum

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == parentSessionId {
                parentUpdates.append(notification.update)
                updatesExp.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - create concurrent delegations
        let tool = OpenAgentsLocalProvider.FMTool_DelegateRun(
            sessionId: parentSessionId,
            updateHub: hub,
            workspaceRoot: FileManager.default.currentDirectoryPath,
            server: server
        )

        async let _ = tool.call(arguments: .init(user_prompt: "task 1", provider: "codex", description: nil))
        async let _ = tool.call(arguments: .init(user_prompt: "task 2", provider: "codex", description: nil))
        async let _ = tool.call(arguments: .init(user_prompt: "task 3", provider: "codex", description: nil))

        // Then - all updates routed to parent session
        await fulfillment(of: [updatesExp], timeout: 3.0)

        let toolCalls = parentUpdates.compactMap { update -> ACPToolCallWire? in
            if case .toolCall(let tc) = update { return tc }
            return nil
        }
        XCTAssertGreaterThanOrEqual(toolCalls.count, 3, "Should receive tool call updates from all delegations")
    }
    #endif

    // MARK: - Cleanup Tests

    func testSessionMapping_ClearsAfterDelegationComplete() async throws {
        // Given
        let parentId = ACPSessionId("cleanup-parent")
        let subId = ACPSessionId("cleanup-sub")

        // Register mapping
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        // Verify mapping active
        let beforeExp = expectation(description: "mapping active before cleanup")
        let beforeSub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == parentId {
                beforeExp.fulfill()
            }
        }
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
        await server.sendSessionUpdate(sessionId: subId, update: .userMessageChunk(chunk))
        await fulfillment(of: [beforeExp], timeout: 1.0)
        beforeSub.cancel()

        // When - unregister (simulating delegation cleanup)
        server.unregisterSessionMapping(subSessionId: subId)

        // Then - mapping no longer active
        let afterExp = expectation(description: "mapping removed after cleanup")
        let afterSub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == subId {
                afterExp.fulfill()
            }
        }
        await server.sendSessionUpdate(sessionId: subId, update: .userMessageChunk(chunk))
        await fulfillment(of: [afterExp], timeout: 1.0)
        afterSub.cancel()
    }

    // MARK: - Edge Cases

    func testUpdateForwarding_UnmappedSessionPassesThrough() async throws {
        // Given - unmapped session (no parent)
        let standaloneId = ACPSessionId("standalone-session")

        let expectation = expectation(description: "unmapped session passes through")
        var receivedSessionId: ACPSessionId?

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload) {
                receivedSessionId = notification.session_id
                expectation.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - send update to unmapped session
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "standalone test")))
        await server.sendSessionUpdate(sessionId: standaloneId, update: .userMessageChunk(chunk))

        // Then - update uses original session ID (no forwarding)
        await fulfillment(of: [expectation], timeout: 2.0)
        XCTAssertEqual(receivedSessionId, standaloneId, "Unmapped session should pass through without forwarding")
    }

    func testSessionMapping_ReregisteringOverwritesPrevious() {
        // Given
        let subId = ACPSessionId("sub-reregister")
        let parent1 = ACPSessionId("parent-1")
        let parent2 = ACPSessionId("parent-2")

        server.registerSessionMapping(subSessionId: subId, parentSessionId: parent1)

        // When - re-register same sub-session with different parent
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parent2)

        // Then - should forward to new parent
        let expectation = expectation(description: "forwards to new parent")
        var receivedSessionId: ACPSessionId?

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload) {
                receivedSessionId = notification.session_id
                expectation.fulfill()
            }
        }
        defer { sub.cancel() }

        Task {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
            await server.sendSessionUpdate(sessionId: subId, update: .userMessageChunk(chunk))
        }

        wait(for: [expectation], timeout: 2.0)
        XCTAssertEqual(receivedSessionId, parent2, "Re-registration should update to new parent")
    }
}
#endif
