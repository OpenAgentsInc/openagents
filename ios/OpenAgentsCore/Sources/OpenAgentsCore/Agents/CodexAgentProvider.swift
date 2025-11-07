#if os(macOS)
import Foundation

/// Agent provider for OpenAI Codex CLI
public final class CodexAgentProvider: CLIAgentProvider {
    // MARK: - State

    /// Codex thread IDs by session (for resume)
    private var threadIdBySession: [String: String] = [:]

    // MARK: - Initialization

    public init() {
        super.init(
            id: .codex,
            displayName: "Codex",
            binaryName: "codex",
            envOverride: "OPENAGENTS_CODEX_CLI",
            capabilities: AgentCapabilities(
                executionMode: .cli,
                streamingMode: .jsonl,
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
        var args: [String] = ["exec", "--json"]

        // Working directory
        if let wd = context.workingDirectory?.path {
            args += ["--cd", wd]
        }

        // Check for existing thread to resume
        let sidStr = sessionId.value
        if let thread = threadIdBySession[sidStr], !thread.isEmpty {
            args += ["resume", thread]
        }

        // Prompt
        args.append(prompt)

        print("[Codex] Arguments: \(args.joined(separator: " "))")
        return args
    }

    public override func buildResumeArguments(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext
    ) -> [String] {
        var args: [String] = ["exec", "--json"]

        // Working directory
        if let wd = context.workingDirectory?.path {
            args += ["--cd", wd]
        }

        // Resume with thread ID
        let sidStr = sessionId.value
        if let thread = threadIdBySession[sidStr], !thread.isEmpty {
            args += ["resume", thread]
        } else if let thread = handle.threadId, !thread.isEmpty {
            args += ["resume", thread]
        }

        // Prompt
        args.append(prompt)

        print("[Codex] Resume arguments: \(args.joined(separator: " "))")
        return args
    }

    // MARK: - Output Processing

    public override func processStdoutLine(
        _ line: String,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) async {
        // Parse Codex JSONL and translate to ACP
        guard let data = line.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return
        }

        let type = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        print("[Codex] Event: \(type)")

        // Extract thread ID for resume capability
        if type == "thread.started" || type == "session_meta" {
            let sidStr = sessionId.value
            if let threadId = obj["thread_id"] as? String {
                threadIdBySession[sidStr] = threadId
            } else if let payload = obj["payload"] as? [String: Any],
                      let threadId = payload["id"] as? String {
                threadIdBySession[sidStr] = threadId
            }
        }

        // For now, simple handling: extract any text and stream as agent chunks
        // TODO: Full ACP translation using CodexAcpTranslator
        let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any])
        if let text = extractText(from: item ?? obj), !text.isEmpty {
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: text)))
            await updateHub.sendSessionUpdate(
                sessionId: sessionId,
                update: .agentMessageChunk(chunk)
            )
        }
    }

    // MARK: - Parsing Helpers

    private func extractText(from dict: [String: Any]) -> String? {
        // Try various text field names
        if let text = dict["text"] as? String { return text }
        if let text = dict["content"] as? String { return text }
        if let text = dict["message"] as? String { return text }

        // Try nested content
        if let content = dict["content"] as? [String: Any],
           let text = content["text"] as? String {
            return text
        }

        return nil
    }

    // MARK: - Cleanup

    public override func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async {
        // Clear thread ID
        threadIdBySession.removeValue(forKey: sessionId.value)

        // Call super to handle process termination
        await super.cancel(sessionId: sessionId, handle: handle)
    }
}
#endif
