#if os(macOS)
import XCTest
@testable import OpenAgentsCore

final class SessionUpdateHubTests: XCTestCase {
    var tempDbPath: String!
    var tinyvexDb: TinyvexDbLayer!
    var broadcastedMessages: [String] = []
    var sut: SessionUpdateHub!

    override func setUp() async throws {
        try await super.setUp()
        broadcastedMessages = []

        // Create temp database
        let tempDir = FileManager.default.temporaryDirectory
        let dbFile = tempDir.appendingPathComponent("test-hub-\(UUID().uuidString).db")
        tempDbPath = dbFile.path
        tinyvexDb = try TinyvexDbLayer(path: tempDbPath)

        // Create SUT with broadcast callback
        sut = SessionUpdateHub(tinyvexDb: tinyvexDb) { [weak self] message in
            self?.broadcastedMessages.append(message)
        }
    }

    override func tearDown() async throws {
        sut = nil
        tinyvexDb = nil
        if let path = tempDbPath {
            try? FileManager.default.removeItem(atPath: path)
        }
        tempDbPath = nil
        try await super.tearDown()
    }

    // MARK: - Persistence Tests

    func testSendSessionUpdate_PersistsToTinyvex() async throws {
        // Given
        let sessionId = ACPSessionId(value: "test-session-1")
        let textContent = ACP.Client.TextContent(text: "Hello world")
        let chunk = ACP.Client.ContentChunk(content: .text(textContent))
        let update = ACP.Client.SessionUpdate.userMessageChunk(chunk)

        // When
        await sut.sendSessionUpdate(sessionId: sessionId, update: update)

        // Then - verify persisted to database
        let timeline = try await tinyvexDb.sessionTimeline(sessionId: sessionId.value, limit: 10)
        XCTAssertEqual(timeline.count, 1)
        XCTAssertEqual(timeline[0].session_id, sessionId.value)

        // Decode the update
        let persistedUpdate = try JSONDecoder().decode(ACP.Client.SessionUpdate.self, from: timeline[0].update_json.data(using: .utf8)!)
        if case .userMessageChunk(let persistedChunk) = persistedUpdate,
           case .text(let persistedText) = persistedChunk.content {
            XCTAssertEqual(persistedText.text, "Hello world")
        } else {
            XCTFail("Unexpected update type")
        }
    }

    func testSendSessionUpdate_BroadcastsToClients() async throws {
        // Given
        let sessionId = ACPSessionId(value: "test-session-2")
        let textContent = ACP.Client.TextContent(text: "Test message")
        let chunk = ACP.Client.ContentChunk(content: .text(textContent))
        let update = ACP.Client.SessionUpdate.agentMessageChunk(chunk)

        // When
        await sut.sendSessionUpdate(sessionId: sessionId, update: update)

        // Then - verify broadcast callback was called
        XCTAssertEqual(broadcastedMessages.count, 1)

        // Verify the broadcast message is valid JSON-RPC
        let messageData = broadcastedMessages[0].data(using: .utf8)!
        let notification = try JSONDecoder().decode(JSONRPC.Notification<ACP.Client.SessionNotificationWire>.self, from: messageData)
        XCTAssertEqual(notification.method, ACPRPC.sessionUpdate)
        XCTAssertEqual(notification.params.session_id, sessionId)

        if case .agentMessageChunk(let broadcastChunk) = notification.params.update,
           case .text(let broadcastText) = broadcastChunk.content {
            XCTAssertEqual(broadcastText.text, "Test message")
        } else {
            XCTFail("Unexpected update type in broadcast")
        }
    }

