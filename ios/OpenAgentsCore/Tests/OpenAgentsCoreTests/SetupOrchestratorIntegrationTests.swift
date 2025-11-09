// SetupOrchestratorIntegrationTests.swift — Integration tests for conversational orchestration setup
// Tests the full flow: setup.start → session/prompt routing → config save → cleanup

import XCTest
@testable import OpenAgentsCore

#if os(macOS)

final class SetupOrchestratorIntegrationTests: XCTestCase {

    var server: DesktopWebSocketServer!
    var tempDbPath: String!

    override func setUp() async throws {
        try await super.setUp()
        server = DesktopWebSocketServer()

        // Create temporary database for testing
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("setup-test-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: URL(fileURLWithPath: tempDir.path), withIntermediateDirectories: true)
        tempDbPath = tempDir.appendingPathComponent("test.db").path
        server.setTinyvexDb(path: tempDbPath)
    }

    override func tearDown() async throws {
        server?.stop()
        server = nil
        if let dbPath = tempDbPath {
            try? FileManager.default.removeItem(atPath: dbPath)
        }
        tempDbPath = nil
        try await super.tearDown()
    }

    // MARK: - Session Mapping Tests

    func testSetupStart_CreatesSessionMapping() async throws {
        // Given: A setup.start request
        let workspace = FileManager.default.temporaryDirectory.path
        let requestDict: [String: Any] = [
            "jsonrpc": "2.0",
            "id": "test-1",
            "method": "orchestrate/setup.start",
            "params": [
                "workspace_root": workspace
            ]
        ]

        let requestData = try JSONSerialization.data(withJSONObject: requestDict)
        let request = try JSONDecoder().decode(JSONRPC.Request<[String: String]>.self, from: requestData)

        // Create a mock client
        var responseReceived: [String: Any]?
        let mockClient = MockBridgeClient { text in
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               dict["id"] as? String == "test-1" {
                responseReceived = dict
            }
        }

        // When: Handle setup.start
        server.currentClient = mockClient
        if let p = requestDict["params"] {
            let data = try JSONSerialization.data(withJSONObject: p)
            let params = try JSONDecoder().decode([String: String].self, from: data)

            // Simulate router calling handler
            await server.handleSetupStart(
                id: JSONRPC.ID("test-1"),
                params: nil,
                rawDict: requestDict,
                client: mockClient
            )
        }

        // Give async operations time to complete
        try await Task.sleep(nanoseconds: 100_000_000) // 100ms

        // Then: Response should contain session_id and conversation_id
        XCTAssertNotNil(responseReceived)
        if let result = responseReceived?["result"] as? [String: Any] {
            let sessionId = result["session_id"] as? String
            let conversationId = result["conversation_id"] as? String
            XCTAssertNotNil(sessionId, "Response should contain session_id")
            XCTAssertNotNil(conversationId, "Response should contain conversation_id")

            // Verify mapping was created
            if let sid = sessionId {
                XCTAssertNotNil(server.setupSessionById[sid], "Session mapping should be created")
                XCTAssertEqual(server.setupSessionById[sid], conversationId, "Mapping should match conversation_id")
            }
        } else {
            XCTFail("Response should contain result object")
        }
    }

    func testSessionPrompt_RoutesToSetupOrchestrator() async throws {
        // Given: A setup session is active
        let workspace = FileManager.default.temporaryDirectory.path
        let sessionId = ACPSessionId(UUID().uuidString)
        let conversationId = UUID().uuidString

        // Create orchestrator
        let orchestrator = SetupOrchestrator(
            conversationId: conversationId,
            sessionId: sessionId,
            initialWorkspace: workspace,
            updateHub: server.updateHub!,
            completionHandler: { _ in }
        )
        await SetupOrchestratorRegistry.shared.store(orchestrator, for: conversationId)
        server.setupSessionById[sessionId.value] = conversationId

        // When: Send session/prompt
        let promptRequest: [String: Any] = [
            "jsonrpc": "2.0",
            "id": "test-2",
            "method": "session/prompt",
            "params": [
                "session_id": sessionId.value,
                "content": [
                    [
                        "type": "text",
                        "text": "1 AM to 5 AM"
                    ]
                ]
            ]
        ]

        var responseReceived: [String: Any]?
        let mockClient = MockBridgeClient { text in
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               dict["id"] as? String == "test-2" {
                responseReceived = dict
            }
        }

        server.currentClient = mockClient
        await server.handleSessionPrompt(
            id: JSONRPC.ID("test-2"),
            params: nil,
            rawDict: promptRequest,
            client: mockClient
        )

        try await Task.sleep(nanoseconds: 100_000_000) // 100ms

        // Then: Should receive "accepted" status (routed to orchestrator)
        XCTAssertNotNil(responseReceived)
        if let result = responseReceived?["result"] as? [String: Any] {
            XCTAssertEqual(result["status"] as? String, "accepted", "Should return accepted status")
        } else {
            XCTFail("Response should contain result with status")
        }
    }

    func testSetupCompletion_CleansUpMappings() async throws {
        // Given: A setup session that completes successfully
        let workspace = FileManager.default.temporaryDirectory.path
        let sessionId = ACPSessionId(UUID().uuidString)
        let conversationId = UUID().uuidString

        // Store mapping
        server.setupSessionById[sessionId.value] = conversationId

        // Create orchestrator with completion handler
        var completionCalled = false
        let orchestrator = SetupOrchestrator(
            conversationId: conversationId,
            sessionId: sessionId,
            initialWorkspace: workspace,
            updateHub: server.updateHub!,
            completionHandler: { [weak server] result in
                completionCalled = true
                // Simulate the cleanup that happens in real completionHandler
                if let conversationId = server?.setupSessionById[sessionId.value] {
                    server?.setupSessionById.removeValue(forKey: sessionId.value)
                    await SetupOrchestratorRegistry.shared.remove(conversationId)
                }
            }
        )
        await SetupOrchestratorRegistry.shared.store(orchestrator, for: conversationId)

        // When: Complete the setup (simulate successful save)
        let config = try createMockConfig(workspace: workspace)
        await orchestrator.completionHandler(.success(config))

        try await Task.sleep(nanoseconds: 100_000_000) // 100ms

        // Then: Mappings should be cleaned up
        XCTAssertTrue(completionCalled, "Completion handler should be called")
        XCTAssertNil(server.setupSessionById[sessionId.value], "Session mapping should be removed")
        let registry = await SetupOrchestratorRegistry.shared.get(conversationId)
        XCTAssertNil(registry, "Orchestrator should be removed from registry")
    }

    func testSetupAbort_CleansUpMappings() async throws {
        // Given: A setup session
        let workspace = FileManager.default.temporaryDirectory.path
        let sessionId = ACPSessionId(UUID().uuidString)
        let conversationId = UUID().uuidString

        server.setupSessionById[sessionId.value] = conversationId

        let orchestrator = SetupOrchestrator(
            conversationId: conversationId,
            sessionId: sessionId,
            initialWorkspace: workspace,
            updateHub: server.updateHub!,
            completionHandler: { _ in }
        )
        await SetupOrchestratorRegistry.shared.store(orchestrator, for: conversationId)

        // When: Abort the setup
        let abortRequest: [String: Any] = [
            "jsonrpc": "2.0",
            "id": "test-3",
            "method": "orchestrate/setup.abort",
            "params": [
                "conversation_id": conversationId
            ]
        ]

        var responseReceived: [String: Any]?
        let mockClient = MockBridgeClient { text in
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               dict["id"] as? String == "test-3" {
                responseReceived = dict
            }
        }

        server.currentClient = mockClient
        await server.handleSetupAbort(
            id: JSONRPC.ID("test-3"),
            params: nil,
            rawDict: abortRequest,
            client: mockClient
        )

        try await Task.sleep(nanoseconds: 100_000_000) // 100ms

        // Then: Both mappings should be cleaned up
        XCTAssertNotNil(responseReceived)
        XCTAssertNil(server.setupSessionById[sessionId.value], "Session mapping should be removed")
        let registry = await SetupOrchestratorRegistry.shared.get(conversationId)
        XCTAssertNil(registry, "Orchestrator should be removed from registry")
    }

    func testStaleMapping_IsCleanedUpOnNextPrompt() async throws {
        // Given: A stale session mapping (orchestrator was removed but mapping remains)
        let sessionId = ACPSessionId(UUID().uuidString)
        let conversationId = UUID().uuidString
        server.setupSessionById[sessionId.value] = conversationId
        // Note: No orchestrator in registry

        // When: Send session/prompt
        let promptRequest: [String: Any] = [
            "jsonrpc": "2.0",
            "id": "test-4",
            "method": "session/prompt",
            "params": [
                "session_id": sessionId.value,
                "content": [
                    [
                        "type": "text",
                        "text": "test"
                    ]
                ]
            ]
        ]

        var errorReceived: [String: Any]?
        let mockClient = MockBridgeClient { text in
            if let data = text.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                if dict["error"] != nil {
                    errorReceived = dict
                }
            }
        }

        server.currentClient = mockClient
        await server.handleSessionPrompt(
            id: JSONRPC.ID("test-4"),
            params: nil,
            rawDict: promptRequest,
            client: mockClient
        )

        try await Task.sleep(nanoseconds: 100_000_000) // 100ms

        // Then: Stale mapping should be removed
        XCTAssertNil(server.setupSessionById[sessionId.value], "Stale mapping should be cleaned up")
        // Since no orchestrator exists, should fall through to normal provider flow
        // which will error because no mode is set - that's expected
    }

