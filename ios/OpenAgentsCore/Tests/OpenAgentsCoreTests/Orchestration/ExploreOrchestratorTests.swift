// ExploreOrchestratorTests.swift — Comprehensive tests for workspace orchestration
// Ensures workspace validation, error handling, and FM availability checks work correctly

import XCTest
@testable import OpenAgentsCore

#if canImport(FoundationModels)
import FoundationModels
#endif

@available(macOS 26.0, iOS 26.0, *)
final class ExploreOrchestratorTests: XCTestCase {

    // MARK: - Workspace Validation Tests

    func testWorkspaceValidation_NonexistentPath_ThrowsError() async throws {
        // Given: A nonexistent workspace path
        let nonexistentPath = "/nonexistent/path/that/does/not/exist"
        var capturedUpdates: [ACP.Client.SessionUpdate] = []

        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: nonexistentPath,
            goals: ["Test"],
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When/Then: Starting exploration should throw workspaceInvalid error
        do {
            _ = try await orchestrator.startExploration()
            XCTFail("Expected workspaceInvalid error but got success")
        } catch let error as OrchestrationError {
            switch error {
            case .workspaceInvalid(let message):
                XCTAssertTrue(message.contains("does not exist"), "Error message should mention path doesn't exist")
            default:
                XCTFail("Expected workspaceInvalid error but got: \(error)")
            }
        }

        // Verify: Error message was streamed to client
        XCTAssertFalse(capturedUpdates.isEmpty, "Should have streamed error message")

        // Check for agent message chunk with error
        let hasErrorMessage = capturedUpdates.contains { update in
            if case .agentMessageChunk(let chunk) = update {
                switch chunk.content {
                case .text(let text):
                    return text.text.contains("Orchestration failed") || text.text.contains("unavailable")
                default:
                    return false
                }
            }
            return false
        }

        XCTAssertTrue(hasErrorMessage, "Should have streamed an error message to the client")
    }

    func testWorkspaceValidation_FilePath_ThrowsError() async throws {
        // Given: A path that points to a file, not a directory
        let tempFile = FileManager.default.temporaryDirectory.appendingPathComponent("test-file-\(UUID().uuidString).txt")
        try "test content".write(to: tempFile, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: tempFile) }

