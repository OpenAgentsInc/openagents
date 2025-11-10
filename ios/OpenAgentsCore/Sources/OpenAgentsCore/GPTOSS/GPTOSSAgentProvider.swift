#if os(macOS)
import Foundation

public final class GPTOSSAgentProvider: AgentProvider, @unchecked Sendable {
    public let id: ACPSessionModeId = .gptoss_20b
    public let displayName: String = "GPTOSS 20B"
    public let capabilities: AgentCapabilities = .init(
        executionMode: .native,
        streamingMode: .acp,
        supportsResume: true,
        supportsWorkingDirectory: true,
        requiresExternalBinary: false,
        supportsMCP: false
    )

    private let modelManager: GPTOSSModelManager
    private var cancelled: Set<String> = []
    private let config: GPTOSSConfig

    public init(config: GPTOSSConfig = .default) {
        self.config = config
        self.modelManager = GPTOSSModelManager(config: config)
    }

    public func isAvailable() async -> Bool {
        // macOS + 16GB heuristic
        #if os(macOS)
        let mem = ProcessInfo.processInfo.physicalMemory
        return mem >= 16_000_000_000
        #else
        return false
        #endif
    }

    public func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        try await modelManager.loadModel()

        // Phase 1: single response (non-streaming)
        let response = try await modelManager.generate(
            prompt: prompt,
            options: GPTOSSGenerationOptions(
                temperature: config.temperature,
                topP: config.topP,
                maxTokens: config.maxTokens
            )
        )

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: response)))
        await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
        return AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
    }

    public func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        _ = try await start(sessionId: sessionId, prompt: prompt, context: context, updateHub: updateHub)
    }

    public func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
        cancelled.insert(sessionId.value)
    }

    private func isCancelled(_ sessionId: ACPSessionId) -> Bool { cancelled.contains(sessionId.value) }
}
#endif

