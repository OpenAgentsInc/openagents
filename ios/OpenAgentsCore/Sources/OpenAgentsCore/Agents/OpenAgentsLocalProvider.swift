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

        When the user wants to work with code/files (list, read, write, search, analyze, refactor, etc.),
        use the delegate.run tool to route the task to Codex or Claude Code.

        For general conversation, introductions, or capability questions, respond conversationally.
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

        // Send response as agent message with _meta indicating it's from FM orchestrator
        let chunk = ACP.Client.ContentChunk(
            content: .text(.init(text: response.content)),
            _meta: [
                "source": AnyEncodable("fm_orchestrator"),
                "provider": AnyEncodable("foundation_models")
            ]
        )
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
        let name = ToolName.delegate.rawValue
        let description = "Route a coding task to specialized agents (Codex or Claude Code) for execution. Use this for any file operations, code analysis, refactoring, or workspace exploration."

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
            @Guide(description: "The specific coding task to perform (e.g., 'list files', 'refactor auth module')") var user_prompt: String
            @Guide(description: "Which agent to use: 'codex' or 'claude_code' (default: codex)") var provider: String?
            @Guide(description: "Optional brief description of the delegation") var description: String?
        }

        func call(arguments a: Arguments) async throws -> Output {
            let providerName = a.provider ?? "codex"
            let modeId: ACPSessionModeId = (providerName.lowercased() == "claude_code" || providerName.lowercased() == "claude-code") ? .claude_code : .codex

            guard let server = self.server else {
                return "❌ Server not available for delegation"
            }

            let updateHub = self.updateHub  // Not optional

            // Emit a tool call update to the UI showing delegation is happening
            let toolCallWire = ACPToolCallWire(
                call_id: UUID().uuidString,
                name: ToolName.delegate.rawValue,
                arguments: [
                    "user_prompt": AnyEncodable(a.user_prompt),
                    "provider": AnyEncodable(providerName),
                    "description": AnyEncodable(a.description)
                ]
            )
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(toolCallWire))

            // Use the SAME sessionId so responses appear in the current conversation
            // Switch the mode for this session to the delegated provider
            await server.localSessionSetMode(sessionId: sessionId, mode: modeId)

            // Send the prompt to the delegated provider in the current session
            let contentBlock = ACP.Client.ContentBlock.text(.init(text: a.user_prompt))
            let promptRequest = ACP.Agent.SessionPromptRequest(
                session_id: sessionId,
                content: [contentBlock]
            )

            // Start the delegation in the background (don't await - let it stream naturally)
            Task {
                do {
                    try await server.localSessionPrompt(request: promptRequest)
                    // After delegation completes, switch back to orchestrator mode
                    // so the next user message goes to Foundation Models, not the delegated agent
                    await server.localSessionSetMode(sessionId: sessionId, mode: .default_mode)
                } catch {
                    let errorChunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ Delegation error: \(error.localizedDescription)")))
                    await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(errorChunk))
                    // Even on error, switch back to orchestrator
                    await server.localSessionSetMode(sessionId: sessionId, mode: .default_mode)
                }
            }

            // Return acknowledgment to Foundation Models
            return "✓ Delegated to \(providerName). Task: \(a.user_prompt)"
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
