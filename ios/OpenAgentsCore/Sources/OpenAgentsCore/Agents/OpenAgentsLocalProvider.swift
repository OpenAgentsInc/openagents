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

        // Stubbed tool-calling demo: if the user mentions "codex", emit a tool call with arguments
        let lower = prompt.lowercased()
        if lower.contains("codex") {
            let callId = UUID().uuidString
            var args: [String: AnyEncodable] = [
                "provider": AnyEncodable("codex"),
                "task": AnyEncodable("delegate"),
                "description": AnyEncodable("OpenAgents → Codex delegation"),
                "user_prompt": AnyEncodable(prompt),
                "files_include_glob": AnyEncodable(["**/*"]),
                "summarize": AnyEncodable(true),
                "max_files": AnyEncodable(5000)
            ]
            if let wd = context?.workingDirectory?.path, !wd.isEmpty {
                args["workspace_root"] = AnyEncodable(wd)
            }
            let call = ACPToolCallWire(call_id: callId, name: "codex.run", arguments: args)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCall(call))

            // Immediately follow with a stubbed completion update so UI shows output/args
            let argsJSON: String = {
                if let data = try? JSONEncoder().encode(args), let s = String(data: data, encoding: .utf8) { return s }
                return "{}"
            }()
            let output: [String: AnyEncodable] = [
                "status": AnyEncodable("stubbed"),
                "command": AnyEncodable("codex exec --json"),
                "arguments_json": AnyEncodable(argsJSON)
            ]
            let upd = ACPToolCallUpdateWire(call_id: callId, status: .completed, output: AnyEncodable(output), error: nil)
            await updateHub.sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(upd))
            return
        }

        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, *) {
            do {
                try await fmStream(prompt: prompt, sessionId: sessionId, updateHub: updateHub)
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

    private func ensureSession(for sessionId: ACPSessionId) async throws -> LanguageModelSession {
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
        let s = LanguageModelSession(model: model, tools: [], instructions: instructions)
        s.prewarm(promptPrefix: nil)
        Self.fmSessions[sessionId.value] = s
        return s
    }

    private func fmStream(prompt: String, sessionId: ACPSessionId, updateHub: SessionUpdateHub) async throws {
        let session = try await ensureSession(for: sessionId)
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
}
#endif
#endif
