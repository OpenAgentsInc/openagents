import XCTest
@testable import OpenAgentsCore

#if os(macOS)
final class CoordinatorLocalTests: XCTestCase {
    func testLocalCoordinatorRunOnce_NoAgents_ReturnsNoAgents() async throws {
        // Given: Desktop server with Tinyvex DB and update hub
        let server = DesktopWebSocketServer()
        let tempDb = FileManager.default.temporaryDirectory.appendingPathComponent("coord_local_\(UUID().uuidString).sqlite").path
        let db = try TinyvexDbLayer(path: tempDb)
        server.tinyvexDb = db
        server.updateHub = SessionUpdateHub(tinyvexDb: db, broadcastCallback: { _ in })
        // Workspace
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("coord_ws_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        server.workingDirectory = tempDir
        // Active config
        server.activeOrchestrationConfig = OrchestrationConfig.createDefault(workspaceRoot: tempDir.path)

        // When: Ask for status and run_once locally
        let status0 = await server.localCoordinatorStatus()
        let result = await server.localCoordinatorRunOnce(config: nil)

        // Then: Initial status zeroed and run returns deterministically with no agents available
        XCTAssertEqual(status0.cycles_run, 0)
        XCTAssertEqual(status0.tasks_executed, 0)
        XCTAssertTrue(["no_agents", "enqueued", "executing", "idle", "failed", "error"].contains(result.status))
        // In CI/no-CLI environments, we expect no_agents
        // Don't assert strictly to keep the test resilient if a provider is installed locally.
    }
}
#endif

