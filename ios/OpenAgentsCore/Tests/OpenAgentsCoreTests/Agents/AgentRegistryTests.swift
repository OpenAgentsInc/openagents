#if os(macOS)
import XCTest
@testable import OpenAgentsCore

final class AgentRegistryTests: XCTestCase {
    func testRegisterLookupAndUnregister() async throws {
        let registry = AgentRegistry()
        let provider = MockAgentProvider(
            id: .codex,
            displayName: "Codex CLI",
            availability: true
        )

        await registry.register(provider)
        let fetched = await registry.provider(for: .codex)
        XCTAssertEqual(fetched?.displayName, "Codex CLI")

        let allProviders = await registry.allProviders()
        XCTAssertEqual(allProviders.count, 1)
        XCTAssertEqual(allProviders.first?.id, .codex)

        await registry.unregister(.codex)
        XCTAssertNil(await registry.provider(for: .codex))
    }

    func testAvailableProvidersAndDisplayNames() async throws {
        let registry = AgentRegistry()
        let available = MockAgentProvider(
            id: .codex,
            displayName: "Codex CLI",
            availability: true
        )
        let unavailable = MockAgentProvider(
            id: .claude_code,
            displayName: "Claude Code",
            availability: false
        )

        await registry.register(available)
        await registry.register(unavailable)

        let providers = await registry.availableProviders()
        XCTAssertEqual(providers.count, 1)
        XCTAssertEqual(providers.first?.id, .codex)
        XCTAssertTrue(await registry.isAvailable(.codex))
        XCTAssertFalse(await registry.isAvailable(.claude_code))

        let displayNames = await registry.availableDisplayNames()
        XCTAssertEqual(displayNames, ["Codex CLI"])
        XCTAssertEqual(
            await registry.modeId(forDisplayName: "codex cli"),
            .codex
        )
    }

    func testHandleLifecycle() async {
        let registry = AgentRegistry()
        let sessionId = ACPSessionId("session-1")
        let handle = AgentHandle(
            sessionId: sessionId,
            mode: .codex,
            processId: 42,
            threadId: "thread-1",
            isStarted: true,
            metadata: ["env": "dev"]
        )

        await registry.setHandle(handle, for: sessionId)
        XCTAssertEqual(await registry.handle(for: sessionId)?.processId, 42)
        XCTAssertEqual(await registry.allHandles()[sessionId.value]?.threadId, "thread-1")

        await registry.removeHandle(for: sessionId)
        XCTAssertNil(await registry.handle(for: sessionId))
    }

    func testCapabilitiesLookup() async {
        let registry = AgentRegistry()
        let caps = AgentCapabilities(
            executionMode: .cli,
            streamingMode: .jsonl,
            supportsResume: true,
            supportsWorkingDirectory: true,
            requiresExternalBinary: true,
            supportsMCP: true
        )
        let provider = MockAgentProvider(
            id: .claude_code,
            displayName: "Claude Code",
            capabilities: caps,
            availability: true
        )

        await registry.register(provider)
        let lookedUp = await registry.capabilities(for: .claude_code)
        XCTAssertEqual(lookedUp?.supportsResume, true)
        XCTAssertEqual(lookedUp?.supportsMCP, true)
    }
}

// MARK: - Test Doubles

private final actor MockAgentProvider: AgentProvider {
    nonisolated let id: ACPSessionModeId
    nonisolated let displayName: String
    nonisolated let capabilities: AgentCapabilities
    nonisolated let availability: Bool

    init(
        id: ACPSessionModeId,
        displayName: String,
        capabilities: AgentCapabilities = AgentCapabilities(
            executionMode: .cli,
            streamingMode: .jsonl
        ),
        availability: Bool
    ) {
        self.id = id
        self.displayName = displayName
        self.capabilities = capabilities
        self.availability = availability
    }

    func isAvailable() async -> Bool {
        availability
    }

    func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        throw AgentProviderError.unsupported("not needed in tests")
    }

    func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        throw AgentProviderError.unsupported("not needed in tests")
    }

    func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async {}
}
#endif