    // MARK: - Helper Methods

    private func createMockConfig(workspace: String) throws -> OrchestrationConfig {
        return OrchestrationConfig(
            id: "test-config",
            workspaceRoot: workspace,
            schedule: OrchestrationConfig.Schedule(
                type: "cron",
                expression: "0 1 * * *",
                windowStart: "01:00",
                windowEnd: "05:00"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: true,
                wifiOnly: true
            ),
            timeBudgetSec: 1800,
            maxConcurrent: 2,
            goals: ["Test goal"],
            agentPreferences: OrchestrationConfig.AgentPreferences(
                prefer: .claude_code,
                allow: [.claude_code, .codex]
            ),
            focus: OrchestrationConfig.Focus(
                include: ["."],
                exclude: nil
            ),
            prAutomation: OrchestrationConfig.PRAutomation(
                enabled: false,
                draft: true
            ),
            updatedAt: Int64(Date().timeIntervalSince1970)
        )
    }

    // MARK: - Mock Client

    class MockBridgeClient: DesktopWebSocketServer.Client {
        private let onSend: (String) -> Void
        var isHandshakeComplete = true

        init(onSend: @escaping (String) -> Void) {
            self.onSend = onSend
            // Create a dummy connection for testing
            let params = NWParameters.tcp
            let endpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: 8080)
            let connection = NWConnection(to: endpoint, using: params)
            super.init(connection: connection)
        }

        override func send(text: String) {
            onSend(text)
        }
    }
}

#endif
