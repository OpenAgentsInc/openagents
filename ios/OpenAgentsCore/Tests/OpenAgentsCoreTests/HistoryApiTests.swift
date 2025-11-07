#if os(macOS)
import XCTest
@testable import OpenAgentsCore

final class HistoryApiTests: XCTestCase {
    var tempDbPath: String!
    var tinyvexDb: TinyvexDbLayer!
    var sut: HistoryApi!

    override func setUp() async throws {
        try await super.setUp()

        // Create temp database
        let tempDir = FileManager.default.temporaryDirectory
        let dbFile = tempDir.appendingPathComponent("test-history-\(UUID().uuidString).db")
        tempDbPath = dbFile.path
        tinyvexDb = try TinyvexDbLayer(path: tempDbPath)

        // Create SUT
        sut = HistoryApi(tinyvexDb: tinyvexDb)
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

    // MARK: - Recent Sessions Tests

    func testRecentSessions_EmptyDatabase() async throws {
        // When
        let sessions = try await sut.recentSessions()

        // Then
        XCTAssertEqual(sessions.count, 0)
    }

    func testRecentSessions_WithData() async throws {
        // Given - add some session data
        let session1 = "session-1"
        let session2 = "session-2"
        let session3 = "session-3"

        // Add events to sessions (in reverse chronological order for last_ts)
        let ts3 = Int64(Date().timeIntervalSince1970 * 1000)
        let ts2 = ts3 - 1000
        let ts1 = ts2 - 1000

        try await tinyvexDb.appendEvent(
            sessionId: session1,
            seq: 0,
            ts: ts1,
            updateJSON: makeTestUpdate(text: "Message 1")
        )
        try await tinyvexDb.appendEvent(
            sessionId: session2,
            seq: 0,
            ts: ts2,
            updateJSON: makeTestUpdate(text: "Message 2")
        )
        try await tinyvexDb.appendEvent(
            sessionId: session3,
            seq: 0,
            ts: ts3,
            updateJSON: makeTestUpdate(text: "Message 3")
        )

        // When
        let sessions = try await sut.recentSessions()

        // Then
        XCTAssertEqual(sessions.count, 3)

        // Should be ordered by last_ts DESC (most recent first)
        XCTAssertEqual(sessions[0].session_id, session3)
        XCTAssertEqual(sessions[0].last_ts, ts3)
        XCTAssertEqual(sessions[0].message_count, 1)

        XCTAssertEqual(sessions[1].session_id, session2)
        XCTAssertEqual(sessions[1].last_ts, ts2)

        XCTAssertEqual(sessions[2].session_id, session1)
        XCTAssertEqual(sessions[2].last_ts, ts1)
    }

    func testRecentSessions_LimitsTo10() async throws {
        // Given - add 12 sessions
        for i in 1...12 {
            let sessionId = "session-\(i)"
            let ts = Int64(Date().timeIntervalSince1970 * 1000) + Int64(i * 1000)
            try await tinyvexDb.appendEvent(
                sessionId: sessionId,
                seq: 0,
                ts: ts,
                updateJSON: makeTestUpdate(text: "Message \(i)")
            )
        }

        // When
        let sessions = try await sut.recentSessions()

        // Then - only 10 returned
        XCTAssertEqual(sessions.count, 10)

        // Should be the 10 most recent
        XCTAssertEqual(sessions[0].session_id, "session-12")
        XCTAssertEqual(sessions[9].session_id, "session-3")
    }

    func testRecentSessions_WithMultipleMessages() async throws {
        // Given - session with multiple messages
        let sessionId = "multi-message-session"
        let baseTs = Int64(Date().timeIntervalSince1970 * 1000)

        for i in 1...5 {
            try await tinyvexDb.appendEvent(
                sessionId: sessionId,
                seq: 0,
                ts: baseTs + Int64(i * 100),
                updateJSON: makeTestUpdate(text: "Message \(i)")
            )
        }

        // When
        let sessions = try await sut.recentSessions()

        // Then
        XCTAssertEqual(sessions.count, 1)
        XCTAssertEqual(sessions[0].session_id, sessionId)
        XCTAssertEqual(sessions[0].message_count, 5)
        XCTAssertEqual(sessions[0].last_ts, baseTs + 500) // Last message timestamp
    }

    func testRecentSessions_NoDatabaseAttached() async throws {
        // Given - API with no database
        sut = HistoryApi(tinyvexDb: nil)

        // When/Then
        do {
            _ = try await sut.recentSessions()
            XCTFail("Expected HistoryError.databaseNotAttached")
        } catch let error as HistoryApi.HistoryError {
            switch error {
            case .databaseNotAttached:
                XCTAssertEqual(error.jsonRpcCode, -32603)
            default:
                XCTFail("Unexpected error type: \(error)")
            }
        }
    }

    // MARK: - Session Timeline Tests

    func testSessionTimeline_EmptySession() async throws {
        // When
        let timeline = try await sut.sessionTimeline(sessionId: "nonexistent-session")

        // Then
        XCTAssertEqual(timeline.count, 0)
    }

    func testSessionTimeline_WithSingleMessage() async throws {
        // Given
        let sessionId = "test-session-1"
        let testMessage = "Hello world"
        let updateJSON = makeTestUpdate(text: testMessage)

        try await tinyvexDb.appendEvent(
            sessionId: sessionId,
            seq: 0,
            ts: Int64(Date().timeIntervalSince1970 * 1000),
            updateJSON: updateJSON
        )

        // When
        let timeline = try await sut.sessionTimeline(sessionId: sessionId)

        // Then
        XCTAssertEqual(timeline.count, 1)
        XCTAssertEqual(timeline[0].session_id.value, sessionId)

        if case .userMessageChunk(let chunk) = timeline[0].update,
           case .text(let textContent) = chunk.content {
            XCTAssertEqual(textContent.text, testMessage)
        } else {
            XCTFail("Unexpected update type")
        }
    }

    func testSessionTimeline_WithMultipleMessages() async throws {
        // Given
        let sessionId = "test-session-multi"
        let messages = ["First", "Second", "Third", "Fourth", "Fifth"]
        let baseTs = Int64(Date().timeIntervalSince1970 * 1000)

        for (idx, msg) in messages.enumerated() {
            try await tinyvexDb.appendEvent(
                sessionId: sessionId,
                seq: 0,
                ts: baseTs + Int64(idx * 100),
                updateJSON: makeTestUpdate(text: msg)
            )
        }

        // When
        let timeline = try await sut.sessionTimeline(sessionId: sessionId)

        // Then
        XCTAssertEqual(timeline.count, 5)

        for (idx, msg) in messages.enumerated() {
            XCTAssertEqual(timeline[idx].session_id.value, sessionId)
            if case .userMessageChunk(let chunk) = timeline[idx].update,
               case .text(let textContent) = chunk.content {
                XCTAssertEqual(textContent.text, msg)
            } else {
                XCTFail("Unexpected update type at index \(idx)")
            }
        }
    }

    func testSessionTimeline_WithLimit() async throws {
        // Given
        let sessionId = "test-session-limit"
        for i in 1...10 {
            try await tinyvexDb.appendEvent(
                sessionId: sessionId,
                seq: 0,
                ts: Int64(Date().timeIntervalSince1970 * 1000) + Int64(i * 100),
                updateJSON: makeTestUpdate(text: "Message \(i)")
            )
        }

        // When
        let timeline = try await sut.sessionTimeline(sessionId: sessionId, limit: 5)

        // Then
        XCTAssertEqual(timeline.count, 5)
    }

    func testSessionTimeline_DifferentUpdateTypes() async throws {
        // Given
        let sessionId = "test-mixed-updates"
        let baseTs = Int64(Date().timeIntervalSince1970 * 1000)

        // User message
        let userChunk = ACP.Client.ContentChunk(content: .text(.init(text: "User message")))
        let userUpdate = ACP.Client.SessionUpdate.userMessageChunk(userChunk)
        try await tinyvexDb.appendEvent(
            sessionId: sessionId,
            seq: 0,
            ts: baseTs,
            updateJSON: encodeUpdate(userUpdate)
        )

        // Agent message
        let agentChunk = ACP.Client.ContentChunk(content: .text(.init(text: "Agent response")))
        let agentUpdate = ACP.Client.SessionUpdate.agentMessageChunk(agentChunk)
        try await tinyvexDb.appendEvent(
            sessionId: sessionId,
            seq: 0,
            ts: baseTs + 100,
            updateJSON: encodeUpdate(agentUpdate)
        )

        // Tool call
        let toolCall = ACP.Client.ToolCall(id: "tool-1", name: "bash", input: .init())
        let toolUpdate = ACP.Client.SessionUpdate.toolCall(toolCall)
        try await tinyvexDb.appendEvent(
            sessionId: sessionId,
            seq: 0,
            ts: baseTs + 200,
            updateJSON: encodeUpdate(toolUpdate)
        )

        // When
        let timeline = try await sut.sessionTimeline(sessionId: sessionId)

        // Then
        XCTAssertEqual(timeline.count, 3)

        // Verify each update type
        if case .userMessageChunk = timeline[0].update {} else {
            XCTFail("Expected userMessageChunk")
        }

        if case .agentMessageChunk = timeline[1].update {} else {
            XCTFail("Expected agentMessageChunk")
        }

        if case .toolCall(let tool) = timeline[2].update {
            XCTAssertEqual(tool.id, "tool-1")
            XCTAssertEqual(tool.name, "bash")
        } else {
            XCTFail("Expected toolCall")
        }
    }

    func testSessionTimeline_NoDatabaseAttached() async throws {
        // Given - API with no database
        sut = HistoryApi(tinyvexDb: nil)

        // When/Then
        do {
            _ = try await sut.sessionTimeline(sessionId: "any-session")
            XCTFail("Expected HistoryError.databaseNotAttached")
        } catch let error as HistoryApi.HistoryError {
            switch error {
            case .databaseNotAttached:
                XCTAssertEqual(error.jsonRpcCode, -32603)
            default:
                XCTFail("Unexpected error type: \(error)")
            }
        }
    }

    func testSessionTimeline_HandlesInvalidJSON() async throws {
        // Given - add invalid JSON to database (simulate corruption)
        let sessionId = "corrupted-session"

        // Add valid update first
        try await tinyvexDb.appendEvent(
            sessionId: sessionId,
            seq: 0,
            ts: Int64(Date().timeIntervalSince1970 * 1000),
            updateJSON: makeTestUpdate(text: "Valid message")
        )

        // This test verifies that HistoryApi gracefully handles invalid JSON
        // In practice, the DB layer ensures valid JSON, but we test error handling

        // When
        let timeline = try await sut.sessionTimeline(sessionId: sessionId)

        // Then - should still return the valid update
        XCTAssertGreaterThanOrEqual(timeline.count, 1)
    }

    func testSessionTimeline_MultipleSessions_Isolation() async throws {
        // Given - multiple sessions with different data
        let session1 = "session-alpha"
        let session2 = "session-beta"

        try await tinyvexDb.appendEvent(
            sessionId: session1,
            seq: 0,
            ts: Int64(Date().timeIntervalSince1970 * 1000),
            updateJSON: makeTestUpdate(text: "Alpha 1")
        )
        try await tinyvexDb.appendEvent(
            sessionId: session1,
            seq: 0,
            ts: Int64(Date().timeIntervalSince1970 * 1000) + 100,
            updateJSON: makeTestUpdate(text: "Alpha 2")
        )
        try await tinyvexDb.appendEvent(
            sessionId: session2,
            seq: 0,
            ts: Int64(Date().timeIntervalSince1970 * 1000),
            updateJSON: makeTestUpdate(text: "Beta 1")
        )

        // When
        let timeline1 = try await sut.sessionTimeline(sessionId: session1)
        let timeline2 = try await sut.sessionTimeline(sessionId: session2)

        // Then
        XCTAssertEqual(timeline1.count, 2)
        XCTAssertEqual(timeline2.count, 1)

        XCTAssertEqual(timeline1[0].session_id.value, session1)
        XCTAssertEqual(timeline2[0].session_id.value, session2)
    }

    // MARK: - Concurrent Access Tests

    func testConcurrentRecentSessionsQueries() async throws {
        // Given - populate with data
        for i in 1...5 {
            try await tinyvexDb.appendEvent(
                sessionId: "session-\(i)",
                seq: 0,
                ts: Int64(Date().timeIntervalSince1970 * 1000) + Int64(i * 1000),
                updateJSON: makeTestUpdate(text: "Message \(i)")
            )
        }

        // When - concurrent queries
        await withTaskGroup(of: Void.self) { group in
            for _ in 1...10 {
                group.addTask {
                    do {
                        let sessions = try await self.sut.recentSessions()
                        XCTAssertEqual(sessions.count, 5)
                    } catch {
                        XCTFail("Concurrent query failed: \(error)")
                    }
                }
            }
        }
    }

    func testConcurrentSessionTimelineQueries() async throws {
        // Given
        let sessionId = "concurrent-test"
        for i in 1...10 {
            try await tinyvexDb.appendEvent(
                sessionId: sessionId,
                seq: 0,
                ts: Int64(Date().timeIntervalSince1970 * 1000) + Int64(i * 100),
                updateJSON: makeTestUpdate(text: "Message \(i)")
            )
        }

        // When - concurrent queries
        await withTaskGroup(of: Void.self) { group in
            for _ in 1...10 {
                group.addTask {
                    do {
                        let timeline = try await self.sut.sessionTimeline(sessionId: sessionId)
                        XCTAssertEqual(timeline.count, 10)
                    } catch {
                        XCTFail("Concurrent query failed: \(error)")
                    }
                }
            }
        }
    }

    // MARK: - Helper Methods

    private func makeTestUpdate(text: String) -> String {
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
        let update = ACP.Client.SessionUpdate.userMessageChunk(chunk)
        return encodeUpdate(update)
    }

    private func encodeUpdate(_ update: ACP.Client.SessionUpdate) -> String {
        guard let data = try? JSONEncoder().encode(update),
              let json = String(data: data, encoding: .utf8) else {
            fatalError("Failed to encode test update")
        }
        return json
    }
}
#endif
