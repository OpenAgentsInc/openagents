import XCTest
@testable import OpenAgentsCore

final class SessionUpdateTests: XCTestCase {
    func testEncodeDecode_availableCommandsUpdate() throws {
        let ac = ACP.Client.AvailableCommandsUpdate(available_commands: [
            .init(name: "create_plan", description: "Propose a plan", input: .unstructured(hint: "topic"))
        ])
        let upd = ACP.Client.SessionUpdate.availableCommandsUpdate(ac)
        let data = try JSONEncoder().encode(upd)
        let s = String(data: data, encoding: .utf8)!
        XCTAssertTrue(s.contains("\"sessionUpdate\":\"available_commands_update\""))
        let back = try JSONDecoder().decode(ACP.Client.SessionUpdate.self, from: data)
        if case let .availableCommandsUpdate(v) = back {
            XCTAssertEqual(v.available_commands.first?.name, "create_plan")
        } else { XCTFail("wrong update") }
    }

    func testEncodeDecode_toolCallUpdate() throws {
        let tcu = ACPToolCallUpdateWire(call_id: "c1", status: .completed, output: AnyEncodable(["ok": true]), error: nil)
        let upd = ACP.Client.SessionUpdate.toolCallUpdate(tcu)
        let data = try JSONEncoder().encode(upd)
        let back = try JSONDecoder().decode(ACP.Client.SessionUpdate.self, from: data)
        if case let .toolCallUpdate(v) = back {
            XCTAssertEqual(v.call_id, "c1")
            XCTAssertEqual(v.status, .completed)
        } else { XCTFail("wrong update") }
    }
}

