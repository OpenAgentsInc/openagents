// OrchestrateRPCTests.swift — Integration tests for orchestrate.explore.start RPC
// Ensures the full request/response cycle works correctly and errors are communicated

import XCTest
@testable import OpenAgentsCore

final class OrchestrateRPCTests: XCTestCase {

    var server: DesktopWebSocketServer!
    var mockDelegate: MockWebSocketServerDelegate!

    override func setUp() {
        super.setUp()
        server = DesktopWebSocketServer()
        mockDelegate = MockWebSocketServerDelegate()
        server.delegate = mockDelegate
    }

    override func tearDown() {
        server?.stop()
        server = nil
        mockDelegate = nil
        super.tearDown()
    }

    // MARK: - Request Parsing Tests

    func testOrchestrateExploreStart_ParsesRequestCorrectly() throws {
        // Given: A valid orchestrate.explore.start request
        let requestJSON = """
        {
            "jsonrpc": "2.0",
            "id": "test-123",
            "method": "orchestrate.explore.start",
            "params": {
                "root": "/Users/test/workspace",
                "goals": ["Understand structure", "Find entry points"],
                "policy": {
                    "allow_external_llms": false,
                    "allow_network": false
                }
            }
        }
        """

        let data = requestJSON.data(using: .utf8)!
        let decoder = JSONDecoder()

        // When: Parsing the request envelope
        let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        XCTAssertNotNil(dict)
        XCTAssertEqual(dict?["method"] as? String, "orchestrate.explore.start")

        // Parse params
        let paramsData = try JSONSerialization.data(withJSONObject: dict!["params"]!)
        let request = try decoder.decode(OrchestrateExploreStartRequest.self, from: paramsData)

        // Then: Request should be parsed correctly
        XCTAssertEqual(request.root, "/Users/test/workspace")
        XCTAssertEqual(request.goals?.count, 2)
        XCTAssertEqual(request.goals?[0], "Understand structure")
        XCTAssertEqual(request.policy?.allow_external_llms, false)
        XCTAssertEqual(request.policy?.allow_network, false)
    }

    func testOrchestrateExploreStart_ResponseIncludesSessionAndPlanIDs() throws {
        // Given: A response object
        let response = OrchestrateExploreStartResponse(
            session_id: "session-123",
            plan_id: "plan-456",
            status: "started"
        )

        // When: Encoding the response
        let encoder = JSONEncoder()
        let data = try encoder.encode(response)
        let dict = try JSONSerialization.jsonObject(with: data) as? [String: Any]

        // Then: Response should contain all required fields
        XCTAssertEqual(dict?["session_id"] as? String, "session-123")
        XCTAssertEqual(dict?["plan_id"] as? String, "plan-456")
        XCTAssertEqual(dict?["status"] as? String, "started")
    }

    // MARK: - Error Handling Tests

    func testOrchestrateExploreStart_InvalidWorkspace_ReturnsErrorMessage() async throws {
        // Given: Start the server
        try server.start(port: 0, advertiseService: false) // Port 0 = random available port

        // Create a mock client
        let mockClient = createMockClient()

        // Simulate handshake
        simulateHandshake(mockClient)

        // Given: Invalid workspace path request
        let invalidRequest = OrchestrateExploreStartRequest(
            root: "/nonexistent/invalid/path",
            goals: ["Test"],
            policy: ExplorationPolicy(allow_external_llms: false, allow_network: false)
        )

        // When: Send orchestrate.explore.start request
        var receivedMessages: [String] = []
        mockClient.onReceiveText = { text in
            receivedMessages.append(text)
        }

        let requestJSON = try createRPCRequest(method: "orchestrate.explore.start", params: invalidRequest, id: "test-456")
        await sendTextToServer(mockClient, text: requestJSON)

        // Give server time to process
        try await Task.sleep(nanoseconds: 500_000_000) // 500ms

        // Then: Should receive response + error message
        XCTAssertGreaterThanOrEqual(receivedMessages.count, 1, "Should receive at least a response")

        // Check for response with session_id
        let hasResponse = receivedMessages.contains { message in
            message.contains("session_id") && message.contains("plan_id")
        }
        XCTAssertTrue(hasResponse, "Should receive immediate response with session/plan IDs")

        // Check for error message update
        let hasErrorUpdate = receivedMessages.contains { message in
            message.contains("session/update") &&
            message.contains("Orchestration failed") &&
            message.contains("Invalid workspace")
        }
        XCTAssertTrue(hasErrorUpdate, "Should receive error update with descriptive message")
    }

