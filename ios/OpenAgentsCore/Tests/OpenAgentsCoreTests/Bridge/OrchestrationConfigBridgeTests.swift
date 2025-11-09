import XCTest
@testable import OpenAgentsCore

#if os(macOS)

final class OrchestrationConfigBridgeTests: XCTestCase {

    // MARK: - Test Helpers

    private func createTestDB() throws -> TinyvexDbLayer {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("bridge_test_\(UUID().uuidString).sqlite")
        return try TinyvexDbLayer(path: tmp.path)
    }

    private func createTestConfig(id: String = "default", workspaceRoot: String = "/workspace") -> OrchestrationConfig {
        return OrchestrationConfig(
            id: id,
            workspaceRoot: workspaceRoot,
            goals: ["test goal"]
        )
    }

    // MARK: - Config Get Tests

    func testConfigGetSuccess() async throws {
        // Given: Database with saved config
        let db = try createTestDB()
        let config = createTestConfig()

        let json = try JSONEncoder().encode(config)
        try await db.insertOrUpdateOrchestrationConfig(
            String(data: json, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // When: Get config via bridge (simulated)
        let retrieved = try await db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )

        // Then: Should retrieve config
        XCTAssertNotNil(retrieved)
        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved!.data(using: .utf8)!
        )
        XCTAssertEqual(retrievedConfig.id, config.id)
        XCTAssertEqual(retrievedConfig.goals, ["test goal"])
    }

    func testConfigGetNotFound() async throws {
        // Given: Empty database
        let db = try createTestDB()

        // When: Get non-existent config
        let retrieved = try await db.getOrchestrationConfig(
            id: "nonexistent",
            workspaceRoot: "/workspace"
        )

        // Then: Should return nil
        XCTAssertNil(retrieved)
    }

    // MARK: - Config Set Tests

    func testConfigSetSuccess() async throws {
        // Given: Valid config
        let db = try createTestDB()
        var config = createTestConfig()
        config.goals = ["refactor error handling", "add tests"]

        // When: Save config
        let errors = config.validate()
        XCTAssertTrue(errors.isEmpty, "Config should be valid")

        let json = try JSONEncoder().encode(config)
        try await db.insertOrUpdateOrchestrationConfig(
            String(data: json, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // Then: Should be retrievable
        let retrieved = try await db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )
        XCTAssertNotNil(retrieved)

        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved!.data(using: .utf8)!
        )
        XCTAssertEqual(retrievedConfig.goals, ["refactor error handling", "add tests"])
    }

