#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // Create a new session ID (local path)
    public func localSessionNew() -> ACP.Agent.SessionNewResponse {
        let sid = ACPSessionId(UUID().uuidString)
        return ACP.Agent.SessionNewResponse(session_id: sid)
    }

    // Set the current session mode and broadcast an update (local path)
    public func localSessionSetMode(sessionId: ACPSessionId, mode: ACPSessionModeId) async {
        modeBySession[sessionId.value] = mode
        await sendSessionUpdate(sessionId: sessionId, update: .currentModeUpdate(.init(current_mode_id: mode)))
    }

    // Cancel an active session if running (local path)
    public func localSessionCancel(sessionId: ACPSessionId) async {
        if let handle = await agentRegistry.handle(for: sessionId),
           let provider = await agentRegistry.provider(for: handle.mode) {
            await provider.cancel(sessionId: sessionId, handle: handle)
            await agentRegistry.removeHandle(for: sessionId)
        }
    }

    // Dispatch a prompt locally through the registered provider(s)
    public func localSessionPrompt(request: ACP.Agent.SessionPromptRequest) async throws {
        let sessionId = request.session_id
        let sidStr = sessionId.value
        let promptText = request.content.compactMap { block -> String? in
            if case .text(let t) = block { return t.text }
            return nil
        }.joined(separator: "\n")
        guard !promptText.isEmpty else { return }

        let mode = modeBySession[sidStr] ?? .default_mode
        guard let provider = await agentRegistry.provider(for: mode) else { return }
        guard await provider.isAvailable() else { return }

        guard let updateHub = self.updateHub else { return }
        let context = AgentContext(
            workingDirectory: workingDirectory,
            mcpServers: nil,
            client: nil,
            metadata: [:]
        )

        if let existingHandle = await agentRegistry.handle(for: sessionId) {
            try? await provider.resume(
                sessionId: sessionId,
                prompt: promptText,
                handle: existingHandle,
                context: context,
                updateHub: updateHub
            )
        } else {
            let handle = try? await provider.start(
                sessionId: sessionId,
                prompt: promptText,
                context: context,
                updateHub: updateHub
            )
            if let h = handle { await agentRegistry.setHandle(h, for: sessionId) }
        }
    }

    // Tinyvex history access (local path)
    public func localHistoryRecentSessions() async throws -> [HistoryApi.SessionItem] {
        guard let api = self.historyApi else { return [] }
        return try await api.recentSessions()
    }

    public func localHistorySessionTimeline(sessionId: String, limit: Int?) async throws -> [ACP.Client.SessionNotificationWire] {
        guard let api = self.historyApi else { return [] }
        return try await api.sessionTimeline(sessionId: sessionId, limit: limit)
    }

    // Conversation titles (local path)
    public func localSetSessionTitle(sessionId: String, title: String, updatedAt: Int64) async {
        try? await self.tinyvexDb?.setSessionTitle(sessionId: sessionId, title: title, updatedAt: updatedAt)
    }
    public func localGetSessionTitle(sessionId: String) async -> String? {
        return try? await self.tinyvexDb?.getSessionTitle(sessionId: sessionId)
    }
}
#endif