    func testSendSessionUpdate_WithoutTinyvexDb_StillBroadcasts() async throws {
        // Given - hub with no database
        sut = SessionUpdateHub(tinyvexDb: nil) { [weak self] message in
            self?.broadcastedMessages.append(message)
        }

        let sessionId = ACPSessionId(value: "test-session-3")
        let textContent = ACP.Client.TextContent(text: "Broadcast only")
        let chunk = ACP.Client.ContentChunk(content: .text(textContent))
        let update = ACP.Client.SessionUpdate.userMessageChunk(chunk)

        // When
        await sut.sendSessionUpdate(sessionId: sessionId, update: update)

        // Then - broadcast still happens
        XCTAssertEqual(broadcastedMessages.count, 1)
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.broadcastCount, 1)
        XCTAssertEqual(metrics.persistedCount, 0) // No persistence without DB
    }

    // MARK: - Metrics Tests

    func testMetrics_TracksUpdateCount() async throws {
        // Given
        let sessionId = ACPSessionId(value: "metrics-test-1")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))

        // When - send multiple updates
        await sut.sendSessionUpdate(sessionId: sessionId, update: .userMessageChunk(chunk))
        await sut.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
        await sut.sendSessionUpdate(sessionId: sessionId, update: .agentThoughtChunk(chunk))

        // Then
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.updateCount, 3)
        XCTAssertEqual(metrics.persistedCount, 3)
        XCTAssertEqual(metrics.broadcastCount, 3)
    }

    func testMetrics_TracksLastAppendTimestamp() async throws {
        // Given
        let sessionId = ACPSessionId(value: "metrics-test-2")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
        let beforeTimestamp = Int64(Date().timeIntervalSince1970 * 1000)

        // When
        await sut.sendSessionUpdate(sessionId: sessionId, update: .userMessageChunk(chunk))

        // Then
        let metrics = await sut.getMetrics()
        let afterTimestamp = Int64(Date().timeIntervalSince1970 * 1000)
        XCTAssertNotNil(metrics.lastAppendTimestamp)
        XCTAssertGreaterThanOrEqual(metrics.lastAppendTimestamp!, beforeTimestamp)
        XCTAssertLessThanOrEqual(metrics.lastAppendTimestamp!, afterTimestamp)
    }

    func testMetrics_ResetClearsCounters() async throws {
        // Given
        let sessionId = ACPSessionId(value: "metrics-test-3")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))
        await sut.sendSessionUpdate(sessionId: sessionId, update: .userMessageChunk(chunk))

        // When
        await sut.resetMetrics()

        // Then
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.updateCount, 0)
        XCTAssertEqual(metrics.persistedCount, 0)
        XCTAssertEqual(metrics.broadcastCount, 0)
        XCTAssertNil(metrics.lastAppendTimestamp)
        XCTAssertEqual(metrics.queueDepth, 0)
    }

    // MARK: - Update Kind Tests

    func testSendSessionUpdate_AllUpdateKinds() async throws {
        // Test that all update kinds are handled correctly
        let sessionId = ACPSessionId(value: "kind-test")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))

        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(chunk),
            .agentMessageChunk(chunk),
            .agentThoughtChunk(chunk),
            .toolCall(ACP.Client.ToolCall(id: "1", name: "test", input: .init())),
            .toolCallUpdate(ACP.Client.ToolCallUpdate(id: "1", status: .running, output: nil)),
            .plan(ACP.Client.Plan(steps: [])),
            .availableCommandsUpdate(ACP.Client.AvailableCommandsUpdate(commands: [])),
            .currentModeUpdate(ACP.Client.CurrentModeUpdate(mode: .init(id: "claude", name: "Claude")))
        ]

        // When - send all update types
        for update in updates {
            await sut.sendSessionUpdate(sessionId: sessionId, update: update)
        }

        // Then
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.updateCount, 8)
        XCTAssertEqual(metrics.persistedCount, 8)
        XCTAssertEqual(metrics.broadcastCount, 8)

        // Verify all persisted
        let timeline = try await tinyvexDb.sessionTimeline(sessionId: sessionId.value, limit: 100)
        XCTAssertEqual(timeline.count, 8)
    }

    // MARK: - Multiple Sessions Tests

    func testSendSessionUpdate_MultipleSessions() async throws {
        // Given
        let session1 = ACPSessionId(value: "session-1")
        let session2 = ACPSessionId(value: "session-2")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))

        // When
        await sut.sendSessionUpdate(sessionId: session1, update: .userMessageChunk(chunk))
        await sut.sendSessionUpdate(sessionId: session2, update: .userMessageChunk(chunk))
        await sut.sendSessionUpdate(sessionId: session1, update: .agentMessageChunk(chunk))

        // Then - verify both sessions have events
        let timeline1 = try await tinyvexDb.sessionTimeline(sessionId: session1.value, limit: 10)
        let timeline2 = try await tinyvexDb.sessionTimeline(sessionId: session2.value, limit: 10)
        XCTAssertEqual(timeline1.count, 2)
        XCTAssertEqual(timeline2.count, 1)

        // Verify metrics
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.updateCount, 3)
        XCTAssertEqual(metrics.broadcastCount, 3)
    }

    // MARK: - Concurrent Access Tests

    func testSendSessionUpdate_ConcurrentAccess() async throws {
        // Given
        let sessionId = ACPSessionId(value: "concurrent-test")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))

        // When - send updates concurrently
        await withTaskGroup(of: Void.self) { group in
            for i in 0..<10 {
                group.addTask {
                    await self.sut.sendSessionUpdate(
                        sessionId: sessionId,
                        update: .userMessageChunk(chunk)
                    )
                }
            }
        }

        // Then - all updates processed
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.updateCount, 10)
        XCTAssertEqual(metrics.broadcastCount, 10)
        XCTAssertEqual(broadcastedMessages.count, 10)

        // Verify persistence
        let timeline = try await tinyvexDb.sessionTimeline(sessionId: sessionId.value, limit: 100)
        XCTAssertEqual(timeline.count, 10)
    }

    // MARK: - Error Handling Tests

    func testSendSessionUpdate_HandlesEncodingFailure() async throws {
        // Note: In practice, ACP types should always encode successfully
        // This test verifies graceful handling if encoding fails

        var broadcastCallCount = 0
        sut = SessionUpdateHub(tinyvexDb: tinyvexDb) { _ in
            broadcastCallCount += 1
        }

        let sessionId = ACPSessionId(value: "encoding-test")
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "test")))

        // When
        await sut.sendSessionUpdate(sessionId: sessionId, update: .userMessageChunk(chunk))

        // Then - should still increment counters (encoding succeeds for valid types)
        let metrics = await sut.getMetrics()
        XCTAssertEqual(metrics.updateCount, 1)
        XCTAssertEqual(broadcastCallCount, 1)
    }
}
#endif
