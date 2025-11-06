import XCTest
@testable import OpenAgents
@testable import OpenAgentsCore

final class TimelineTests: XCTestCase {
    func testReasoningDurationBetweenUserAndAssistant() throws {
        // user at 1000ms, reasoning at 2000ms, assistant at 6000ms → expect 5s
        let user = #"{"item":{"role":"user","type":"message","text":"Q"},"ts":1000}"#
        let think = #"{"type":"event_msg","payload":{"type":"agent_reasoning","text":"**Title**\nthinking..."},"ts":2000}"#
        let assistant = #"{"item":{"role":"assistant","type":"message","text":"A"},"ts":6000}"#
        let (items, _, _) = AcpThreadView_computeTimeline(lines: [user, think, assistant], sourceId: "test", cap: 100)

        // Expect a reasoningSummary followed by a message
        var found = false
        for item in items {
            switch item {
            case .reasoningSummary(let rs):
                let secs = Int((rs.endTs - rs.startTs) / 1000)
                XCTAssertEqual(secs, 5, "Expected 5s, got \(secs)s")
                found = true
            default:
                continue
            }
        }
        XCTAssertTrue(found, "Did not find reasoning summary in timeline")
    }

    func testISOTimeFallbackForZeroTs() throws {
        // user with ISO timestamp, reasoning with ISO, assistant with ISO
        let user = #"{"timestamp":"2025-11-05T07:09:00Z","item":{"role":"user","type":"message","text":"Q"}}"#
        let think = #"{"timestamp":"2025-11-05T07:09:10Z","type":"event_msg","payload":{"type":"agent_reasoning","text":"**Title**\nthinking..."}}"#
        let assistant = #"{"timestamp":"2025-11-05T07:09:20Z","item":{"role":"assistant","type":"message","text":"A"}}"#
        let (items, _, _) = AcpThreadView_computeTimeline(lines: [user, think, assistant], sourceId: "test", cap: 100)
        var secsFound: Int? = nil
        for item in items {
            if case .reasoningSummary(let rs) = item {
                secsFound = Int((rs.endTs - rs.startTs) / 1000)
                break
            }
        }
        XCTAssertEqual(secsFound, 20, "Expected 20s using ISO timestamps fallback")
    }

    // MARK: - Message Type Classification Tests (Regression Prevention)

