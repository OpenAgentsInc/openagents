import XCTest
@testable import OpenAgentsCore

final class OrchestrationConfigTests: XCTestCase {

    // MARK: - Initialization Tests

    func testDefaultInit() {
        // Given: Default configuration
        let config = OrchestrationConfig.createDefault(workspaceRoot: "/Users/test/workspace")

        // Then: Should have sensible defaults
        XCTAssertEqual(config.id, "default")
        XCTAssertEqual(config.workspaceRoot, "/Users/test/workspace")
        XCTAssertEqual(config.schedule.expression, "*/30 1-5 * * *")
        XCTAssertEqual(config.constraints.pluggedIn, true)
        XCTAssertEqual(config.constraints.wifiOnly, true)
        XCTAssertEqual(config.timeBudgetSec, 1800)
        XCTAssertEqual(config.maxConcurrent, 2)
        XCTAssertTrue(config.goals.isEmpty)
        XCTAssertNil(config.agentPreferences.prefer)
        XCTAssertTrue(config.agentPreferences.allow.isEmpty)
        XCTAssertNil(config.focus.include)
        XCTAssertNil(config.focus.exclude)
        XCTAssertEqual(config.prAutomation.enabled, false)
        XCTAssertEqual(config.prAutomation.draft, true)
    }

    func testCustomInit() {
        // Given: Custom configuration values
        let config = OrchestrationConfig(
            id: "custom",
            workspaceRoot: "/path/to/workspace",
            timeBudgetSec: 3600,
            maxConcurrent: 4,
            goals: ["refactor error handling", "add tests"],
            agentPreferences: OrchestrationConfig.AgentPreferences(
                prefer: .claude_code,
                allow: [.claude_code, .codex]
            )
        )

        // Then: Should preserve all custom values
        XCTAssertEqual(config.id, "custom")
        XCTAssertEqual(config.workspaceRoot, "/path/to/workspace")
        XCTAssertEqual(config.timeBudgetSec, 3600)
        XCTAssertEqual(config.maxConcurrent, 4)
        XCTAssertEqual(config.goals, ["refactor error handling", "add tests"])
        XCTAssertEqual(config.agentPreferences.prefer, .claude_code)
        XCTAssertEqual(config.agentPreferences.allow, [.claude_code, .codex])
    }

    // MARK: - Codable Tests

    func testCodableRoundtrip() throws {
        // Given: A config with all fields set
        let original = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            schedule: OrchestrationConfig.Schedule(
                type: "cron",
                expression: "0 2 * * *",
                windowStart: "02:00",
                windowEnd: "06:00",
                jitterMs: 600000,
                onMissed: "skip"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: true,
                wifiOnly: false
            ),
            timeBudgetSec: 2400,
            maxConcurrent: 3,
            goals: ["goal1", "goal2"],
            agentPreferences: OrchestrationConfig.AgentPreferences(
                prefer: .codex,
                allow: [.codex]
            ),
            focus: OrchestrationConfig.Focus(
                include: ["ios/**"],
                exclude: ["**/Tests/**"]
            ),
            prAutomation: OrchestrationConfig.PRAutomation(
                enabled: true,
                draft: false,
                branchPrefix: "agent/test/"
            ),
            updatedAt: 1731038400000
        )

        // When: Encode and decode
        let encoder = JSONEncoder()
        let data = try encoder.encode(original)
        let decoder = JSONDecoder()
        let decoded = try decoder.decode(OrchestrationConfig.self, from: data)

