import XCTest
@testable import OpenAgentsCore

final class ACPTypesTests: XCTestCase {
    func testContentBlock_text_decode_encode() throws {
        let json = """
        {"type":"text","text":"Hello","annotations":null}
        """.data(using: .utf8)!
        let block = try JSONDecoder().decode(ACP.Client.ContentBlock.self, from: json)
        switch block {
        case .text(let t):
            XCTAssertEqual(t.text, "Hello")
        default:
            XCTFail("wrong variant")
        }
        let data = try JSONEncoder().encode(block)
        let s = String(data: data, encoding: .utf8)!
        XCTAssertTrue(s.contains("\"type\":\"text\""))
        XCTAssertTrue(s.contains("\"text\":\"Hello\""))
    }

    func testContentBlock_image_decode() throws {
        let json = """
        {"type":"image","data":"iVBOR...","mimeType":"image/png","uri":"file:///tmp/x.png"}
        """.data(using: .utf8)!
        let block = try JSONDecoder().decode(ACP.Client.ContentBlock.self, from: json)
        if case let .image(img) = block {
            XCTAssertEqual(img.mimeType, "image/png")
            XCTAssertEqual(img.uri, "file:///tmp/x.png")
        } else { XCTFail("wrong variant") }
    }

    func testContentBlock_resourceLink_decode() throws {
        let json = """
        {"type":"resource_link","title":"Spec","uri":"https://example.com/spec","mimeType":"text/html"}
        """.data(using: .utf8)!
        let block = try JSONDecoder().decode(ACP.Client.ContentBlock.self, from: json)
        if case let .resource_link(link) = block {
            XCTAssertEqual(link.title, "Spec")
            XCTAssertEqual(link.uri, "https://example.com/spec")
            XCTAssertEqual(link.mimeType, "text/html")
        } else { XCTFail("wrong variant") }
    }

    func testContentBlock_embeddedResource_text_decode() throws {
        let json = """
        {"type":"resource","resource":{"mimeType":"text/plain","text":"hi"}}
        """.data(using: .utf8)!
        let block = try JSONDecoder().decode(ACP.Client.ContentBlock.self, from: json)
        if case let .resource(res) = block {
            if case let .textResource(t) = res.resource {
                XCTAssertEqual(t.mimeType, "text/plain")
                XCTAssertEqual(t.text, "hi")
            } else { XCTFail("wrong inner variant") }
        } else { XCTFail("wrong variant") }
    }

    func testSessionNotification_userChunk_decode() throws {
        let json = """
        {"session_id":"s1","update":{"sessionUpdate":"user_message_chunk","content":{"type":"text","text":"Go"}}}
        """.data(using: .utf8)!
        let note = try JSONDecoder().decode(ACP.Client.SessionNotificationWire.self, from: json)
        XCTAssertEqual(note.session_id.value, "s1")
        if case let .userMessageChunk(chunk) = note.update {
            if case let .text(t) = chunk.content { XCTAssertEqual(t.text, "Go") } else { XCTFail("bad content") }
        } else { XCTFail("wrong update") }
    }

    func testJSONRPC_initialize_roundtrip() throws {
        let req = JSONRPC.Request(id: JSONRPC.ID("1"), method: ACPRPC.initialize, params: ACP.Agent.InitializeRequest(protocol_version: "0.7.0", client_capabilities: .init(), client_info: .init(name: "test", version: "0.0.1")))
        let data = try JSONEncoder().encode(req)
        let text = String(data: data, encoding: .utf8)!
        XCTAssertTrue(text.contains("\"jsonrpc\":\"2.0\""))
        XCTAssertTrue(text.contains("\"method\":\"initialize\""))

        let resp = JSONRPC.Response(id: JSONRPC.ID("1"), result: ACP.Agent.InitializeResponse(protocol_version: "0.7.0", agent_capabilities: .init(), auth_methods: [], agent_info: .init(name: "agent", version: "1.0.0")))
        let rdata = try JSONEncoder().encode(resp)
        let back = try JSONDecoder().decode(JSONRPC.Response<ACP.Agent.InitializeResponse>.self, from: rdata)
        XCTAssertEqual(back.result.protocol_version, "0.7.0")
    }
}
