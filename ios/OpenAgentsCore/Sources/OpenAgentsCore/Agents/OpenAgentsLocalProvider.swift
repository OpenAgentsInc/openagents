// OpenAgentsLocalProvider.swift
// Native provider that uses on-device capabilities (or a simple fallback)

#if os(macOS)
import Foundation

public final class OpenAgentsLocalProvider: AgentProvider, @unchecked Sendable {
    public let id: ACPSessionModeId = .default_mode
    public let displayName: String = "OpenAgents"
    public let capabilities: AgentCapabilities = .init(
        executionMode: .native,
        streamingMode: .acp,
        supportsResume: true,
        supportsWorkingDirectory: true,
        requiresExternalBinary: false,
        supportsMCP: false
    )

    private var cancelled: Set<String> = []

    public init() {}

    public func isAvailable() async -> Bool {
        // Always available with a deterministic fallback; if Apple models are present,
        // they will be used by future enhancements without changing the contract.
        return true
    }

    public func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        let handle = AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
        await streamResponse(prompt: prompt, sessionId: sessionId, updateHub: updateHub)
        return handle
    }

    public func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        await streamResponse(prompt: prompt, sessionId: sessionId, updateHub: updateHub)
    }

    public func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
        cancelled.insert(sessionId.value)
    }

    // MARK: - Internal
    private func streamResponse(prompt: String, sessionId: ACPSessionId, updateHub: SessionUpdateHub) async {
        if await isCancelled(sessionId) { return }

        // Placeholder deterministic local generation. In a follow-up, swap to SystemLanguageModel
        // when available on macOS 15+ to produce a proper completion.
        let reply = makeLocalReply(for: prompt)
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: reply)))
        await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
    }

    private func isCancelled(_ sessionId: ACPSessionId) async -> Bool {
        return cancelled.contains(sessionId.value)
    }

    private func makeLocalReply(for prompt: String) -> String {
        // Very small heuristic to feel responsive while we wire up Apple models.
        let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "OpenAgents (local): How can I help?" }
        return "OpenAgents (local): \(trimmed)"
    }
}
#endif

