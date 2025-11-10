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

        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) {
            do {
                let wd = context?.workingDirectory?.path
                try await fmStream(prompt: prompt, sessionId: sessionId, updateHub: updateHub, workspaceRoot: wd)
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

    private func ensureSession(for sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?) async throws -> LanguageModelSession {
        if let s = Self.fmSessions[sessionId.value] { return s }
        let model = SystemLanguageModel.default
        switch model.availability { case .available: break; default:
            throw NSError(domain: "OpenAgentsLocalProvider", code: -10, userInfo: [NSLocalizedDescriptionKey: "FM unavailable"]) }
        let instructions = Instructions("""
        You are OpenAgents. Respond with 2-3 sentences.
        
        - Identify as \"We are OpenAgents.\" when asked who you are. Always respond in the first-person plural ("We ___", not "I ___".)

        Examples:
        Q: who are you?
        A: We are OpenAgents. Ready to assist.

        Q: what can you do?
        A: We command other agents. How can we help?

        Q: how do i start?
        A: Ask me to issue commands to Claude Code or Codex.
        """)
        // Register minimal tools for chat; allow the model to decide to delegate to Codex
        var tools: [any Tool] = []
        tools.append(FMTool_CodexRun(sessionId: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot))
        let s = LanguageModelSession(model: model, tools: tools, instructions: instructions)
        s.prewarm(promptPrefix: nil)
        Self.fmSessions[sessionId.value] = s
        return s
    }

    private func fmStream(prompt: String, sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?) async throws {
        let session = try await ensureSession(for: sessionId, updateHub: updateHub, workspaceRoot: workspaceRoot)
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
        let description = "Delegate a task to the Codex agent. Provide a concise description and the user's prompt."
        typealias Output = String

        private let sessionId: ACPSessionId
        private let updateHub: SessionUpdateHub
        private let workspaceRoot: String?

        init(sessionId: ACPSessionId, updateHub: SessionUpdateHub, workspaceRoot: String?) {
            self.sessionId = sessionId
            self.updateHub = updateHub
            self.workspaceRoot = workspaceRoot
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

            // Emit ACP tool_call
            let call = ACPToolCallWire(call_id: callId, name: name, arguments: args)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(call))

            // Emit a stubbed completion update so UI shows output; real execution can be wired later
            let argsJSON: String = {
                if let data = try? JSONEncoder().encode(args), let s = String(data: data, encoding: .utf8) { return s }
                return "{}"
            }()
            let output: [String: AnyEncodable] = [
                "status": AnyEncodable("requested"),
                "command": AnyEncodable("codex exec --json"),
                "arguments_json": AnyEncodable(argsJSON)
            ]
            let upd = ACPToolCallUpdateWire(call_id: callId, status: .completed, output: AnyEncodable(output), error: nil)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(upd))

            return "Delegated to Codex with provided parameters."
        }
    }
}
#endif
#endif