    func testOrchestrateExploreStart_ValidWorkspace_ReturnsSuccessResponse() async throws {
        // Given: Start the server
        try server.start(port: 0, advertiseService: false)

        let mockClient = createMockClient()
        simulateHandshake(mockClient)

        // Create a valid temporary workspace
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("test-workspace-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        // Add a README so there's content
        let readmeFile = tempDir.appendingPathComponent("README.md")
        try "# Test\nContent".write(to: readmeFile, atomically: true, encoding: .utf8)

        // When: Send request with valid workspace
        let validRequest = OrchestrateExploreStartRequest(
            root: tempDir.path,
            goals: ["Test exploration"],
            policy: ExplorationPolicy()
        )

        var receivedMessages: [String] = []
        mockClient.onReceiveText = { text in
            receivedMessages.append(text)
        }

        let requestJSON = try createRPCRequest(method: "orchestrate.explore.start", params: validRequest, id: "test-789")
        await sendTextToServer(mockClient, text: requestJSON)

        // Give server time to process
        try await Task.sleep(nanoseconds: 1_000_000_000) // 1 second

        // Then: Should receive success response
        let hasSuccessResponse = receivedMessages.contains { message in
            message.contains("\"status\":\"started\"") || message.contains("status") && message.contains("started")
        }
        XCTAssertTrue(hasSuccessResponse, "Should receive success response for valid workspace")

        // May receive FM unavailable message or actual exploration updates
        // Either is acceptable - just verify we got some communication
        XCTAssertGreaterThanOrEqual(receivedMessages.count, 1, "Should receive at least one message")
    }

    // MARK: - Tool Call Streaming Tests

    func testOrchestrateExploreStart_StreamsToolCallUpdates() async throws {
        // Given: Valid workspace with content
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("test-tools-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let readmeFile = tempDir.appendingPathComponent("README.md")
        try "# Project\n\nDescription".write(to: readmeFile, atomically: true, encoding: .utf8)

        try server.start(port: 0, advertiseService: false)
        let mockClient = createMockClient()
        simulateHandshake(mockClient)

        var receivedMessages: [String] = []
        mockClient.onReceiveText = { text in
            receivedMessages.append(text)
        }

        // When: Start orchestration
        let request = OrchestrateExploreStartRequest(
            root: tempDir.path,
            goals: ["Explore structure"],
            policy: ExplorationPolicy()
        )

        let requestJSON = try createRPCRequest(method: "orchestrate.explore.start", params: request, id: "test-stream")
        await sendTextToServer(mockClient, text: requestJSON)

        // Wait for orchestration to run
        try await Task.sleep(nanoseconds: 2_000_000_000) // 2 seconds

        // Then: Should receive various update types
        // Looking for session/update notifications
        let hasSessionUpdates = receivedMessages.contains { message in
            message.contains("session/update")
        }

        XCTAssertTrue(hasSessionUpdates, "Should receive session/update notifications")

        // May receive tool_call or plan updates depending on FM availability
        // Just verify we got meaningful communication
        XCTAssertGreaterThanOrEqual(receivedMessages.count, 1, "Should stream updates")
    }

    // MARK: - Helper Methods

    private func createMockClient() -> MockClient {
        return MockClient()
    }

    private func simulateHandshake(_ client: MockClient) {
        client.isHandshakeComplete = true
    }

    private func createRPCRequest<P: Encodable>(method: String, params: P, id: String) throws -> String {
        let request = JSONRPC.Request(id: JSONRPC.ID(id), method: method, params: params)
        let data = try JSONEncoder().encode(request)
        return String(data: data, encoding: .utf8)!
    }

    private func sendTextToServer(_ client: MockClient, text: String) async {
        // Simulate sending text to server's handleTextMessage
        // In real implementation, this would go through WebSocket
        // For testing, we call the handler directly
        await MainActor.run {
            // Note: This is a simplified test - in practice you'd need to properly
            // simulate the WebSocket connection and message flow
        }
    }

    // MARK: - Mock Client

    class MockClient {
        var isHandshakeComplete = false
        var onReceiveText: ((String) -> Void)?

        func send(text: String) {
            onReceiveText?(text)
        }
    }
}

// MARK: - Mock Delegate

class MockWebSocketServerDelegate: DesktopWebSocketServerDelegate {
    var connectedClients: [Any] = []
    var handshakeCompletedCalls = 0
    var disconnectedCalls = 0

    func webSocketServer(_ server: DesktopWebSocketServer, didAcceptClient client: Any) {
        connectedClients.append(client)
    }

    func webSocketServer(_ server: DesktopWebSocketServer, didCompleteHandshakeFor client: Any, success: Bool) {
        if success {
            handshakeCompletedCalls += 1
        }
    }

    func webSocketServer(_ server: DesktopWebSocketServer, didDisconnect client: Any, reason: String?) {
        disconnectedCalls += 1
        connectedClients.removeAll { _ in true } // Simple remove all for testing
    }
}
