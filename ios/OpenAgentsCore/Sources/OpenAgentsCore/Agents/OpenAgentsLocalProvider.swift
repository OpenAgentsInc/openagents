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
        You are OpenAgents. Respond concisely (2-3 sentences max). Always use first-person plural ("We", not "I").

        - When asked who you are: "We are OpenAgents."
        - For coding tasks (read/write files, generate code, analyze repos): use delegate.run tool immediately without announcing it
        - For meta questions about capabilities: answer directly, do not call tools
        - Mentioning an agent name is NOT a reason to call tools; only call when execution is needed

        Examples:
        User: who are you?
        You: We are OpenAgents. Ready to assist.

        User: what can you do?
        You: We command other agents and help with coding tasks.

        User: list files in the workspace
        You: (immediately call delegate.run with appropriate arguments)
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
        var last = ""
        let stream = session.streamResponse(to: prompt, options: options)
        for try await snapshot in stream {
            if await isCancelled(sessionId) { break }
            if let text = extractText(from: snapshot) {
                // Filter out tool call syntax that FM outputs as text
                let cleaned = filterToolCallSyntax(text)
                let delta = cleaned.hasPrefix(last) ? String(cleaned.dropFirst(last.count)) : cleaned
                last = cleaned
                if !delta.isEmpty {
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: delta)))
                    await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
                }
            }
        }
    }

    private func filterToolCallSyntax(_ text: String) -> String {
        // Remove FM's internal tool call syntax that sometimes leaks into text stream
        // Pattern: [{"name": "tool", "arguments": {...}}]```<executable_end>
        var cleaned = text

        // Remove <executable_end> tags
        cleaned = cleaned.replacingOccurrences(of: "```<executable_end>", with: "")
        cleaned = cleaned.replacingOccurrences(of: "<executable_end>", with: "")

        // Remove inline JSON tool call arrays
        let toolCallPattern = #"\[\{"name":\s*"[^"]+",\s*"arguments":\s*\{[^\]]*\}\}\]"#
        if let regex = try? NSRegularExpression(pattern: toolCallPattern, options: []) {
            let range = NSRange(cleaned.startIndex..., in: cleaned)
            cleaned = regex.stringByReplacingMatches(in: cleaned, options: [], range: range, withTemplate: "")
        }

        return cleaned
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
        let description = "Route a concrete coding task to a configured agent provider when execution is needed. Not for meta questions about capabilities; answer those inline."
        typealias Output = String

        private let sessionId: ACPSessionId
        private let updateHub: SessionUpdateHub
        private let workspaceRoot: String?
        private weak var server: DesktopWebSocketServer?

        init(sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?, server: DesktopWebSocketServer?) {
            self.sessionId = sessionId
            self.updateHub = updateHub
            self.workspaceRoot = workspaceRoot
            self.server = server
        }

        @Generable
        struct Arguments {
            @Guide(description: "Provider: auto (default), codex, gptoss, local_fm, custom:<id>") var provider: String?
            @Guide(description: "Task type (e.g., delegate, search, run, generate)") var task: String?
            @Guide(description: "Short one-liner description for the UI") var description: String?
            @Guide(description: "The exact instruction to pass to the provider") var user_prompt: String
            @Guide(description: "Workspace root (absolute path)") var workspace_root: String?
            @Guide(description: "Include patterns (glob)") var files_include_glob: [String]?
            @Guide(description: "Request a succinct summary upon completion") var summarize: Bool?
            @Guide(description: "Upper bound for files to scan/edit") var max_files: Int?
            @Guide(description: "Priority: low, normal, high (advisory)") var priority: String?
            @Guide(description: "Soft time budget in milliseconds") var time_budget_ms: Int?
            @Guide(description: "Emit tool_call without dispatching (preview mode)") var dry_run: Bool?
        }

        func call(arguments a: Arguments) async throws -> Output {
            let callId = UUID().uuidString

            // Resolve provider (auto defaults to codex for now)
            let resolvedProvider = resolveProvider(requested: a.provider)

            // Build args dict for ACP tool_call
            var args: [String: AnyEncodable] = [:]
            args["provider"] = AnyEncodable(resolvedProvider)
            if let t = a.task { args["task"] = AnyEncodable(t) }
            if let d = a.description { args["description"] = AnyEncodable(d) }
            args["user_prompt"] = AnyEncodable(a.user_prompt)
            if let wr = a.workspace_root ?? workspaceRoot { args["workspace_root"] = AnyEncodable(wr) }
            if let inc = a.files_include_glob { args["files_include_glob"] = AnyEncodable(inc) }
            if let s = a.summarize { args["summarize"] = AnyEncodable(s) }
            if let m = a.max_files { args["max_files"] = AnyEncodable(m) }
            if let p = a.priority { args["priority"] = AnyEncodable(p) }
            if let tb = a.time_budget_ms { args["time_budget_ms"] = AnyEncodable(tb) }
            if let dr = a.dry_run { args["dry_run"] = AnyEncodable(dr) }

            // Emit ACP tool_call for UI visibility
            let call = ACPToolCallWire(call_id: callId, name: name, arguments: args)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(call))

            // Handle dry-run mode (preview only)
            if a.dry_run == true {
                return "delegate.run dry-run (not dispatched)"
            }

            // Apply workspace root to server working directory if supplied
            if let wr = a.workspace_root ?? workspaceRoot, let srv = self.server {
                srv.workingDirectory = URL(fileURLWithPath: wr)
            }

            // Route to the resolved provider
            guard let server = self.server else {
                return "delegate.run failed: server unavailable"
            }

            let mode = modeForProvider(resolvedProvider)
            await server.localSessionSetMode(sessionId: sessionId, mode: mode)

            let text = Self.composeDelegationPrompt(
                provider: resolvedProvider,
                description: a.description,
                userPrompt: a.user_prompt,
                workspaceRoot: a.workspace_root ?? workspaceRoot,
                includeGlobs: a.files_include_glob,
                summarize: a.summarize,
                maxFiles: a.max_files
            )
            let req = ACP.Agent.SessionPromptRequest(session_id: sessionId, content: [.text(.init(text: text))])
            try? await server.localSessionPrompt(request: req)

            return "delegate.run dispatched to \(resolvedProvider)"
        }

        private func resolveProvider(requested: String?) -> String {
            let provider = requested ?? "auto"
            if provider == "auto" {
                // Default to codex for now; could add availability checks here
                return "codex"
            }
            return provider
        }

        private func modeForProvider(_ provider: String) -> ACPSessionModeId {
            switch provider {
            case "codex": return .codex
            case "claude_code": return .claude_code
            default: return .codex  // fallback
            }
        }

        // MARK: - Delegation heuristics (disabled)
        // Keeping for reference; not used in "agent decides" mode.
        // private func shouldDispatch(userPrompt: String, task: String?) -> Bool {
        //     let p = userPrompt.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        //     let directivePrefixes = [
        //         "delegate to codex:", "codex:", "send to codex:", "run on codex:", "run in codex:", "run with codex:"
        //     ]
        //     if directivePrefixes.contains(where: { p.hasPrefix($0) }) { return true }
        //     let directivePhrases = [
        //         "tell codex to", "ask codex to", "have codex ", "use codex to", "delegate to codex", "send this to codex"
        //     ]
        //     if directivePhrases.contains(where: { p.contains($0) }) { return true }
        //     if let t = task?.lowercased(), ["delegate", "run", "execute"].contains(t) {
        //         let looksQuestion = p.hasPrefix("can you") || p.hasPrefix("can codex") || p.hasPrefix("what can") || p.hasPrefix("are you able") || p.hasPrefix("how ")
        //         if !looksQuestion { return true }
        //     }
        //     return false
        // }

        static func composeDelegationPrompt(provider: String, description: String?, userPrompt: String, workspaceRoot: String?, includeGlobs: [String]?, summarize: Bool?, maxFiles: Int?) -> String {
            var parts: [String] = []
            parts.append("OpenAgents → \(provider) delegation")
            if let d = description, !d.isEmpty { parts.append("Description: \(d)") }
            if let wr = workspaceRoot { parts.append("Workspace: \(wr)") }
            if let inc = includeGlobs, !inc.isEmpty { parts.append("Include: \(inc.joined(separator: ", "))") }
            if let s = summarize { parts.append("Summarize: \(s ? "yes" : "no")") }
            if let m = maxFiles { parts.append("Max files: \(m)") }
            parts.append("")
            parts.append(userPrompt)
            return parts.joined(separator: "\n")
        }
    }
}
#endif
#endif