        var capturedUpdates: [ACP.Client.SessionUpdate] = []
        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: tempFile.path,
            goals: ["Test"],
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When/Then: Should throw error because path is not a directory
        do {
            _ = try await orchestrator.startExploration()
            XCTFail("Expected workspaceInvalid error for file path")
        } catch let error as OrchestrationError {
            switch error {
            case .workspaceInvalid(let message):
                XCTAssertTrue(message.contains("not a directory"), "Error should mention it's not a directory")
            default:
                XCTFail("Expected workspaceInvalid error but got: \(error)")
            }
        }
    }

    func testWorkspaceValidation_ValidPath_Succeeds() async throws {
        // Given: A valid temporary directory
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("test-workspace-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        // Create a README file so there's something to explore
        let readmeFile = tempDir.appendingPathComponent("README.md")
        try "# Test Project\n\nThis is a test.".write(to: readmeFile, atomically: true, encoding: .utf8)

        var capturedUpdates: [ACP.Client.SessionUpdate] = []
        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: tempDir.path,
            goals: ["Test exploration"],
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When: Starting exploration with valid path
        // Note: This will attempt to use Foundation Models, which may or may not be available
        do {
            let summary = try await orchestrator.startExploration()

            // Then: Should complete successfully
            XCTAssertFalse(summary.repo_name.isEmpty, "Should have a repo name")
            XCTAssertFalse(capturedUpdates.isEmpty, "Should have streamed some updates")

            // Verify we got a plan
            let hasPlan = capturedUpdates.contains { update in
                if case .plan = update { return true }
                return false
            }
            XCTAssertTrue(hasPlan, "Should have streamed a plan")

        } catch let error as OrchestrationError {
            // If FM is unavailable, that's expected - just verify error was communicated
            switch error {
            case .modelUnavailable:
                // Verify error message was streamed
                let hasErrorMessage = capturedUpdates.contains { update in
                    if case .agentMessageChunk(let chunk) = update {
                        switch chunk.content {
                        case .text(let text):
                            return text.text.contains("unavailable")
                        default:
                            return false
                        }
                    }
                    return false
                }
                XCTAssertTrue(hasErrorMessage, "FM unavailable error should be communicated to client")
            default:
                XCTFail("Unexpected error: \(error)")
            }
        }
    }

    // MARK: - Error Message Streaming Tests

    func testErrorMessagesAreStreamedToClient() async throws {
        // Given: An orchestrator that will fail (invalid path)
        let invalidPath = "/absolutely/nonexistent/path/for/testing"
        var capturedUpdates: [ACP.Client.SessionUpdate] = []

        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: invalidPath,
            goals: ["Test"],
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When: Starting exploration fails
        do {
            _ = try await orchestrator.startExploration()
        } catch {
            // Expected to fail
        }

        // Then: Error message should be in captured updates
        var foundErrorText: String?
        for update in capturedUpdates {
            if case .agentMessageChunk(let chunk) = update {
                switch chunk.content {
                case .text(let textContent):
                    if textContent.text.contains("❌") || textContent.text.contains("failed") || textContent.text.contains("unavailable") {
                        foundErrorText = textContent.text
                        break
                    }
                default:
                    break
                }
            }
        }

        XCTAssertNotNil(foundErrorText, "Should have streamed an error message")
        if let errorText = foundErrorText {
            XCTAssertTrue(
                errorText.contains("Orchestration failed") || errorText.contains("unavailable"),
                "Error message should be descriptive. Got: \(errorText)"
            )
        }
    }

    // MARK: - Foundation Models Availability Tests

    #if canImport(FoundationModels)
    func testFoundationModelsUnavailable_StreamsErrorMessage() async throws {
        // This test verifies that when FM is unavailable, a proper error message is sent

        // Given: A valid workspace (so the only failure point is FM availability)
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("test-fm-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        var capturedUpdates: [ACP.Client.SessionUpdate] = []
        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: tempDir.path,
            goals: ["Test FM availability"],
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When: Starting exploration
        do {
            _ = try await orchestrator.startExploration()

            // If we get here, FM was available - verify we got proper updates
            XCTAssertFalse(capturedUpdates.isEmpty, "Should have updates when FM is available")

        } catch let error as OrchestrationError {
            // If FM is unavailable, verify proper error handling
            if case .modelUnavailable = error {
                // Verify error was streamed to client
                let hasUnavailableMessage = capturedUpdates.contains { update in
                    if case .agentMessageChunk(let chunk) = update {
                        switch chunk.content {
                        case .text(let text):
                            return text.text.contains("Foundation Models are unavailable")
                        default:
                            return false
                        }
                    }
                    return false
                }

                XCTAssertTrue(hasUnavailableMessage, "FM unavailable message should be streamed to client")
            } else {
                XCTFail("Unexpected error type: \(error)")
            }
        }
    }
    #endif

    // MARK: - Policy Tests

    func testPolicyRespected_OnDeviceOnly() {
        // Given: Policy with external LLMs disabled
        let policy = ExplorationPolicy(allow_external_llms: false, allow_network: false)

        // Then: Policy flags should be set correctly
        XCTAssertFalse(policy.allow_external_llms, "External LLMs should be disabled")
        XCTAssertFalse(policy.allow_network, "Network should be disabled")
    }

    // MARK: - Context Size Limit Tests

    func testPromptExceedsContextLimit_ThrowsError() async throws {
        // Given: A workspace with extremely long goals that will exceed 4096 token limit
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("test-context-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        // Create absurdly long goals (each char ~= 0.25 tokens, so 20,000 chars ~= 5,000 tokens)
        let longGoal = String(repeating: "Find all instances of authentication patterns and session management code including OAuth2 flows, JWT token validation, refresh token handling, password reset mechanisms, MFA implementations, SSO integrations, and security audit logs ", count: 100)
        let goals = [longGoal]

        var capturedUpdates: [ACP.Client.SessionUpdate] = []
        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: tempDir.path,
            goals: goals,
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When/Then: Should fail with context size error
        do {
            _ = try await orchestrator.startExploration()
            XCTFail("Expected executionFailed error due to context size limit")
        } catch let error as OrchestrationError {
            switch error {
            case .executionFailed(let message):
                XCTAssertTrue(
                    message.contains("Prompt too large") || message.contains("tokens") || message.contains("context"),
                    "Error should mention token/context size. Got: \(message)"
                )
            case .modelUnavailable:
                // FM not available - skip this test
                throw XCTSkip("Foundation Models not available on this device")
            default:
                XCTFail("Expected executionFailed error but got: \(error)")
            }
        }

        // Verify error was communicated to client
        let hasErrorMessage = capturedUpdates.contains { update in
            if case .agentMessageChunk(let chunk) = update {
                switch chunk.content {
                case .text(let text):
                    return text.text.contains("too large") || text.text.contains("tokens") || text.text.contains("context")
                default:
                    return false
                }
            }
            return false
        }
        XCTAssertTrue(hasErrorMessage, "Context size error should be streamed to client")
    }

    func testNormalGoals_StayUnderContextLimit() async throws {
        // Given: Normal, reasonable goals
        let tempDir = FileManager.default.temporaryDirectory.appendingPathComponent("test-normal-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tempDir) }

        let normalGoals = [
            "Find recent conversations about this project",
            "Identify most frequently modified files"
        ]

        var capturedUpdates: [ACP.Client.SessionUpdate] = []
        let streamHandler: ACPUpdateStreamHandler = { update in
            capturedUpdates.append(update)
        }

        let orchestrator = ExploreOrchestrator(
            workspaceRoot: tempDir.path,
            goals: normalGoals,
            policy: ExplorationPolicy(),
            streamHandler: streamHandler
        )

        // When: Starting exploration with normal goals
        do {
            _ = try await orchestrator.startExploration()
            // Success - goals were under limit
        } catch let error as OrchestrationError {
            // Should NOT fail due to context size
            switch error {
            case .executionFailed(let message):
                XCTAssertFalse(
                    message.contains("Prompt too large") || message.contains("too many tokens"),
                    "Normal goals should NOT exceed context limit. Error: \(message)"
                )
            case .modelUnavailable:
                // FM not available - skip this test
                throw XCTSkip("Foundation Models not available on this device")
            default:
                // Other errors are fine (e.g., parsing failures)
                break
            }
        }
    }
}