    func testConfigSetValidationFailure() throws {
        // Given: Invalid config (time budget too low)
        var config = createTestConfig()
        config.timeBudgetSec = 500  // < 900 minimum

        // When: Validate
        let errors = config.validate()

        // Then: Should have errors
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("timeBudgetSec") })
    }

    func testConfigSetInvalidCronExpression() throws {
        // Given: Config with invalid cron
        var config = createTestConfig()
        config.schedule.expression = "invalid cron"

        // When: Validate
        let errors = config.validate()

        // Then: Should have errors
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("5 fields") })
    }

    func testConfigSetInvalidTimeWindow() throws {
        // Given: Config with start > end
        var config = createTestConfig()
        config.schedule.windowStart = "23:00"
        config.schedule.windowEnd = "01:00"

        // When: Validate
        let errors = config.validate()

        // Then: Should have errors
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("before windowEnd") })
    }

    func testConfigSetInvalidAgentPreferences() throws {
        // Given: Config with prefer not in allow
        var config = createTestConfig()
        config.agentPreferences.prefer = .claude_code
        config.agentPreferences.allow = [.codex]  // Claude Code not in list

        // When: Validate
        let errors = config.validate()

        // Then: Should have errors
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("prefer") && $0.contains("allow") })
    }

    func testConfigSetInvalidFocusGlobs() throws {
        // Given: Config with path traversal
        var config = createTestConfig()
        config.focus.include = ["../etc/passwd"]

        // When: Validate
        let errors = config.validate()

        // Then: Should have errors
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("path traversal") })
    }

    func testConfigSetInvalidBranchPrefix() throws {
        // Given: Config with invalid branch prefix
        var config = createTestConfig()
        config.prAutomation.branchPrefix = "agent/../evil"

        // When: Validate
        let errors = config.validate()

        // Then: Should have errors
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("branchPrefix") })
    }

    // MARK: - Config List Tests

    func testConfigListByWorkspace() async throws {
        // Given: Database with configs for multiple workspaces
        let db = try createTestDB()

        let config1 = createTestConfig(id: "config1", workspaceRoot: "/workspace1")
        let config2 = createTestConfig(id: "config2", workspaceRoot: "/workspace1")
        let config3 = createTestConfig(id: "config3", workspaceRoot: "/workspace2")

        for config in [config1, config2, config3] {
            let json = try JSONEncoder().encode(config)
            try await db.insertOrUpdateOrchestrationConfig(
                String(data: json, encoding: .utf8)!,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )
        }

        // When: List configs for workspace1
        let configs = try await db.listOrchestrationConfigs(workspaceRoot: "/workspace1")

        // Then: Should return only workspace1 configs
        XCTAssertEqual(configs.count, 2)

        let decoded = try configs.map {
            try JSONDecoder().decode(OrchestrationConfig.self, from: $0.data(using: .utf8)!)
        }
        let ids = decoded.map { $0.id }.sorted()
        XCTAssertEqual(ids, ["config1", "config2"])
    }

    func testConfigListAll() async throws {
        // Given: Database with configs
        let db = try createTestDB()

        let config1 = createTestConfig(id: "config1", workspaceRoot: "/workspace1")
        let config2 = createTestConfig(id: "config2", workspaceRoot: "/workspace2")

        for config in [config1, config2] {
            let json = try JSONEncoder().encode(config)
            try await db.insertOrUpdateOrchestrationConfig(
                String(data: json, encoding: .utf8)!,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )
        }

        // When: List all configs
        let configs = try await db.listAllOrchestrationConfigs()

        // Then: Should return all configs
        XCTAssertEqual(configs.count, 2)
    }

    // MARK: - Config Activate Tests

    func testConfigActivateSuccess() async throws {
        // Given: Database with config
        let db = try createTestDB()
        let config = createTestConfig()

        let json = try JSONEncoder().encode(config)
        try await db.insertOrUpdateOrchestrationConfig(
            String(data: json, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // When: Activate config (simulated check for existence)
        let retrieved = try await db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )

        // Then: Should exist
        XCTAssertNotNil(retrieved)
    }

    func testConfigActivateNotFound() async throws {
        // Given: Empty database
        let db = try createTestDB()

        // When: Try to activate non-existent config
        let retrieved = try await db.getOrchestrationConfig(
            id: "nonexistent",
            workspaceRoot: "/workspace"
        )

        // Then: Should not exist
        XCTAssertNil(retrieved)
    }

    // MARK: - Integration Tests

    func testConfigUpdatePreservesOtherFields() async throws {
        // Given: Saved config
        let db = try createTestDB()
        var config = createTestConfig()
        config.goals = ["original goal"]
        config.timeBudgetSec = 1800
        config.maxConcurrent = 2

        let json1 = try JSONEncoder().encode(config)
        try await db.insertOrUpdateOrchestrationConfig(
            String(data: json1, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // When: Update only goals
        config.goals = ["updated goal"]
        config.updatedAt = Int64(Date().timeIntervalSince1970 * 1000)
        let json2 = try JSONEncoder().encode(config)
        try await db.insertOrUpdateOrchestrationConfig(
            String(data: json2, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // Then: Other fields should be preserved
        let retrieved = try await db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )!
        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved.data(using: .utf8)!
        )

        XCTAssertEqual(retrievedConfig.goals, ["updated goal"])
        XCTAssertEqual(retrievedConfig.timeBudgetSec, 1800)
        XCTAssertEqual(retrievedConfig.maxConcurrent, 2)
    }

    func testConfigComplexScenario() async throws {
        // Given: Database
        let db = try createTestDB()

        // When: Create config with all features
        var config = createTestConfig()
        config.schedule = OrchestrationConfig.Schedule(
            type: "cron",
            expression: "*/30 1-5 * * *",
            windowStart: "01:00",
            windowEnd: "05:00",
            jitterMs: 300000,
            onMissed: "catch_up"
        )
        config.constraints = OrchestrationConfig.Constraints(
            pluggedIn: true,
            wifiOnly: true
        )
        config.timeBudgetSec = 1800
        config.maxConcurrent = 2
        config.goals = ["refactor error handling", "increase test coverage"]
        config.agentPreferences = OrchestrationConfig.AgentPreferences(
            prefer: .claude_code,
            allow: [.claude_code, .codex]
        )
        config.focus = OrchestrationConfig.Focus(
            include: ["ios/**"],
            exclude: ["**/Tests/**"]
        )
        config.prAutomation = OrchestrationConfig.PRAutomation(
            enabled: true,
            draft: false,
            branchPrefix: "agent/orchestration/"
        )

        // Validate
        let errors = config.validate()
        XCTAssertTrue(errors.isEmpty, "Config should be valid: \(errors)")

        // Save
        let json = try JSONEncoder().encode(config)
        try await db.insertOrUpdateOrchestrationConfig(
            String(data: json, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // Retrieve
        let retrieved = try await db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )!
        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved.data(using: .utf8)!
        )

        // Then: All fields should match
        XCTAssertEqual(retrievedConfig.schedule.expression, "*/30 1-5 * * *")
        XCTAssertEqual(retrievedConfig.goals, ["refactor error handling", "increase test coverage"])
        XCTAssertEqual(retrievedConfig.agentPreferences.prefer, .claude_code)
        XCTAssertEqual(retrievedConfig.focus.include, ["ios/**"])
        XCTAssertEqual(retrievedConfig.prAutomation.branchPrefix, "agent/orchestration/")
    }
}

#endif
