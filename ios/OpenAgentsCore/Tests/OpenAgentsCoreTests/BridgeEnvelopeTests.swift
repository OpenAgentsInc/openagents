import XCTest
@testable import OpenAgentsCore

final class BridgeEnvelopeTests: XCTestCase {
    func testThreadsListRoundTrip() throws {
        let items = [ThreadSummary(id: "t1", title: "Hello", source: "codex", created_at: nil, updated_at: 1, last_message_ts: 1, message_count: nil)]
        let resp = WebSocketMessage.ThreadsListResponse(items: items)
        let env = try WebSocketMessage.Envelope.envelope(for: resp, type: "threads.list.response")
        let json = try env.jsonString()
        let back = try WebSocketMessage.Envelope.from(jsonString: json)
        XCTAssertEqual(back.type, "threads.list.response")
        let decoded = try back.decodedMessage(as: WebSocketMessage.ThreadsListResponse.self)
        XCTAssertEqual(decoded.items.first?.id, "t1")
    }
}