        // Then: Should match original
        XCTAssertEqual(decoded, original)
    }

    func testCodableWithOptionalFields() throws {
        // Given: Config with minimal required fields (optionals nil)
        let original = OrchestrationConfig(
            id: "minimal",
            workspaceRoot: "/workspace"
        )

        // When: Encode and decode
        let data = try JSONEncoder().encode(original)
        let decoded = try JSONDecoder().decode(OrchestrationConfig.self, from: data)

        // Then: Should preserve nil values
        XCTAssertNil(decoded.schedule.windowStart)
        XCTAssertNil(decoded.schedule.windowEnd)
        XCTAssertNil(decoded.schedule.jitterMs)
        XCTAssertNil(decoded.schedule.onMissed)
        XCTAssertNil(decoded.agentPreferences.prefer)
        XCTAssertNil(decoded.focus.include)
        XCTAssertNil(decoded.focus.exclude)
    }

    // MARK: - Goals Hash Tests

    func testGoalsHash() {
        // Given: Config with goals
        var config = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            goals: ["refactor", "add tests"]
        )

        // When: Compute hash
        let hash1 = config.goalsHash()

        // Then: Should be stable (same goals = same hash)
        let hash2 = config.goalsHash()
        XCTAssertEqual(hash1, hash2)

        // When: Change goals
        config.goals = ["add tests", "refactor"]  // Same goals, different order

        // Then: Hash should be same (sorted before hashing)
        let hash3 = config.goalsHash()
        XCTAssertEqual(hash1, hash3)

        // When: Different goals
        config.goals = ["different goal"]

        // Then: Hash should be different
        let hash4 = config.goalsHash()
        XCTAssertNotEqual(hash1, hash4)
    }

    func testGoalsHashEmpty() {
        // Given: Config with no goals
        let config = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            goals: []
        )

        // When: Compute hash
        let hash = config.goalsHash()

        // Then: Should produce valid hash
        XCTAssertFalse(hash.isEmpty)
        XCTAssertEqual(hash.count, 16)  // 16 hex characters
    }

    // MARK: - Validation Tests

    func testValidationSuccess() {
        // Given: Valid config
        let config = OrchestrationConfig.createDefault(workspaceRoot: "/workspace")

        // When: Validate
        let errors = config.validate()

        // Then: Should have no errors
        XCTAssertTrue(errors.isEmpty, "Expected no errors, got: \(errors)")
    }

    func testValidationTimeBudgetTooLow() {
        // Given: Config with time budget too low
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/workspace")
        config.timeBudgetSec = 600  // 10 minutes (< 15 min minimum)

        // When: Validate
        let errors = config.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("timeBudgetSec") && $0.contains("900") })
    }

    func testValidationTimeBudgetTooHigh() {
        // Given: Config with time budget too high
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/workspace")
        config.timeBudgetSec = 10000  // > 2 hours maximum

        // When: Validate
        let errors = config.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("timeBudgetSec") && $0.contains("7200") })
    }

    func testValidationMaxConcurrentTooLow() {
        // Given: Config with maxConcurrent too low
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/workspace")
        config.maxConcurrent = 0

        // When: Validate
        let errors = config.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("maxConcurrent") })
    }

    func testValidationMaxConcurrentTooHigh() {
        // Given: Config with maxConcurrent too high
        var config = OrchestrationConfig.createDefault(workspaceRoot: "/workspace")
        config.maxConcurrent = 10

        // When: Validate
        let errors = config.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("maxConcurrent") })
    }

    // MARK: - Schedule Validation Tests

    func testScheduleValidationInvalidCronFields() {
        // Given: Schedule with invalid cron expression
        let schedule = OrchestrationConfig.Schedule(
            expression: "0 2 *"  // Only 3 fields instead of 5
        )

        // When: Validate
        let errors = schedule.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("5 fields") })
    }

    func testScheduleValidationInvalidTimeFormat() {
        // Given: Schedule with invalid time format
        let schedule = OrchestrationConfig.Schedule(
            windowStart: "25:00",  // Invalid hour
            windowEnd: "06:00"
        )

        // When: Validate
        let errors = schedule.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("HH:mm") })
    }

    func testScheduleValidationWindowStartAfterEnd() {
        // Given: Schedule with start after end
        let schedule = OrchestrationConfig.Schedule(
            windowStart: "06:00",
            windowEnd: "02:00"
        )

        // When: Validate
        let errors = schedule.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("before windowEnd") })
    }

    func testScheduleValidationInvalidJitter() {
        // Given: Schedule with invalid jitter
        let schedule = OrchestrationConfig.Schedule(
            jitterMs: 5000000  // > 1 hour
        )

        // When: Validate
        let errors = schedule.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("jitterMs") })
    }

    func testScheduleValidationInvalidOnMissed() {
        // Given: Schedule with invalid onMissed value
        let schedule = OrchestrationConfig.Schedule(
            onMissed: "invalid"
        )

        // When: Validate
        let errors = schedule.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("onMissed") && $0.contains("skip") })
    }

    // MARK: - Agent Preferences Tests

    func testAgentPreferencesEffectivePrefer() {
        // Given: Preferences with prefer in allow list
        let prefs = OrchestrationConfig.AgentPreferences(
            prefer: .claude_code,
            allow: [.claude_code, .codex]
        )

        // When: Get effective prefer
        let effective = prefs.effectivePrefer()

        // Then: Should return prefer
        XCTAssertEqual(effective, .claude_code)
    }

    func testAgentPreferencesEffectivePreferNotInAllow() {
        // Given: Preferences with prefer NOT in allow list
        let prefs = OrchestrationConfig.AgentPreferences(
            prefer: .claude_code,
            allow: [.codex]  // Claude Code not in list
        )

        // When: Get effective prefer
        let effective = prefs.effectivePrefer()

        // Then: Should return nil (ignored)
        XCTAssertNil(effective)
    }

    func testAgentPreferencesEffectivePreferEmptyAllow() {
        // Given: Preferences with prefer but empty allow
        let prefs = OrchestrationConfig.AgentPreferences(
            prefer: .claude_code,
            allow: []
        )

        // When: Get effective prefer
        let effective = prefs.effectivePrefer()

        // Then: Should return prefer (allow empty = use available)
        XCTAssertEqual(effective, .claude_code)
    }

    func testAgentPreferencesEffectiveAllowEmptyUsesAvailable() {
        // Given: Preferences with empty allow
        let prefs = OrchestrationConfig.AgentPreferences(
            prefer: nil,
            allow: []
        )
        let available: [ACPSessionModeId] = [.claude_code, .codex]

        // When: Get effective allow
        let effective = prefs.effectiveAllow(availableAgents: available)

        // Then: Should return available agents
        XCTAssertEqual(effective, available)
    }

    func testAgentPreferencesEffectiveAllowNonEmpty() {
        // Given: Preferences with non-empty allow
        let prefs = OrchestrationConfig.AgentPreferences(
            prefer: nil,
            allow: [.codex]
        )
        let available: [ACPSessionModeId] = [.claude_code, .codex]

        // When: Get effective allow
        let effective = prefs.effectiveAllow(availableAgents: available)

        // Then: Should return configured allow list
        XCTAssertEqual(effective, [.codex])
    }

    func testAgentPreferencesValidationPreferNotInAllow() {
        // Given: Preferences with prefer not in allow
        let prefs = OrchestrationConfig.AgentPreferences(
            prefer: .claude_code,
            allow: [.codex]
        )

        // When: Validate
        let errors = prefs.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("prefer") && $0.contains("allow") })
    }

    // MARK: - Focus Tests

    func testFocusEffectiveIncludeEmpty() {
        // Given: Focus with nil include
        let focus = OrchestrationConfig.Focus(include: nil)

        // When: Get effective include
        let effective = focus.effectiveInclude()

        // Then: Should default to ["."]
        XCTAssertEqual(effective, ["."])
    }

    func testFocusEffectiveIncludeEmptyArray() {
        // Given: Focus with empty array
        let focus = OrchestrationConfig.Focus(include: [])

        // When: Get effective include
        let effective = focus.effectiveInclude()

        // Then: Should default to ["."]
        XCTAssertEqual(effective, ["."])
    }

    func testFocusEffectiveIncludeNonEmpty() {
        // Given: Focus with patterns
        let focus = OrchestrationConfig.Focus(include: ["ios/**", "packages/**"])

        // When: Get effective include
        let effective = focus.effectiveInclude()

        // Then: Should return patterns
        XCTAssertEqual(effective, ["ios/**", "packages/**"])
    }

    func testFocusEffectiveExcludeNil() {
        // Given: Focus with nil exclude
        let focus = OrchestrationConfig.Focus(exclude: nil)

        // When: Get effective exclude
        let effective = focus.effectiveExclude()

        // Then: Should return empty array
        XCTAssertTrue(effective.isEmpty)
    }

    func testFocusValidationPathTraversal() {
        // Given: Focus with path traversal in include
        let focus = OrchestrationConfig.Focus(
            include: ["../etc/passwd"],
            exclude: nil
        )

        // When: Validate
        let errors = focus.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("path traversal") })
    }

    func testFocusValidationAbsolutePath() {
        // Given: Focus with absolute path in exclude
        let focus = OrchestrationConfig.Focus(
            include: nil,
            exclude: ["/etc/hosts"]
        )

        // When: Validate
        let errors = focus.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("absolute path") })
    }

    // MARK: - PR Automation Tests

    func testPRAutomationValidationSuccess() {
        // Given: Valid PR automation config
        let prAuto = OrchestrationConfig.PRAutomation(
            enabled: true,
            draft: false,
            branchPrefix: "agent/orchestration/"
        )

        // When: Validate
        let errors = prAuto.validate()

        // Then: Should have no errors
        XCTAssertTrue(errors.isEmpty)
    }

    func testPRAutomationValidationInvalidBranchPrefix() {
        // Given: PR automation with invalid branch prefix
        let prAuto = OrchestrationConfig.PRAutomation(
            branchPrefix: "agent/../evil"  // Path traversal attempt
        )

        // When: Validate
        let errors = prAuto.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("branchPrefix") })
    }

    func testPRAutomationValidationSpecialCharacters() {
        // Given: PR automation with special characters
        let prAuto = OrchestrationConfig.PRAutomation(
            branchPrefix: "agent@feature#123"  // @ and # not allowed
        )

        // When: Validate
        let errors = prAuto.validate()

        // Then: Should have error
        XCTAssertFalse(errors.isEmpty)
        XCTAssertTrue(errors.contains { $0.contains("branchPrefix") })
    }

    // MARK: - Equatable Tests

    func testEquality() {
        // Given: Two identical configs
        let config1 = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            timeBudgetSec: 1800
        )
        let config2 = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            timeBudgetSec: 1800
        )

        // Then: Should be equal
        XCTAssertEqual(config1, config2)
    }

    func testInequality() {
        // Given: Two different configs
        let config1 = OrchestrationConfig(
            id: "test1",
            workspaceRoot: "/workspace"
        )
        let config2 = OrchestrationConfig(
            id: "test2",
            workspaceRoot: "/workspace"
        )

        // Then: Should not be equal
        XCTAssertNotEqual(config1, config2)
    }

    // MARK: - Tinyvex Integration Tests

    private func createTestDB() throws -> TinyvexDbLayer {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("config_test_\(UUID().uuidString).sqlite")
        return try TinyvexDbLayer(path: tmp.path)
    }

    func testTinyvexInsertAndGet() throws {
        // Given: Database and config
        let db = try createTestDB()
        let config = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            goals: ["goal1", "goal2"]
        )

        // When: Insert config
        let json = try JSONEncoder().encode(config)
        let jsonString = String(data: json, encoding: .utf8)!
        try db.insertOrUpdateOrchestrationConfig(
            jsonString,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // And: Retrieve config
        guard let retrieved = try db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        ) else {
            XCTFail("Config should exist")
            return
        }

        // Then: Should match original
        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved.data(using: .utf8)!
        )
        XCTAssertEqual(retrievedConfig, config)
    }

    func testTinyvexUpdate() throws {
        // Given: Database with existing config
        let db = try createTestDB()
        var config = OrchestrationConfig(
            id: "test",
            workspaceRoot: "/workspace",
            goals: ["original goal"]
        )

        let json1 = try JSONEncoder().encode(config)
        try db.insertOrUpdateOrchestrationConfig(
            String(data: json1, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // When: Update config
        config.goals = ["updated goal"]
        config.updatedAt = Int64(Date().timeIntervalSince1970 * 1000)
        let json2 = try JSONEncoder().encode(config)
        try db.insertOrUpdateOrchestrationConfig(
            String(data: json2, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // Then: Should retrieve updated version
        let retrieved = try db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )!
        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved.data(using: .utf8)!
        )
        XCTAssertEqual(retrievedConfig.goals, ["updated goal"])
    }

    func testTinyvexListConfigs() throws {
        // Given: Database with multiple configs
        let db = try createTestDB()
        let workspace = "/workspace"

        let config1 = OrchestrationConfig(id: "config1", workspaceRoot: workspace)
        let config2 = OrchestrationConfig(id: "config2", workspaceRoot: workspace)
        let config3 = OrchestrationConfig(id: "config3", workspaceRoot: "/other")

        for config in [config1, config2, config3] {
            let json = try JSONEncoder().encode(config)
            try db.insertOrUpdateOrchestrationConfig(
                String(data: json, encoding: .utf8)!,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )
        }

        // When: List configs for workspace
        let configs = try db.listOrchestrationConfigs(workspaceRoot: workspace)

        // Then: Should return only matching workspace configs
        XCTAssertEqual(configs.count, 2)

        let decoded = try configs.map {
            try JSONDecoder().decode(OrchestrationConfig.self, from: $0.data(using: .utf8)!)
        }
        let ids = decoded.map { $0.id }.sorted()
        XCTAssertEqual(ids, ["config1", "config2"])
    }

    func testTinyvexListAllConfigs() throws {
        // Given: Database with configs from multiple workspaces
        let db = try createTestDB()

        let config1 = OrchestrationConfig(id: "config1", workspaceRoot: "/workspace1")
        let config2 = OrchestrationConfig(id: "config2", workspaceRoot: "/workspace2")

        for config in [config1, config2] {
            let json = try JSONEncoder().encode(config)
            try db.insertOrUpdateOrchestrationConfig(
                String(data: json, encoding: .utf8)!,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )
        }

        // When: List all configs
        let configs = try db.listAllOrchestrationConfigs()

        // Then: Should return all configs
        XCTAssertEqual(configs.count, 2)
    }

    func testTinyvexDelete() throws {
        // Given: Database with config
        let db = try createTestDB()
        let config = OrchestrationConfig(id: "test", workspaceRoot: "/workspace")

        let json = try JSONEncoder().encode(config)
        try db.insertOrUpdateOrchestrationConfig(
            String(data: json, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // When: Delete config
        try db.deleteOrchestrationConfig(id: config.id, workspaceRoot: config.workspaceRoot)

        // Then: Should not be retrievable
        let retrieved = try db.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )
        XCTAssertNil(retrieved)
    }

    func testTinyvexPersistence() throws {
        // Given: Database with config
        let tempPath = FileManager.default.temporaryDirectory
            .appendingPathComponent("config_persist_\(UUID().uuidString).sqlite")

        var db: TinyvexDbLayer? = try TinyvexDbLayer(path: tempPath.path)

        let config = OrchestrationConfig(
            id: "persistent",
            workspaceRoot: "/workspace",
            goals: ["persist me"]
        )

        let json = try JSONEncoder().encode(config)
        try db!.insertOrUpdateOrchestrationConfig(
            String(data: json, encoding: .utf8)!,
            id: config.id,
            workspaceRoot: config.workspaceRoot,
            updatedAt: config.updatedAt
        )

        // When: Close and reopen database
        db = nil
        db = try TinyvexDbLayer(path: tempPath.path)

        // Then: Config should still exist
        let retrieved = try db!.getOrchestrationConfig(
            id: config.id,
            workspaceRoot: config.workspaceRoot
        )
        XCTAssertNotNil(retrieved)

        let retrievedConfig = try JSONDecoder().decode(
            OrchestrationConfig.self,
            from: retrieved!.data(using: .utf8)!
        )
        XCTAssertEqual(retrievedConfig.goals, ["persist me"])
    }

    func testTinyvexMultiWorkspaceIsolation() throws {
        // Given: Same config ID in different workspaces
        let db = try createTestDB()
        let config1 = OrchestrationConfig(id: "default", workspaceRoot: "/workspace1")
        let config2 = OrchestrationConfig(id: "default", workspaceRoot: "/workspace2", goals: ["different"])

        for config in [config1, config2] {
            let json = try JSONEncoder().encode(config)
            try db.insertOrUpdateOrchestrationConfig(
                String(data: json, encoding: .utf8)!,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )
        }

        // When: Retrieve each by workspace
        let retrieved1 = try db.getOrchestrationConfig(id: "default", workspaceRoot: "/workspace1")!
        let retrieved2 = try db.getOrchestrationConfig(id: "default", workspaceRoot: "/workspace2")!

        // Then: Should get correct config for each workspace
        let decoded1 = try JSONDecoder().decode(OrchestrationConfig.self, from: retrieved1.data(using: .utf8)!)
        let decoded2 = try JSONDecoder().decode(OrchestrationConfig.self, from: retrieved2.data(using: .utf8)!)

        XCTAssertTrue(decoded1.goals.isEmpty)
        XCTAssertEqual(decoded2.goals, ["different"])
    }
}
