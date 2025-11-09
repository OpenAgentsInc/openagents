import XCTest
@testable import OpenAgents
import OpenAgentsCore

final class ExportFormattingTests: XCTestCase {
    func testMarkdownContainsUserAssistant() throws {
        let sid = ACPSessionId("S1")
        let u1 = ACP.Client.SessionNotificationWire(session_id: sid, update: .userMessageChunk(.init(content: .text(.init(text: "Hello")))), _meta: nil)
        let a1 = ACP.Client.SessionNotificationWire(session_id: sid, update: .agentMessageChunk(.init(content: .text(.init(text: "Hi there")))), _meta: nil)
        let md = TranscriptExport.exportMarkdown(updates: [u1, a1])
        XCTAssertTrue(md.contains("**User**"))
        XCTAssertTrue(md.contains("Hello"))
        XCTAssertTrue(md.contains("**Assistant**"))
        XCTAssertTrue(md.contains("Hi there"))
    }

    func testJSONEncodes() throws {
        let sid = ACPSessionId("S1")
        let u1 = ACP.Client.SessionNotificationWire(session_id: sid, update: .userMessageChunk(.init(content: .text(.init(text: "Hello")))), _meta: nil)
        let data = try TranscriptExport.exportJSONData(updates: [u1])
        let arr = try JSONDecoder().decode([ACP.Client.SessionNotificationWire].self, from: data)
        XCTAssertEqual(arr.count, 1)
        XCTAssertEqual(arr.first?.session_id.value, "S1")
    }
}