    func testAgentMessageChunk_AppearsAsRegularMessage_NotThought() throws {
        // CRITICAL: agentMessageChunk must ALWAYS appear as regular message,
        // regardless of content patterns (bullets, markdown, keywords)
        let sessionId = ACPSessionId("test-session")

        let messages = [
            "Let me build iOS and macOS to make sure everything compiles",
            "Good! Now let me build macOS",
            "Perfect! Now let me commit this fix and explain what I did",
            "I'm going to use the Task tool to launch the code-reviewer agent"
        ]

        for messageText in messages {
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: messageText)))
            )
            let wire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

            let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [wire], cap: 100)

            // Should have exactly 1 message item, NOT a reasoning summary
            let messageItems = items.filter {
                if case .message = $0 { return true }
                return false
            }
            let reasoningItems = items.filter {
                if case .reasoningSummary = $0 { return true }
                return false
            }

            XCTAssertEqual(messageItems.count, 1, "agentMessageChunk should appear as regular message: '\(messageText)'")
            XCTAssertEqual(reasoningItems.count, 0, "agentMessageChunk should NOT create reasoning summary: '\(messageText)'")
        }
    }

    func testAgentThoughtChunk_AppearsInReasoningSummary() throws {
        // CRITICAL: agentThoughtChunk must ALWAYS appear in reasoning summary
        let sessionId = ACPSessionId("test-session")

        let thoughtUpdate = ACP.Client.SessionUpdate.agentThoughtChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Internal reasoning about the approach...")))
        )
        let thoughtWire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: thoughtUpdate)

        // Add a message after to flush reasoning
        let messageUpdate = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Here's what I found")))
        )
        let messageWire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: messageUpdate)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [thoughtWire, messageWire], cap: 100)

        // Should have 1 reasoning summary and 1 message
        let reasoningItems = items.filter {
            if case .reasoningSummary = $0 { return true }
            return false
        }
        let messageItems = items.filter {
            if case .message = $0 { return true }
            return false
        }

        XCTAssertEqual(reasoningItems.count, 1, "agentThoughtChunk should create reasoning summary")
        XCTAssertEqual(messageItems.count, 1, "agentMessageChunk should appear as message")
    }

    func testUserFacingSummary_AppearsAsMessage_NotThought() throws {
        // CRITICAL: Comprehensive summaries that are user-facing should appear as messages
        let sessionId = ACPSessionId("test-session")

        let summaryText = """
        ## ✅ Fixed: TodoWrite Now Shows as Plans with Checkboxes

        ### What Was Implemented

        **TodoWrite Conversion:**
        - Automatically converts TodoWrite tool results to plan updates
        - Hides TodoWrite from tool call timeline
        - Maps todo statuses to plan entry statuses

        ### Testing

        Please rebuild the app to see the changes take effect.
        """

        let update = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: summaryText)))
        )
        let wire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [wire], cap: 100)

        let messageItems = items.filter {
            if case .message = $0 { return true }
            return false
        }
        let reasoningItems = items.filter {
            if case .reasoningSummary = $0 { return true }
            return false
        }

        XCTAssertEqual(messageItems.count, 1, "User-facing summary should appear as message")
        XCTAssertEqual(reasoningItems.count, 0, "User-facing summary should NOT be a thought")
    }

    func testMixedMessages_ClassifiedCorrectly() throws {
        // Test a realistic conversation flow with mixed message types
        let sessionId = ACPSessionId("test-session")

        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Fix the bug")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Let me analyze the code...")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Found the issue in line 42")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "I've identified the bug and will fix it now")))),
            .agentThoughtChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Making the fix...")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "✅ Bug fixed! The issue was in the error handler."))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // Should have: 1 user message, 2 reasoning summaries, 2 agent messages
        var userMsgs = 0
        var reasoningSummaries = 0
        var agentMsgs = 0

        for item in items {
            switch item {
            case .message(let msg):
                if msg.role == .user { userMsgs += 1 }
                else if msg.role == .assistant { agentMsgs += 1 }
            case .reasoningSummary:
                reasoningSummaries += 1
            default:
                break
            }
        }

        XCTAssertEqual(userMsgs, 1, "Should have 1 user message")
        XCTAssertEqual(reasoningSummaries, 2, "Should have 2 reasoning summaries (thoughts flushed by messages)")
        XCTAssertEqual(agentMsgs, 2, "Should have 2 agent messages")
    }

    func testBulletedMessage_NotMisclassifiedAsThought() throws {
        // Regression test: Bulleted messages were being misclassified as thoughts
        let sessionId = ACPSessionId("test-session")

        let bulletedMessage = """
        Here's what I completed:
        - Fixed the null arguments bug
        - Added direct toJSONValue() conversion
        - Updated both real-time and history paths
        - Built and tested successfully
        """

        let update = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: bulletedMessage)))
        )
        let wire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [wire], cap: 100)

        let messageItems = items.filter {
            if case .message = $0 { return true }
            return false
        }

        XCTAssertEqual(messageItems.count, 1, "Bulleted message should NOT be classified as thought")
    }

    func testNoHeuristicsApplied_StrictProtocolCompliance() throws {
        // CRITICAL: Verify that NO heuristics are applied during timeline computation
        // The ACP protocol type should be the ONLY factor in classification
        let sessionId = ACPSessionId("test-session")

        // These all have "thought-like" patterns but are agentMessageChunk
        let edgeCases = [
            "Let me think about this approach...",  // Contains "let me" + "think"
            "I'm reasoning through the solution",    // Contains "reasoning"
            "My internal monologue says...",         // Contains "internal monologue"
            "- First, analyze\n- Then, implement\n- Finally, test",  // Bullets
            "1. Step one\n2. Step two\n3. Step three"  // Numbered list
        ]

        for text in edgeCases {
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: text)))
            )
            let wire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

            let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: [wire], cap: 100)

            let messageItems = items.filter {
                if case .message = $0 { return true }
                return false
            }
            let reasoningItems = items.filter {
                if case .reasoningSummary = $0 { return true }
                return false
            }

            XCTAssertEqual(messageItems.count, 1, "agentMessageChunk must be message regardless of content: '\(text)'")
            XCTAssertEqual(reasoningItems.count, 0, "No heuristics should reclassify agentMessageChunk: '\(text)'")
        }
    }
}
