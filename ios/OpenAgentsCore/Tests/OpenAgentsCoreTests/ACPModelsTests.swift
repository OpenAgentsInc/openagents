import XCTest
@testable import OpenAgentsCore

final class ACPModelsTests: XCTestCase {
    func testMessageTextRoundtrip() throws {
        let msg = ACPMessage(
            id: "m1",
            thread_id: "t1",
            role: .user,
            parts: [.text(ACPText(text: "Hello world"))],
            ts: 1_700_000_000_000
        )
        let ev = ACPEvent(id: "e1", ts: msg.ts, message: msg)
        let thread = ACPThread(id: "t1", events: [ev])
        let data = try JSONEncoder().encode(thread)
        let decoded = try JSONDecoder().decode(ACPThread.self, from: data)
        XCTAssertEqual(thread, decoded)
    }

    func testToolCallAndResultRoundtrip() throws {
        let call = ACPToolCall(id: "c1", tool_name: "search", arguments: .object(["q": .string("swift codable"), "limit": .number(3)]), ts: 100)
        let res = ACPToolResult(call_id: "c1", ok: true, result: .array([.string("a"), .string("b")] ), ts: 200)
        let e1 = ACPEvent(id: "e-call", ts: 100, tool_call: call)
        let e2 = ACPEvent(id: "e-res", ts: 200, tool_result: res)
        let thread = ACPThread(id: "t2", events: [e1, e2])
        let data = try JSONEncoder().encode(thread)
        let decoded = try JSONDecoder().decode(ACPThread.self, from: data)
        XCTAssertEqual(thread, decoded)
        // Ensure snake_case keys like tool_name are present on the wire
        let jsonObj = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let events = jsonObj?["events"] as? [Any]
        XCTAssertNotNil(events)
        let first = events?.first as? [String: Any]
        XCTAssertEqual(first?["kind"] as? String, "tool_call")
        if let tc = first?["tool_call"] as? [String: Any] {
            XCTAssertNotNil(tc["tool_name"]) // snake_case
        } else {
            XCTFail("tool_call payload missing")
        }
    }

    func testPlanStateEncoding() throws {
        let ps = ACPPlanState(status: .running, summary: "Working", steps: ["fetch", "analyze"], ts: 42)
        let ev = ACPEvent(id: "e3", ts: 42, plan_state: ps)
        let thread = ACPThread(id: "t3", events: [ev])
        let data = try JSONEncoder().encode(thread)
        let decoded = try JSONDecoder().decode(ACPThread.self, from: data)
        XCTAssertEqual(decoded.events.first?.plan_state?.status, .running)
    }

    func testJSONValueCodingPrimitives() throws {
        let obj: JSONValue = .object([
            "s": .string("x"),
            "n": .number(1.5),
            "b": .bool(true),
            "a": .array([.number(1), .null])
        ])
        let data = try JSONEncoder().encode(obj)
        let round = try JSONDecoder().decode(JSONValue.self, from: data)
        XCTAssertEqual(obj, round)
    }
}

