import XCTest
@testable import OpenAgents

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
}
