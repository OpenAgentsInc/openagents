import XCTest
@testable import OpenAgentsCore

final class TranslatorTests: XCTestCase {
    func testTranslateToolCallAndResult() throws {
        let lines: [String] = [
            // session meta (optional)
            #"{"type":"session_meta","payload":{"id":"t-1","title":"Demo"}}"#,
            // user message
            #"{"type":"user_message","item":{"type":"user_message","text":"search for swift"},"ts": 10}"#,
            // tool call
            #"{"type":"tool_call","item":{"type":"tool_call","id":"c1","tool_name":"search","arguments":{"q":"swift","limit":3}},"ts": 11}"#,
            // tool result
            #"{"type":"tool_result","item":{"type":"tool_result","call_id":"c1","ok":true,"result":["a","b"]},"ts": 12}"#
        ]

        let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: "test"))
        XCTAssertEqual(thread.id.isEmpty, false)
        XCTAssertEqual(thread.events.count, 3 + 1) // user message + tool call + tool result + session meta ignored for events

        let kinds = thread.events.map { $0.kind }
        XCTAssertTrue(kinds.contains(.message))
        XCTAssertTrue(kinds.contains(.tool_call))
        XCTAssertTrue(kinds.contains(.tool_result))

        let tc = thread.events.compactMap { $0.tool_call }.first
        XCTAssertEqual(tc?.tool_name, "search")
        if case let .object(obj)? = tc?.arguments {
            XCTAssertEqual(obj["q"], .string("swift"))
        } else {
            XCTFail("Expected object arguments")
        }

        let tr = thread.events.compactMap { $0.tool_result }.first
        XCTAssertEqual(tr?.call_id, "c1")
        XCTAssertEqual(tr?.ok, true)
    }

    func testTranslatePlanState() throws {
        let lines: [String] = [
            #"{"type":"plan_state","item":{"type":"plan_state","status":"running","summary":"Working","steps":["fetch","analyze"]},"ts": 20}"#
        ]
        let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: "plan"))
        let ps = thread.events.compactMap { $0.plan_state }.first
        XCTAssertNotNil(ps)
        XCTAssertEqual(ps?.status, .running)
        XCTAssertEqual(ps?.summary, "Working")
        XCTAssertEqual(ps?.steps ?? [], ["fetch","analyze"])
    }
}

