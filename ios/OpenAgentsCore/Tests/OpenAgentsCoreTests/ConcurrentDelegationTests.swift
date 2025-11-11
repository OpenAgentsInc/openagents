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

    // MARK: - Interleaved Chunk Tests

    func testInterleavedChunks_BothSessionsReceiveAllUpdates() async throws {
        // Given - two concurrent delegations
        let parentId = ACPSessionId("parent-interleaved")
        let subId1 = ACPSessionId("sub-1")
        let subId2 = ACPSessionId("sub-2")

        server.registerSessionMapping(subSessionId: subId1, parentSessionId: parentId)
        server.registerSessionMapping(subSessionId: subId2, parentSessionId: parentId)

        var session1Updates: [String] = []
        var session2Updates: [String] = []

        let exp1 = expectation(description: "session 1 gets all chunks")
        exp1.expectedFulfillmentCount = 3
        let exp2 = expectation(description: "session 2 gets all chunks")
        exp2.expectedFulfillmentCount = 3

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               case .agentMessageChunk(let chunk) = notification.update,
               case .text(let textContent) = chunk.content {

                // Track which original session this came from
                if notification.session_id == parentId {
                    // This was forwarded - check metadata to determine original session
                    // For this test, we'll track by content
                    if textContent.text.contains("Session1") {
                        session1Updates.append(textContent.text)
                        exp1.fulfill()
                    } else if textContent.text.contains("Session2") {
                        session2Updates.append(textContent.text)
                        exp2.fulfill()
                    }
                }
            }
        }
        defer { sub.cancel() }

        // When - send interleaved chunks
        let chunk1a = ACP.Client.ContentChunk(content: .text(.init(text: "Session1-ChunkA")))
        let chunk2a = ACP.Client.ContentChunk(content: .text(.init(text: "Session2-ChunkA")))
        let chunk1b = ACP.Client.ContentChunk(content: .text(.init(text: "Session1-ChunkB")))
        let chunk2b = ACP.Client.ContentChunk(content: .text(.init(text: "Session2-ChunkB")))
        let chunk1c = ACP.Client.ContentChunk(content: .text(.init(text: "Session1-ChunkC")))
        let chunk2c = ACP.Client.ContentChunk(content: .text(.init(text: "Session2-ChunkC")))

        await server.sendSessionUpdate(sessionId: subId1, update: .agentMessageChunk(chunk1a))
        await server.sendSessionUpdate(sessionId: subId2, update: .agentMessageChunk(chunk2a))
        await server.sendSessionUpdate(sessionId: subId1, update: .agentMessageChunk(chunk1b))
        await server.sendSessionUpdate(sessionId: subId2, update: .agentMessageChunk(chunk2b))
        await server.sendSessionUpdate(sessionId: subId1, update: .agentMessageChunk(chunk1c))
        await server.sendSessionUpdate(sessionId: subId2, update: .agentMessageChunk(chunk2c))

        // Then - both sessions receive all their chunks in order
        await fulfillment(of: [exp1, exp2], timeout: 2.0)
        XCTAssertEqual(session1Updates.count, 3, "Session 1 should receive all 3 chunks")
        XCTAssertEqual(session2Updates.count, 3, "Session 2 should receive all 3 chunks")

        // Verify order preserved
        XCTAssertEqual(session1Updates[0], "Session1-ChunkA")
        XCTAssertEqual(session1Updates[1], "Session1-ChunkB")
        XCTAssertEqual(session1Updates[2], "Session1-ChunkC")

        XCTAssertEqual(session2Updates[0], "Session2-ChunkA")
        XCTAssertEqual(session2Updates[1], "Session2-ChunkB")
        XCTAssertEqual(session2Updates[2], "Session2-ChunkC")
    }

    func testNonConsecutiveChunks_SameSessionAggregates() async throws {
        // Given - parent with sub-session
        let parentId = ACPSessionId("parent-nonconsec")
        let subId = ACPSessionId("sub-nonconsec")
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        var receivedChunks: [String] = []
        let exp = expectation(description: "all chunks received")
        exp.expectedFulfillmentCount = 5

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == parentId,
               case .agentMessageChunk(let chunk) = notification.update,
               case .text(let textContent) = chunk.content {
                receivedChunks.append(textContent.text)
                exp.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - send 5 chunks (simulating non-consecutive arrival)
        for i in 1...5 {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "Chunk\(i)")))
            await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(chunk))
        }

        // Then - all chunks forwarded to parent
        await fulfillment(of: [exp], timeout: 2.0)
        XCTAssertEqual(receivedChunks.count, 5, "Should receive all 5 chunks")
        XCTAssertEqual(receivedChunks, ["Chunk1", "Chunk2", "Chunk3", "Chunk4", "Chunk5"], "Order should be preserved")
    }

    func testMultipleConcurrentSessions_IsolatedUpdates() async throws {
        // Given - 3 concurrent delegations
        let parentId = ACPSessionId("parent-multi")
        let subId1 = ACPSessionId("sub-a")
        let subId2 = ACPSessionId("sub-b")
        let subId3 = ACPSessionId("sub-c")

        server.registerSessionMapping(subSessionId: subId1, parentSessionId: parentId)
        server.registerSessionMapping(subSessionId: subId2, parentSessionId: parentId)
        server.registerSessionMapping(subSessionId: subId3, parentSessionId: parentId)

        var allUpdates: [(sessionId: String, text: String)] = []
        let exp = expectation(description: "all updates received")
        exp.expectedFulfillmentCount = 9 // 3 chunks per session

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == parentId,
               case .agentMessageChunk(let chunk) = notification.update,
               case .text(let textContent) = chunk.content {
                // Track which sub-session by content prefix
                let sessionTag = String(textContent.text.prefix(1)) // A, B, or C
                allUpdates.append((sessionTag, textContent.text))
                exp.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - send interleaved chunks from 3 sessions
        await server.sendSessionUpdate(sessionId: subId1, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "A1")))))
        await server.sendSessionUpdate(sessionId: subId2, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "B1")))))
        await server.sendSessionUpdate(sessionId: subId3, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "C1")))))

        await server.sendSessionUpdate(sessionId: subId1, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "A2")))))
        await server.sendSessionUpdate(sessionId: subId2, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "B2")))))
        await server.sendSessionUpdate(sessionId: subId3, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "C2")))))

        await server.sendSessionUpdate(sessionId: subId1, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "A3")))))
        await server.sendSessionUpdate(sessionId: subId2, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "B3")))))
        await server.sendSessionUpdate(sessionId: subId3, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "C3")))))

        // Then - verify all updates received
        await fulfillment(of: [exp], timeout: 3.0)
        XCTAssertEqual(allUpdates.count, 9, "Should receive all 9 chunks")

        // Verify each session's chunks are present
        let aChunks = allUpdates.filter { $0.sessionId == "A" }.map { $0.text }
        let bChunks = allUpdates.filter { $0.sessionId == "B" }.map { $0.text }
        let cChunks = allUpdates.filter { $0.sessionId == "C" }.map { $0.text }

        XCTAssertEqual(aChunks.count, 3)
        XCTAssertEqual(bChunks.count, 3)
        XCTAssertEqual(cChunks.count, 3)

        // Verify order within each session
        XCTAssertEqual(aChunks, ["A1", "A2", "A3"])
        XCTAssertEqual(bChunks, ["B1", "B2", "B3"])
        XCTAssertEqual(cChunks, ["C1", "C2", "C3"])
    }

    func testChunkAggregation_PreservesOrderAcrossGaps() async throws {
        // Given - simulating real scenario where chunks arrive with gaps
        let parentId = ACPSessionId("parent-gaps")
        let subId = ACPSessionId("sub-gaps")
        server.registerSessionMapping(subSessionId: subId, parentSessionId: parentId)

        var orderedChunks: [String] = []
        let exp = expectation(description: "chunks with gaps")
        exp.expectedFulfillmentCount = 4

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == parentId,
               case .agentMessageChunk(let chunk) = notification.update,
               case .text(let textContent) = chunk.content {
                orderedChunks.append(textContent.text)
                exp.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - send chunks: 1, 2, [mode update], 3, 4
        await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Step1")))))
        await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Step2")))))

        // Simulate a mode update or other non-chunk update
        await server.sendSessionUpdate(sessionId: subId, update: .currentModeUpdate(ACP.Client.CurrentModeUpdate(current_mode_id: .codex)))

        await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Step3")))))
        await server.sendSessionUpdate(sessionId: subId, update: .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Step4")))))

        // Then - all chunks received in order
        await fulfillment(of: [exp], timeout: 2.0)
        XCTAssertEqual(orderedChunks, ["Step1", "Step2", "Step3", "Step4"], "Chunks should maintain order even with gaps")
    }

    // MARK: - Mode Transition Tests

    func testModeTransition_ToCodex_EmitsModeUpdate() async throws {
        // Given
        let sessionId = ACPSessionId("test-mode-transition")

        var receivedUpdates: [ACP.Client.SessionUpdate] = []
        let exp = expectation(description: "mode update received")

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == sessionId {
                receivedUpdates.append(notification.update)
                if case .currentModeUpdate = notification.update {
                    exp.fulfill()
                }
            }
        }
        defer { sub.cancel() }

        // When - switch to Codex mode
        await server.localSessionSetMode(sessionId: sessionId, mode: .codex)

        // Then - mode update should be emitted
        await fulfillment(of: [exp], timeout: 1.0)

        let modeUpdates = receivedUpdates.compactMap { update -> ACPSessionModeId? in
            if case .currentModeUpdate(let modeUpdate) = update {
                return modeUpdate.current_mode_id
            }
            return nil
        }

        XCTAssertTrue(modeUpdates.contains(.codex), "Should have emitted Codex mode update")
    }

    func testModeTransition_ToClaudeCode_EmitsModeUpdate() async throws {
        // Given
        let sessionId = ACPSessionId("test-claude-code-mode")

        var modeUpdates: [ACPSessionModeId] = []
        let exp = expectation(description: "Claude Code mode update")

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == sessionId,
               case .currentModeUpdate(let update) = notification.update {
                modeUpdates.append(update.current_mode_id)
                if update.current_mode_id == .claude_code {
                    exp.fulfill()
                }
            }
        }
        defer { sub.cancel() }

        // When
        await server.localSessionSetMode(sessionId: sessionId, mode: .claude_code)

        // Then
        await fulfillment(of: [exp], timeout: 1.0)
        XCTAssertTrue(modeUpdates.contains(.claude_code), "Should emit Claude Code mode update")
    }

    func testDelegationVsDirectMode_BothShowIndicators() async throws {
        // Given - parent session that will have both delegation and direct mode switch
        let parentId = ACPSessionId("test-both-patterns")
        guard let hub = server.updateHub else {
            XCTFail("UpdateHub not initialized")
            return
        }

        var toolCalls: [String] = []
        var modeUpdates: [ACPSessionModeId] = []

        let exp = expectation(description: "delegation and mode switch")
        exp.expectedFulfillmentCount = 2 // 1 tool call + 1 mode update

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == parentId {
                switch notification.update {
                case .toolCall(let call):
                    toolCalls.append(call.name)
                    exp.fulfill()
                case .currentModeUpdate(let update):
                    modeUpdates.append(update.current_mode_id)
                    if update.current_mode_id == .codex {
                        exp.fulfill()
                    }
                default:
                    break
                }
            }
        }
        defer { sub.cancel() }

        // When - first do a delegation (creates tool call)
        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            let tool = OpenAgentsLocalProvider.FMTool_DelegateRun(
                sessionId: parentId,
                updateHub: hub,
                workspaceRoot: FileManager.default.currentDirectoryPath,
                server: server
            )

            // This creates a tool call and a sub-session
            _ = try? await tool.call(arguments: .init(user_prompt: "test task", provider: "codex", description: nil))
        }
        #endif

        // Then directly switch mode (creates mode update)
        await server.localSessionSetMode(sessionId: parentId, mode: .codex)

        // Then - should have both indicators
        await fulfillment(of: [exp], timeout: 2.0)

        // Verify we got the tool call from delegation
        XCTAssertTrue(toolCalls.contains(ToolName.delegate.rawValue), "Should have delegate.run tool call")

        // Verify we got the mode update from direct switch
        XCTAssertTrue(modeUpdates.contains(.codex), "Should have Codex mode update")
    }

    func testMultipleModeSwitches_AllEmitted() async throws {
        // Given
        let sessionId = ACPSessionId("test-multi-mode")

        var modeSequence: [ACPSessionModeId] = []
        let exp = expectation(description: "multiple mode switches")
        exp.expectedFulfillmentCount = 3

        let sub = server.notificationPublisher.sink { event in
            guard event.method == ACPRPC.sessionUpdate else { return }
            if let notification = try? JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: event.payload),
               notification.session_id == sessionId,
               case .currentModeUpdate(let update) = notification.update {
                modeSequence.append(update.current_mode_id)
                exp.fulfill()
            }
        }
        defer { sub.cancel() }

        // When - switch modes multiple times
        await server.localSessionSetMode(sessionId: sessionId, mode: .codex)
        await server.localSessionSetMode(sessionId: sessionId, mode: .claude_code)
        await server.localSessionSetMode(sessionId: sessionId, mode: .default_mode)

        // Then - all switches should be recorded
        await fulfillment(of: [exp], timeout: 2.0)
        XCTAssertEqual(modeSequence.count, 3, "Should have 3 mode updates")
        XCTAssertEqual(modeSequence[0], .codex)
        XCTAssertEqual(modeSequence[1], .claude_code)
        XCTAssertEqual(modeSequence[2], .default_mode)
    }
}
#endif
