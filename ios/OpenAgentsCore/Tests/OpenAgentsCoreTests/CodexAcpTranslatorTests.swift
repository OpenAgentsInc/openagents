import XCTest
@testable import OpenAgentsCore

final class CodexAcpTranslatorTests: XCTestCase {
    func testTranslateBasicTimeline() throws {
        // Trimmed Codex-like JSONL
        let lines = [
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"t-123\",\"title\":\"Build feature\",\"created_at\":1000}}",
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"user_message\",\"text\":\"Hi\",\"ts\":1100},\"ts\":1100}",
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"Hello\",\"ts\":1200},\"ts\":1200}",
            "{\"type\":\"tool_call\",\"item\":{\"type\":\"tool_call\",\"id\":\"c1\",\"tool_name\":\"search\",\"arguments\":{\"q\":\"swift\"},\"ts\":1300},\"ts\":1300}",
            "{\"type\":\"tool_result\",\"item\":{\"type\":\"tool_result\",\"call_id\":\"c1\",\"ok\":true,\"result\":[\"a\",\"b\"],\"ts\":1400},\"ts\":1400}",
            "{\"type\":\"plan_state\",\"item\":{\"type\":\"plan_state\",\"status\":\"running\",\"summary\":\"Working\",\"steps\":[\"s1\",\"s2\"],\"ts\":1500},\"ts\":1500}"
        ]
        let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: "fixture.jsonl"))
        XCTAssertEqual(thread.id, "t-123")
        XCTAssertEqual(thread.title, "Build feature")
        XCTAssertEqual(thread.created_at, 1000)
        XCTAssertEqual(thread.updated_at, 1500)
        XCTAssertEqual(thread.events.count, 5)

        // Order by ts asc
        let times = thread.events.map { $0.ts }
        XCTAssertEqual(times, [1100, 1200, 1300, 1400, 1500])

        // Roles and kinds
        XCTAssertEqual(thread.events[0].kind, .message)
        XCTAssertEqual(thread.events[0].message?.role, .user)
        XCTAssertEqual(thread.events[1].message?.role, .assistant)
        XCTAssertEqual(thread.events[2].kind, .tool_call)
        XCTAssertEqual(thread.events[2].tool_call?.tool_name, "search")
        if case let .object(args)? = thread.events[2].tool_call?.arguments {
            XCTAssertEqual(args["q"], .string("swift"))
        } else { XCTFail("missing args") }
        XCTAssertEqual(thread.events[3].kind, .tool_result)
        XCTAssertEqual(thread.events[3].tool_result?.call_id, "c1")
        XCTAssertEqual(thread.events[4].plan_state?.status, .running)
    }

    func testTranslateWithoutSessionMetaGeneratesStableThreadId() throws {
        let lines = [
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"user_message\",\"text\":\"X\",\"ts\":1},\"ts\":1}"
        ]
        let t = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: "no-meta.jsonl"))
        XCTAssertFalse(t.id.isEmpty)
        XCTAssertEqual(t.events.count, 1)
        XCTAssertEqual(t.events[0].message?.role, .user)
    }
}

