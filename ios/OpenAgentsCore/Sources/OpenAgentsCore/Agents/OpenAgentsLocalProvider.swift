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
    #if os(macOS)
    private let gptossManager = GPTOSSModelManager()
    #endif

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

        // Preferred path: GPT‑OSS if installed/available on macOS
        #if os(macOS)
        do {
            let info = await gptossManager.verifyLocalSnapshot()
            if info.ok {
                OpenAgentsLog.orchestration.info("OpenAgentsLocalProvider: GPT‑OSS path selected (snapshot verified \(info.shardCount)/\(info.expectedShardCount))")
                // Emit a small placeholder so UI shows immediate activity
                let starting = ACP.Client.ContentChunk(content: .text(.init(text: "⏳ Loading GPT‑OSS 20B…")))
                await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(starting))
                try await gptossManager.loadModel()
                OpenAgentsLog.orchestration.info("OpenAgentsLocalProvider: GPT‑OSS model loaded; starting stream")
                var total = 0
                try await gptossManager.stream(prompt: prompt) { delta in
                    total += delta.count
                    if await self.isCancelled(sessionId) { return }
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: delta)))
                    await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
                }
                OpenAgentsLog.orchestration.info("OpenAgentsLocalProvider: GPT‑OSS stream complete chars=\(total)")
                return
            } else {
                OpenAgentsLog.orchestration.info("OpenAgentsLocalProvider: GPT‑OSS incomplete snapshot; trying Foundation Models or local fallback")
            }
        } catch {
            let msg = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            OpenAgentsLog.orchestration.error("OpenAgentsLocalProvider: GPT‑OSS path failed: \(msg). Falling back to FM/local.")
            // Surface the reason to the chat timeline
            let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ GPT‑OSS unavailable: \(msg)\nOpen the GPT‑OSS card in the sidebar and click Download/Repair.")))
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
        }
        #endif

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
        You are OpenAgents. Respond with 2-3 sentences.
        
        - Identify as \"We are OpenAgents.\" when asked who you are. Always respond in the first-person plural ("We ___", not "I ___".)
        - Decision rubric for tools:
          • Use tools only for actionable coding tasks (read/write files, run searches, generate code/tests, repo analysis).
          • Do NOT call tools to answer meta/capability questions (e.g., "who can you delegate to?", "what can you do?", "can you command codex?"). Answer those inline.
          • Mentioning an agent by name is NOT a reason to use a tool. Choose a tool only when execution is actually needed to satisfy the user's task.

        Examples:
        Q: who are you?
        A: We are OpenAgents. Ready to assist.

        Q: what can you do?
        A: We command other agents and can help with coding tasks. (No tool calls.)

        Q: who can you delegate to?
        A: We can delegate to Codex and Claude Code. (No tool calls.)
        """)
        // Register minimal tools for chat; allow the model to decide to delegate to Codex
        var tools: [any Tool] = []
        tools.append(FMTool_CodexRun(sessionId: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot, server: server))
        let s = LanguageModelSession(model: model, tools: tools, instructions: instructions)
        s.prewarm(promptPrefix: nil)
        Self.fmSessions[sessionId.value] = s
        return s
    }

    private func fmStream(prompt: String, sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?, server: DesktopWebSocketServer?) async throws {
        let session = try await ensureSession(for: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot, server: server)
        let options = GenerationOptions(temperature: 0.15, maximumResponseTokens: 140)
        var last = ""
        let stream = session.streamResponse(to: prompt, options: options)
        for try await snapshot in stream {
            if await isCancelled(sessionId) { break }
            if let text = extractText(from: snapshot) {
                let delta = text.hasPrefix(last) ? String(text.dropFirst(last.count)) : text
                last = text
                if !delta.isEmpty {
                    let chunk = ACP.Client.ContentChunk(content: .text(.init(text: delta)))
                    await updateHub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
                }
            }
        }
    }

    private func extractText(from snapshot: Any) -> String? {
        let mirror = Mirror(reflecting: snapshot)
        if let content = mirror.children.first(where: { $0.label == "content" })?.value as? String { return content }
        let desc = String(describing: snapshot)
        return FoundationModelSummarizer_extractFromDescription(desc)
    }

    // MARK: - FM Tool: codex.run
        struct FMTool_CodexRun: Tool {
        let name = "codex.run"
        let description = "Delegate a concrete coding task to the Codex agent. Not for questions about capabilities or availability; answer those inline. Provide a concise task description and the user's actionable prompt."
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
            @Guide(description: "Task type (e.g., delegate, search, run)") var task: String?
            @Guide(description: "Short description of what Codex should do") var description: String?
            @Guide(description: "The user's prompt to pass through") var user_prompt: String
            @Guide(description: "Workspace root (absolute path)") var workspace_root: String?
            @Guide(description: "Include patterns (glob)") var files_include_glob: [String]?
            @Guide(description: "Summarize results for user") var summarize: Bool?
            @Guide(description: "File limit") var max_files: Int?
        }

        func call(arguments a: Arguments) async throws -> Output {
            let callId = UUID().uuidString
            var args: [String: AnyEncodable] = [:]
            args["provider"] = AnyEncodable("codex")
            if let t = a.task { args["task"] = AnyEncodable(t) } else { args["task"] = AnyEncodable("delegate") }
            if let d = a.description { args["description"] = AnyEncodable(d) }
            args["user_prompt"] = AnyEncodable(a.user_prompt)
            if let wr = a.workspace_root ?? workspaceRoot { args["workspace_root"] = AnyEncodable(wr) }
            if let inc = a.files_include_glob { args["files_include_glob"] = AnyEncodable(inc) }
            if let s = a.summarize { args["summarize"] = AnyEncodable(s) }
            if let m = a.max_files { args["max_files"] = AnyEncodable(m) }

            // Gating (disabled): previously required explicit delegation phrases.
            // Keeping code for reference while running in "agent decides" mode.
            // if !shouldDispatch(userPrompt: a.user_prompt, task: a.task) {
            //     return "codex.run declined: no explicit delegation"
            // }

            // Apply workspace root to server working directory if supplied
            if let wr = a.workspace_root ?? workspaceRoot, let srv = self.server {
                srv.workingDirectory = URL(fileURLWithPath: wr)
            }

            // Emit ACP tool_call for UI visibility only when dispatching
            let call = ACPToolCallWire(call_id: callId, name: name, arguments: args)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(call))

            // Trigger real Codex run via DesktopWebSocketServer local helpers
            if let server = self.server {
                await server.localSessionSetMode(sessionId: sessionId, mode: .codex)
                let text = Self.composeDelegationPrompt(description: a.description, userPrompt: a.user_prompt, workspaceRoot: a.workspace_root ?? workspaceRoot, includeGlobs: a.files_include_glob, summarize: a.summarize, maxFiles: a.max_files)
                let req = ACP.Agent.SessionPromptRequest(session_id: sessionId, content: [.text(.init(text: text))])
                try? await server.localSessionPrompt(request: req)
            }

            return "codex.run dispatched"
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
        
        private static func composeDelegationPrompt(description: String?, userPrompt: String, workspaceRoot: String?, includeGlobs: [String]?, summarize: Bool?, maxFiles: Int?) -> String {
            var parts: [String] = []
            parts.append("OpenAgents → Codex delegation")
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
