// OpenAgentsLocalProvider.swift
// Native provider that uses on-device Foundation Models when available,
// with a deterministic local fallback. Streams text via ACP agentMessageChunk.

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

    public func isAvailable() async -> Bool { true }

    public func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        let handle = AgentHandle(sessionId: sessionId, mode: id, isStarted: true)
        await streamResponse(prompt: prompt, sessionId: sessionId, updateHub: updateHub, context: context)
        return handle
    }

    public func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws { await streamResponse(prompt: prompt, sessionId: sessionId, updateHub: updateHub, context: context) }

    public func cancel(sessionId: ACPSessionId, handle: AgentHandle) async {
        cancelled.insert(sessionId.value)
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, * ) {
            OpenAgentsLocalProvider.fmSessions.removeValue(forKey: sessionId.value)
        }
        #endif
    }

    // MARK: - Internal
    private func streamResponse(prompt: String, sessionId: ACPSessionId, updateHub: SessionUpdateHub, context: AgentContext?) async {
        if await isCancelled(sessionId) { return }

        // Use Foundation Models if available, otherwise fall back to local reply
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) {
            do {
                let wd = context?.workingDirectory?.path
                try await fmStream(prompt: prompt, sessionId: sessionId, updateHub: updateHub, workspaceRoot: wd, server: context?.server)
                return
            } catch { /* fallback below */ }
        }
        #endif

        // Deterministic fallback
        let reply = makeLocalReply(for: prompt)
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: reply)))
        await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
    }

    private func isCancelled(_ sessionId: ACPSessionId) async -> Bool { cancelled.contains(sessionId.value) }

    private func makeLocalReply(for prompt: String) -> String {
        let trimmed = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "We are OpenAgents. How can we help?" }
        return "We are OpenAgents. \(trimmed)"
    }
}

#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, * )
extension OpenAgentsLocalProvider {
    private static var fmSessions: [String: LanguageModelSession] = [:]

    private func ensureSession(for sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?, server: DesktopWebSocketServer?) async throws -> LanguageModelSession {
        if let s = Self.fmSessions[sessionId.value] { return s }
        let model = SystemLanguageModel.default
        switch model.availability { case .available: break; default:
            throw NSError(domain: "OpenAgentsLocalProvider", code: -10, userInfo: [NSLocalizedDescriptionKey: "FM unavailable"]) }
        let instructions = Instructions("""
        You are OpenAgents, a helpful assistant that can delegate coding tasks to specialized agents.

        You can respond conversationally to questions about yourself and your capabilities.
        You also have a tool called delegate.run that routes tasks to Codex or Claude Code.

        USE delegate.run when:
        - User wants to work with code/files (list, read, write, search, analyze)
        - User explicitly mentions "codex", "claude code", or "delegate"
        - User wants help with a coding task

        RESPOND CONVERSATIONALLY when:
        - User asks who you are or what you can do
        - User greets you or chats casually
        - User asks general questions not requiring code work

        Examples:

        User: who are you
        You: I'm OpenAgents, an assistant that helps with coding tasks. I can delegate work to Codex or Claude Code when you need help with code.

        User: what can you do
        You: I can help with coding tasks by delegating to specialized agents like Codex and Claude Code. Just tell me what you'd like to work on.

        User: list files in the project
        You: (call delegate.run with user_prompt="list files in the project")

        User: delegate to codex: refactor auth module
        You: (call delegate.run with provider="codex", user_prompt="refactor auth module")
        """)
        // Register delegate tool for routing to external agents
        var tools: [any Tool] = []
        tools.append(FMTool_DelegateRun(sessionId: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot, server: server))
        let s = LanguageModelSession(model: model, tools: tools, instructions: instructions)
        s.prewarm(promptPrefix: nil)
        Self.fmSessions[sessionId.value] = s
        return s
    }

    private func fmStream(prompt: String, sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?, server: DesktopWebSocketServer?) async throws {
        let session = try await ensureSession(for: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot, server: server)
        let options = GenerationOptions(temperature: 0.7, maximumResponseTokens: 200)

        // Use respond() not streamResponse() - respond() properly invokes tools
        print("[FM] Calling respond(to:) with prompt: \(prompt.prefix(100))...")
        let response = try await session.respond(to: prompt, options: options)
        print("[FM] Response received (\(response.content.count) chars)")
        print("[FM] Response content: \(response.content.prefix(300))")

        // Check transcript for tool calls
        print("[FM] Transcript has \(session.transcript.count) entries")
        for (index, entry) in session.transcript.enumerated() {
            let desc = String(describing: entry).prefix(200)
            print("[FM] Transcript[\(index)]: \(desc)")
        }

        // Send response as agent message
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: response.content)))
        await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
    }

    private func extractText(from snapshot: Any) -> String? {
        let mirror = Mirror(reflecting: snapshot)
        if let content = mirror.children.first(where: { $0.label == "content" })?.value as? String { return content }
        let desc = String(describing: snapshot)
        return FoundationModelSummarizer_extractFromDescription(desc)
    }

    // MARK: - FM Tool: delegate.run
    struct FMTool_DelegateRun: Tool {
        let name = "delegate.run"
        let description = "Delegate a coding task to Codex or Claude Code."

        // Stored properties for dependencies (Pattern B from FMTools.swift)
        private let sessionId: ACPSessionId
        private let updateHub: SessionUpdateHub
        private let workspaceRoot: String?
        private let server: DesktopWebSocketServer?

        init(sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?, server: DesktopWebSocketServer?) {
            self.sessionId = sessionId
            self.updateHub = updateHub
            self.workspaceRoot = workspaceRoot
            self.server = server
        }

        typealias Output = String

        @Generable
        struct Arguments {
            @Guide(description: "The task to delegate") var user_prompt: String
            @Guide(description: "Provider: codex or claude_code") var provider: String?
            @Guide(description: "Brief description of delegation") var description: String?
        }

        func call(arguments a: Arguments) async throws -> Output {
            // For now, just acknowledge the call - we'll wire up delegation next
            let provider = a.provider ?? "codex"
            let desc = a.description ?? "delegation"
            return "delegate.run called: provider=\(provider) desc=\(desc) prompt=\(a.user_prompt.prefix(50))"
        }

        // Static method for composing delegation prompts (used by tests)
        static func composeDelegationPrompt(
            provider: String,
            description: String,
            userPrompt: String,
            workspaceRoot: String?,
            includeGlobs: [String]?,
            summarize: Bool?,
            maxFiles: Int?
        ) -> String {
            var lines: [String] = []
            lines.append("OpenAgents → \(provider) delegation")
            lines.append("Description: \(description)")
            if let ws = workspaceRoot {
                lines.append("Workspace: \(ws)")
            }
            if let globs = includeGlobs, !globs.isEmpty {
                lines.append("Include: \(globs.joined(separator: ", "))")
            }
            if let sum = summarize {
                lines.append("Summarize: \(sum ? "yes" : "no")")
            }
            if let max = maxFiles {
                lines.append("Max files: \(max)")
            }
            lines.append("")
            lines.append(userPrompt)
            return lines.joined(separator: "\n")
        }
    }

}
#endif
#endif
