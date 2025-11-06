import XCTest
@testable import OpenAgentsCore

#if os(macOS)
/// Performance tests for ACP protocol implementation
/// Phase 7: Performance Tests - Large sessions, memory usage, concurrent updates, timeline computation
final class PerformanceTests: XCTestCase {

    // MARK: - Large Session Loading Tests

    func testPerformance_LargeSessionLoading_1000Messages() throws {
        let sessionId = ACPSessionId("perf-large-session")

        // Create 1000 messages
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<1000 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i): This is a test message with some content to simulate realistic message sizes.")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Measure timeline computation time
        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 1000)
        }

        // Should complete in reasonable time (measured by XCTest)
        // Typical expectation: < 0.1s for 1000 messages
    }

    func testPerformance_LargeSessionLoading_5000Messages() throws {
        let sessionId = ACPSessionId("perf-very-large-session")

        // Create 5000 messages (stress test)
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<5000 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Measure performance with large dataset
        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 5000)
        }
    }

    func testPerformance_LargeSessionWithMixedContent() throws {
        let sessionId = ACPSessionId("perf-mixed-content")

        // Create 1000 updates with mixed content types
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<1000 {
            if i % 5 == 0 {
                // Tool use
                updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                    id: ACP.ToolUseId("tool-\(i)"),
                    name: "Bash",
                    arguments: TestHelpers.makeToolArguments(["command": "echo test"])
                )))))
            } else if i % 5 == 1 {
                // Tool result
                updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .toolResult(.init(
                    tool_use_id: ACP.ToolUseId("tool-\(i-1)"),
                    content: [.text(.init(text: "test"))],
                    is_error: false
                )))))
            } else if i % 5 == 2 {
                // Thinking
                updates.append(.agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Thinking about task \(i)")))))
            } else {
                // Text
                updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
            }
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 1000)
        }
    }

    // MARK: - Memory Usage Tests

    func testMemory_LargeMessageAccumulation() throws {
        let sessionId = ACPSessionId("mem-large-accum")

        // Create large text messages
        var updates: [ACP.Client.SessionUpdate] = []
        let largeText = String(repeating: "Lorem ipsum dolor sit amet. ", count: 100) // ~2.8KB per message

        for i in 0..<500 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "\(i): \(largeText)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Compute timeline and check it completes
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 500)

        // Should handle large text without memory issues
        XCTAssertFalse(items.isEmpty, "Should process large text messages")
    }

    func testMemory_RingBufferBehavior() throws {
        let sessionId = ACPSessionId("mem-ring-buffer")

        // Test that ring buffer properly caps memory usage
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<10000 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // With cap of 200, should not retain all 10000 updates
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 200)

        XCTAssertLessThanOrEqual(items.count, 200, "Ring buffer should cap items")
    }

    func testMemory_UpdateDeduplication() throws {
        let sessionId = ACPSessionId("mem-dedup")

        // Create many duplicate updates
        let duplicateUpdate = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Duplicate message")))
        )

        var updates: [ACP.Client.SessionUpdate] = []
        for _ in 0..<1000 {
            updates.append(duplicateUpdate)
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Should deduplicate efficiently
        let (items, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 1000)

        // All duplicates should accumulate into a single message
        XCTAssertEqual(items.count, 1, "Should deduplicate repeated chunks")
    }

    // MARK: - Concurrent Update Performance Tests

    func testPerformance_ConcurrentSessionUpdates() throws {
        // Simulate multiple sessions updating concurrently
        var allWires: [ACP.Client.SessionNotificationWire] = []

        // Create 10 concurrent sessions, each with 100 updates
        for sessionNum in 0..<10 {
            let sessionId = ACPSessionId("concurrent-\(sessionNum)")
            var updates: [ACP.Client.SessionUpdate] = []

            for i in 0..<100 {
                updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Session \(sessionNum) Message \(i)")))))
            }

            let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }
            allWires.append(contentsOf: wires)
        }

        // Shuffle to simulate concurrent arrival
        allWires.shuffle()

        // Measure performance processing concurrent updates
        measure {
            // Process updates for each session
            for sessionNum in 0..<10 {
                let sessionId = ACPSessionId("concurrent-\(sessionNum)")
                let sessionWires = allWires.filter { $0.session_id == sessionId }
                let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: sessionWires, cap: 100)
            }
        }
    }

    func testPerformance_RapidUpdateStream() throws {
        let sessionId = ACPSessionId("rapid-stream")

        // Simulate rapid stream of small updates
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<2000 {
            // Very small chunks to simulate streaming
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: String(i % 10))))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 2000)
        }
    }

    // MARK: - Timeline Computation Performance Tests

    func testPerformance_TimelineRecomputation() throws {
        let sessionId = ACPSessionId("timeline-recomp")

        // Create initial set of updates
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<500 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Message \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Measure repeated recomputation (simulating UI updates)
        measure {
            for _ in 0..<10 {
                let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 500)
            }
        }
    }

    func testPerformance_IncrementalTimelineUpdate() throws {
        let sessionId = ACPSessionId("incremental-timeline")

        // Start with 100 messages
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<100 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Initial \(i)")))))
        }

        var wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        // Measure incremental additions
        measure {
            // Add 100 more messages incrementally
            for i in 100..<200 {
                let newUpdate = ACP.Client.SessionUpdate.agentMessageChunk(
                    ACP.Client.ContentChunk(content: .text(.init(text: "New \(i)")))
                )
                wires.append(ACP.Client.SessionNotificationWire(session_id: sessionId, update: newUpdate))

                // Recompute after each addition
                let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 200)
            }
        }
    }

    // MARK: - Rendering Performance Tests

    func testPerformance_ToolCallParsing() throws {
        let sessionId = ACPSessionId("tool-parsing")

        // Create 500 tool calls with complex arguments
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<500 {
            let complexArgs = TestHelpers.makeToolArguments([
                "command": "bash -lc \"cd /tmp && ls -la | grep test\"",
                "cwd": "/Users/test/project",
                "env": "PATH=/usr/local/bin:/usr/bin"
            ])

            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("tool-\(i)"),
                name: "Bash",
                arguments: complexArgs
            )))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 500)
        }
    }

    func testPerformance_ThinkingConsolidation() throws {
        let sessionId = ACPSessionId("thinking-consolidation")

        // Create 1000 thinking chunks to consolidate
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<1000 {
            updates.append(.agentThoughtChunk(ACP.Client.ContentChunk(content: .thinking(.init(thinking: "Thought \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 1000)
        }
    }

    // MARK: - JSON Encoding/Decoding Performance Tests

    func testPerformance_JSONEncodingUpdates() throws {
        let sessionId = ACPSessionId("json-encode")

        // Create 500 updates with various content types
        var updates: [ACP.Client.SessionUpdate] = []
        for i in 0..<500 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "Encoded message \(i)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        let encoder = JSONEncoder()

        measure {
            for wire in wires {
                _ = try? encoder.encode(wire)
            }
        }
    }

    func testPerformance_JSONDecodingUpdates() throws {
        let sessionId = ACPSessionId("json-decode")

        // Create and encode 500 updates
        var encodedWires: [Data] = []
        let encoder = JSONEncoder()

        for i in 0..<500 {
            let update = ACP.Client.SessionUpdate.agentMessageChunk(
                ACP.Client.ContentChunk(content: .text(.init(text: "Decoded message \(i)")))
            )
            let wire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

            if let data = try? encoder.encode(wire) {
                encodedWires.append(data)
            }
        }

        let decoder = JSONDecoder()

        measure {
            for data in encodedWires {
                _ = try? decoder.decode(ACP.Client.SessionNotificationWire.self, from: data)
            }
        }
    }

    // MARK: - Edge Case Performance Tests

    func testPerformance_DeepNestedJSON() throws {
        // Test performance with deeply nested JSON structures
        let sessionId = ACPSessionId("deep-nested")

        // Create tool arguments with deeply nested structure
        var nestedDict: [String: Any] = ["value": "bottom"]
        for i in 0..<50 {
            nestedDict = ["level_\(i)": nestedDict]
        }

        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("nested-tool"),
                name: "Test",
                arguments: TestHelpers.makeToolArguments(nestedDict)
            ))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)
        }
    }

    func testPerformance_LargeArrayInToolArguments() throws {
        let sessionId = ACPSessionId("large-array")

        // Create tool arguments with large arrays
        var largeArray: [String] = []
        for i in 0..<1000 {
            largeArray.append("item_\(i)")
        }

        let updates: [ACP.Client.SessionUpdate] = [
            .agentMessageChunk(ACP.Client.ContentChunk(content: .toolUse(.init(
                id: ACP.ToolUseId("array-tool"),
                name: "Process",
                arguments: TestHelpers.makeToolArguments(["items": largeArray])
            ))))
        ]

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 100)
        }
    }

    func testPerformance_UnicodeHeavyContent() throws {
        let sessionId = ACPSessionId("unicode-heavy")

        // Create updates with heavy unicode content (emoji, special chars)
        var updates: [ACP.Client.SessionUpdate] = []
        let unicodeText = "🚀🎉✨💻🔥📱🌟⚡️🎯🏆" * 50 // 500 emoji characters

        for i in 0..<200 {
            updates.append(.agentMessageChunk(ACP.Client.ContentChunk(content: .text(.init(text: "\(i): \(unicodeText)")))))
        }

        let wires = updates.map { ACP.Client.SessionNotificationWire(session_id: sessionId, update: $0) }

        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: wires, cap: 200)
        }
    }

    // MARK: - Baseline Performance Tests

    func testPerformance_Baseline_EmptySession() throws {
        let sessionId = ACPSessionId("empty")

        // Measure baseline with no updates
        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: [], cap: 100)
        }
    }

    func testPerformance_Baseline_SingleMessage() throws {
        let sessionId = ACPSessionId("single")

        let update = ACP.Client.SessionUpdate.agentMessageChunk(
            ACP.Client.ContentChunk(content: .text(.init(text: "Single message")))
        )
        let wire = ACP.Client.SessionNotificationWire(session_id: sessionId, update: update)

        // Measure baseline with single update
        measure {
            let (_, _) = AcpThreadView_computeTimelineFromUpdates(updates: [wire], cap: 100)
        }
    }
}
#endif
