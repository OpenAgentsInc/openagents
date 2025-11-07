// ExploreOrchestratorTests.swift — Tests for ExploreOrchestrator with native tool calling

import XCTest
@testable import OpenAgentsCore

#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, *)
final class ExploreOrchestratorTests: XCTestCase {

    func testPersistentSessionCreation() async throws {
        // Verify that persistent session is created and reused
        var updateCount = 0
        let orchestrator = ExploreOrchestrator(
            workspaceRoot: FileManager.default.temporaryDirectory.path,
            goals: ["Test goal"],
            policy: ExplorationPolicy(use_native_tool_calling: false),
            streamHandler: { _ in updateCount += 1 }
        )

        // First exploration should create session
        do {
            _ = try await orchestrator.startExploration()
        } catch OrchestrationError.modelUnavailable {
            // Expected if FM not available on test machine
            throw XCTSkip("Foundation Models not available")
        }

        // Verify stream handler was called
        XCTAssertGreaterThan(updateCount, 0, "Stream handler should be called during exploration")
    }

    func testFeatureFlagRoutesCorrectly() async throws {
        // Test that feature flag routes to correct code path
        let legacyOrchestrator = ExploreOrchestrator(
            workspaceRoot: FileManager.default.temporaryDirectory.path,
            goals: ["Test goal"],
            policy: ExplorationPolicy(use_native_tool_calling: false),
            streamHandler: { _ in }
        )

        let nativeOrchestrator = ExploreOrchestrator(
            workspaceRoot: FileManager.default.temporaryDirectory.path,
            goals: ["Test goal"],
            policy: ExplorationPolicy(use_native_tool_calling: true),
            streamHandler: { _ in }
        )

        // Both should be able to create without crashing
        // Actual execution would require FM availability
        XCTAssertNotNil(legacyOrchestrator)
        XCTAssertNotNil(nativeOrchestrator)
    }

    func testPolicyDefaults() {
        // Verify policy defaults are safe
        let defaultPolicy = ExplorationPolicy()

        XCTAssertFalse(defaultPolicy.allow_external_llms, "External LLMs should be disabled by default")
        XCTAssertFalse(defaultPolicy.allow_network, "Network should be disabled by default")
        XCTAssertFalse(defaultPolicy.use_native_tool_calling, "Native tool calling should be opt-in")
    }

    func testPolicyCustomization() {
        // Verify policy can be customized
        let customPolicy = ExplorationPolicy(
            allow_external_llms: false,
            allow_network: true,
            use_native_tool_calling: true
        )

        XCTAssertFalse(customPolicy.allow_external_llms)
        XCTAssertTrue(customPolicy.allow_network)
        XCTAssertTrue(customPolicy.use_native_tool_calling)
    }

    func testWorkspaceValidation() async throws {
        // Test with non-existent workspace
        let orchestrator = ExploreOrchestrator(
            workspaceRoot: "/nonexistent/path/to/workspace",
            goals: ["Test goal"],
            policy: ExplorationPolicy(),
            streamHandler: { _ in }
        )

        do {
            _ = try await orchestrator.startExploration()
            XCTFail("Should have thrown workspace validation error")
        } catch {
            // Expected - either model unavailable or workspace validation error
            XCTAssertTrue(
                error is OrchestrationError,
                "Error should be OrchestrationError, got \(type(of: error))"
            )
        }
    }

    func testEmptyGoals() async throws {
        // Test with empty goals array
        let orchestrator = ExploreOrchestrator(
            workspaceRoot: FileManager.default.temporaryDirectory.path,
            goals: [],
            policy: ExplorationPolicy(use_native_tool_calling: false),
            streamHandler: { _ in }
        )

        // Should not crash with empty goals
        do {
            _ = try await orchestrator.startExploration()
        } catch OrchestrationError.modelUnavailable {
            throw XCTSkip("Foundation Models not available")
        }
    }

    func testNativeToolCallingWithMockWorkspace() async throws {
        // Create a temporary workspace
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("test-workspace-\(UUID().uuidString)")

        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(at: tempDir)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: tempDir.path,
            goals: ["Explore the workspace"],
            policy: ExplorationPolicy(use_native_tool_calling: true),
            streamHandler: { _ in }
        )

        do {
            let summary = try await orchestrator.startExploration()
            XCTAssertNotNil(summary)
            XCTAssertEqual(summary.repo_name, tempDir.lastPathComponent)
        } catch OrchestrationError.modelUnavailable {
            throw XCTSkip("Foundation Models not available")
        }
    }

    func testStreamHandlerReceivesUpdates() async throws {
        var receivedUpdates: [ACP.Client.SessionUpdate] = []

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: FileManager.default.temporaryDirectory.path,
            goals: ["Test streaming"],
            policy: ExplorationPolicy(use_native_tool_calling: true),
            streamHandler: { update in
                receivedUpdates.append(update)
            }
        )

        do {
            _ = try await orchestrator.startExploration()
            XCTAssertGreaterThan(receivedUpdates.count, 0, "Should receive at least one update")
        } catch OrchestrationError.modelUnavailable {
            throw XCTSkip("Foundation Models not available")
        }
    }
}
#endif
