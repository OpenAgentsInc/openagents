import XCTest
@testable import OpenAgentsCore

final class TinyvexDbLayerTests: XCTestCase {
    func testAppendAndHistory() async throws {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("tvx_\(UUID().uuidString).sqlite")
        let db = try TinyvexDbLayer(path: tmp.path)
        // Append one ACP update
        let wire = ACP.Client.SessionNotificationWire(session_id: ACPSessionId("s1"), update: .agentMessageChunk(.init(content: .text(.init(text: "hello")))))
        let updateJSON = String(data: try JSONEncoder().encode(wire.update), encoding: .utf8)!
        try await db.appendEvent(sessionId: "s1", seq: 1, ts: 123, updateJSON: updateJSON)
        let rows = try await db.history(sessionId: "s1", sinceSeq: nil, sinceTs: nil, limit: 10)
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows.first?.seq, 1)
        // Decode to ACP update
        let upd = try JSONDecoder().decode(ACP.Client.SessionUpdate.self, from: Data(rows[0].update_json.utf8))
        if case .agentMessageChunk(let ch) = upd {
            if case .text(let t) = ch.content { XCTAssertEqual(t.text, "hello") }
            else { XCTFail("expected text") }
        } else {
            XCTFail("expected agentMessageChunk")
        }
    }
}

