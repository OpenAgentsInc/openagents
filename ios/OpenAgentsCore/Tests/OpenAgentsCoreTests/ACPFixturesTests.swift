import XCTest
@testable import OpenAgentsCore

final class ACPFixturesTests: XCTestCase {
    private func fixturesDir() -> URL {
        // Compute path relative to this test file location
        // This file: ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/ACPFixturesTests.swift
        // Fixtures:  ios/OpenAgentsCore/Tests/Fixtures/acp/
        let fileURL = URL(fileURLWithPath: #file)
        let testsDir = fileURL.deletingLastPathComponent().deletingLastPathComponent()
        return testsDir.appendingPathComponent("Fixtures/acp", isDirectory: true)
    }

    private func loadFixture(_ name: String) throws -> Data {
        let url = fixturesDir().appendingPathComponent(name)
        return try Data(contentsOf: url)
    }

    private func jsonObject(_ data: Data) throws -> Any {
        return try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed, .topLevelDictionaryAssumed])
    }

    private func assertRoundTripEqual<T: Codable>(_ type: T.Type, data: Data, file: StaticString = #file, line: UInt = #line) throws {
        // Decode -> Encode -> Compare JSON dictionaries for structural equality
        let original = try jsonObject(data)
        let decoded = try JSONDecoder().decode(T.self, from: data)
        let reencoded = try JSONEncoder().encode(decoded)
        let rt = try jsonObject(reencoded)
        XCTAssertTrue(NSDictionary(dictionary: original as! [String: Any]).isEqual(to: rt as! [String: Any]), "Round-trip JSON mismatch", file: file, line: line)
    }

    // MARK: - Initialize
    func test_initialize_request_fixture() throws {
        let data = try loadFixture("initialize_request.json")
        try assertRoundTripEqual(JSONRPC.Request<ACP.Agent.InitializeRequest>.self, data: data)
    }

    func test_initialize_response_fixture() throws {
        let data = try loadFixture("initialize_response.json")
        try assertRoundTripEqual(JSONRPC.Response<ACP.Agent.InitializeResponse>.self, data: data)
    }

    // MARK: - Session lifecycle
    func test_session_new_request_fixture() throws {
        let data = try loadFixture("session_new_request.json")
        try assertRoundTripEqual(JSONRPC.Request<ACP.Agent.SessionNewRequest>.self, data: data)
    }

    func test_session_new_response_fixture() throws {
        let data = try loadFixture("session_new_response.json")
        try assertRoundTripEqual(JSONRPC.Response<ACP.Agent.SessionNewResponse>.self, data: data)
    }

    func test_session_prompt_request_fixture() throws {
        let data = try loadFixture("session_prompt_request.json")
        try assertRoundTripEqual(JSONRPC.Request<ACP.Agent.SessionPromptRequest>.self, data: data)
    }

    func test_session_cancel_notification_fixture() throws {
        let data = try loadFixture("session_cancel_notification.json")
        try assertRoundTripEqual(JSONRPC.Notification<[String: String]>.self, data: data)
    }

    // MARK: - Session Update variants
    private func assertUpdateVariant(_ filename: String, expected kindCheck: (ACP.Client.SessionUpdate) -> Bool) throws {
        let data = try loadFixture(filename)
        let original = try jsonObject(data)
        let note = try JSONDecoder().decode(JSONRPC.Notification<ACP.Client.SessionNotificationWire>.self, from: data)
        XCTAssertTrue(kindCheck(note.params.update), "Unexpected SessionUpdate variant in \(filename)")
        let reencoded = try JSONEncoder().encode(note)
        let rt = try jsonObject(reencoded)
        XCTAssertTrue(NSDictionary(dictionary: original as! [String: Any]).isEqual(to: rt as! [String: Any]))
    }

    func test_session_update_user_message_chunk_fixture() throws {
        try assertUpdateVariant("session_update_user_message_chunk.json") { upd in
            if case .userMessageChunk = upd { return true }; return false
        }
    }

    func test_session_update_agent_message_chunk_fixture() throws {
        try assertUpdateVariant("session_update_agent_message_chunk.json") { upd in
            if case .agentMessageChunk = upd { return true }; return false
        }
    }

    func test_session_update_agent_thought_chunk_fixture() throws {
        try assertUpdateVariant("session_update_agent_thought_chunk.json") { upd in
            if case .agentThoughtChunk = upd { return true }; return false
        }
    }

    func test_session_update_plan_fixture() throws {
        try assertUpdateVariant("session_update_plan.json") { upd in
            if case .plan = upd { return true }; return false
        }
    }

    func test_session_update_available_commands_update_fixture() throws {
        try assertUpdateVariant("session_update_available_commands_update.json") { upd in
            if case .availableCommandsUpdate = upd { return true }; return false
        }
    }

    func test_session_update_current_mode_update_fixture() throws {
        try assertUpdateVariant("session_update_current_mode_update.json") { upd in
            if case .currentModeUpdate = upd { return true }; return false
        }
    }

    func test_session_update_tool_call_fixture() throws {
        try assertUpdateVariant("session_update_tool_call.json") { upd in
            if case .toolCall = upd { return true }; return false
        }
    }

    func test_session_update_tool_call_update_fixture() throws {
        try assertUpdateVariant("session_update_tool_call_update.json") { upd in
            if case .toolCallUpdate = upd { return true }; return false
        }
    }
}

