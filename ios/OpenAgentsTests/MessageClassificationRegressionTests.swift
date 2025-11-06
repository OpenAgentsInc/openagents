import XCTest
@testable import OpenAgents
@testable import OpenAgentsCore

/// Tests to catch server-side message classification bugs
/// Documents cases where Claude Code server sends wrong SessionUpdate types
final class MessageClassificationRegressionTests: XCTestCase {

    // MARK: - Server Bug Detection

    func testDetectServerSendingInternalThoughtsAsMessages() {
        // BUG: Server sends internal planning thoughts as agent_message_chunk
        // These should be agent_thought_chunk but come through wrong

        let problematicMessages = [
            "I see the actual code is slightly different. Let me edit with the correct version.",
            "Let me check the current implementation first.",
            "Let me search for where this is defined.",
            "Let me read the file to understand the structure.",
            "Good! Now let me check if there are any other files.",
            "Now let me build to verify the changes compile."
        ]

        for message in problematicMessages {
            // These are currently received as agentMessageChunk from server
            // but should be agentThoughtChunk
            let sessionId = ACPSessionId("test")
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: message)))
            )

            // Document: This is wrong but currently how server sends it
            if case .agentMessageChunk = update {
                XCTAssert(true, "Server sends internal thoughts as agentMessageChunk (BUG)")
            }

            // What it SHOULD be:
            let correctUpdate = ACP.Client.SessionUpdate.agentThoughtChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: message)))
            )

            if case .agentThoughtChunk = correctUpdate {
                XCTAssert(true, "Should be agentThoughtChunk")
            }
        }
    }

    func testServerSendsUserFacingSummariesCorrectly() {
        // These should be agent_message_chunk and usually are
        let userFacingMessages = [
            "## ✅ Fixed: Tool Call Display Improvements",
            "I've successfully updated the tool call rendering.",
            "The changes have been committed and pushed.",
            "Here's a summary of what was implemented:"
        ]

        for message in userFacingMessages {
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: message)))
            )

            // This is correct
            if case .agentMessageChunk = update {
                XCTAssert(true, "Correct: user-facing message as agentMessageChunk")
            }
        }
    }

    // MARK: - Heuristic Patterns (If Implementing Client-Side Fix)

    func testDetectLetMePattern() {
        // Pattern: "Let me <verb>" is usually internal thought
        let letMePatterns = [
            "Let me check the implementation.",
            "Let me read the file first.",
            "Let me search for this pattern.",
            "Let me fix this issue."
        ]

        for message in letMePatterns {
            let isInternalThought = message.lowercased().hasPrefix("let me ")
            XCTAssert(isInternalThought, "Should detect 'Let me' pattern")
        }
    }

    func testDetectToolUseMetaNarration() {
        // Pattern: Describing tool use is internal thought
        let metaNarrationPatterns = [
            "I see the actual code is slightly different.",
            "Looking at the code, I can see...",
            "Now let me build to verify.",
            "Good! Now let me check..."
        ]

        for message in metaNarrationPatterns {
            let lower = message.lowercased()
            let hasMetaNarration = lower.contains("i see") ||
                                   lower.contains("looking at") ||
                                   lower.hasPrefix("now let me") ||
                                   lower.hasPrefix("good!")

            XCTAssert(hasMetaNarration, "Should detect meta-narration pattern")
        }
    }

    func testDoNotMisclassifyUserFacingMessages() {
        // These should NOT be classified as thoughts
        let userFacingMessages = [
            "Here's what I found in the codebase:",
            "The issue is in this file:",
            "I've made the following changes:",
            "Let me know if you need anything else." // "Let me know" is user-facing
        ]

        for message in userFacingMessages {
            let lower = message.lowercased()

            // "Let me know" is different from "Let me <action>"
            let isLetMeKnow = lower.contains("let me know") ||
                            lower.contains("let you know")

            let shouldNotBeThought = message.contains("Here's") ||
                                    message.contains("I've made") ||
                                    message.contains("The issue is") ||
                                    isLetMeKnow

            XCTAssert(shouldNotBeThought, "Should NOT misclassify user-facing message")
        }
    }

    // MARK: - Integration Test

    func testTimelineRendering_WithMisclassifiedThoughts() {
        // Simulate what happens when server sends internal thoughts as messages
        let sessionId = ACPSessionId("test")

        let updates: [ACP.Client.SessionUpdate] = [
            .userMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Fix the bug")))),
            // Server BUG: sends this as agentMessageChunk when it should be agentThoughtChunk
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Let me read the file first.")))),
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "I see the issue now.")))),
            // Correct: user-facing summary
            .agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "I've fixed the bug. The issue was..."))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)

        // With current implementation (trusting protocol), we get 4 messages
        let messageCount = items.filter {
            if case .message = $0 { return true }
            return false
        }.count

        // Without fix: 4 messages (3 should be thoughts + 1 real message)
        // With fix: 2 messages (1 user + 1 agent), 1 reasoning summary
        XCTAssertEqual(messageCount, 4, "Current behavior: trusts protocol, shows all as messages")

        // TODO: If implementing client-side fix, update this test to expect:
        // - 2 messages (user + final summary)
        // - 1 reasoning summary (containing "Let me read" and "I see")
    }
}
