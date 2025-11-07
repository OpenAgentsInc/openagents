#if os(macOS)
import Foundation

/// Agent provider for Claude Code CLI
public final class ClaudeCodeAgentProvider: CLIAgentProvider {
    // MARK: - State

    /// Track whether a session has been started (for --resume logic)
    private var sessionStarted: [String: Bool] = [:]

    // MARK: - Initialization

    public init() {
        super.init(
            id: .claude_code,
            displayName: "Claude Code",
            binaryName: "claude",
            capabilities: AgentCapabilities(
                executionMode: .cli,
                streamingMode: .text,  // Claude outputs plain text, not JSONL
                supportsResume: true,
                supportsWorkingDirectory: true,
                requiresExternalBinary: true,
                supportsMCP: false
            )
        )
    }

    // MARK: - CLI Arguments

    public override func buildStartArguments(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext
    ) -> [String] {
        let sidStr = sessionId.value
        let started = sessionStarted[sidStr] ?? false

        let args: [String]
        if started {
            // Resume existing Claude session
            args = ["--resume", sidStr, prompt]
            print("[Claude Code] Resume arguments: --resume \(sidStr) \"\(prompt)\"")
        } else {
            // Start new Claude session with specific session ID
            args = ["--session-id", sidStr, prompt]
            print("[Claude Code] Start arguments: --session-id \(sidStr) \"\(prompt)\"")
            sessionStarted[sidStr] = true
        }

        return args
    }

    public override func buildResumeArguments(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext
    ) -> [String] {
        let sidStr = sessionId.value

        // Always use --resume for subsequent prompts
        let args = ["--resume", sidStr, prompt]
        print("[Claude Code] Resume arguments: --resume \(sidStr) \"\(prompt)\"")

        return args
    }

    // MARK: - Override start() to track session state

    public override func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        let handle = try await super.start(
            sessionId: sessionId,
            prompt: prompt,
            context: context,
            updateHub: updateHub
        )

        // Mark session as started so subsequent prompts use --resume
        sessionStarted[sessionId.value] = true

        return handle
    }

    // MARK: - Output Processing

    public override func processStdoutLine(
        _ line: String,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) async {
        // Claude Code outputs plain text, not JSONL
        // Stream as agent message chunks
        let trimmed = line.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        print("[Claude Code] stdout: \(trimmed)")

        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: trimmed)))
        await updateHub.sendSessionUpdate(
            sessionId: sessionId,
            update: .agentMessageChunk(chunk)
        )
    }

    // MARK: - Cleanup

    public override func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async {
        // Clear session state
        sessionStarted.removeValue(forKey: sessionId.value)

        // Call super to handle process termination
        await super.cancel(sessionId: sessionId, handle: handle)
    }
}
#endif
