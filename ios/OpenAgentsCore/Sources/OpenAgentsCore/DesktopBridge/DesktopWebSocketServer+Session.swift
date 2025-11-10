#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - Session Handlers

    func registerSessionNewHandler() {
        router.register(method: ACPRPC.sessionNew) { [weak self] id, _, _ in
            guard let self = self, let client = self.currentClient else { return }
            let sid = ACPSessionId(UUID().uuidString)
            let result = ACP.Agent.SessionNewResponse(session_id: sid)

            JsonRpcRouter.sendResponse(id: id, result: result) { text in
                client.send(text: text)
            }

            // Set a preferred default mode immediately for this session and broadcast to client
            Task { [weak self] in
                guard let self = self else { return }
                let sidStr = sid.value
                self.modeBySession[sidStr] = self.preferredDefaultMode
                let update = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: self.preferredDefaultMode))
                await self.sendSessionUpdate(sessionId: sid, update: update)
                OpenAgentsLog.bridgeServer.info("session/new: defaulting mode to \(self.preferredDefaultMode.rawValue) for session=\(sidStr)")
            }
        }
    }

    func registerSessionHandlers() {
        // session/prompt
        router.register(method: ACPRPC.sessionPrompt) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSessionPrompt(id: id, params: params, rawDict: rawDict, client: client)
        }

        // session/set_mode
        router.register(method: ACPRPC.sessionSetMode) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSessionSetMode(id: id, params: params, rawDict: rawDict, client: client)
        }

        // session/cancel
        router.register(method: ACPRPC.sessionCancel) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSessionCancel(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    func handleSessionPrompt(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Parse request
        guard let data = try? JSONSerialization.data(withJSONObject: rawDict),
              let req = try? JSONDecoder().decode(JSONRPC.Request<ACP.Agent.SessionPromptRequest>.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid session/prompt parameters") { text in
                client.send(text: text)
            }
            return
        }

        let sessionId = req.params.session_id
        let sidStr = sessionId.value

        // Extract prompt text from content blocks
        let promptText = req.params.content.compactMap { block -> String? in
            if case .text(let textBlock) = block {
                return textBlock.text
            }
            return nil
        }.joined(separator: "\n")

        guard !promptText.isEmpty else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Empty prompt") { text in
                client.send(text: text)
            }
            return
        }

        OpenAgentsLog.bridgeServer.debug("handleSessionPrompt session=\(sidStr, privacy: .public) prompt=\(promptText.prefix(50), privacy: .private)...")

        // Check if this is a setup session (conversational orchestration setup)
        if let conversationId = setupSessionById[sidStr] {
            if let orchestrator = await SetupOrchestratorRegistry.shared.get(conversationId) {
                OpenAgentsLog.bridgeServer.info("Routing to SetupOrchestrator conversation=\(conversationId)")
                await orchestrator.handleUserResponse(promptText)
                JsonRpcRouter.sendResponse(id: id, result: ["status": "accepted"]) { text in
                    client.send(text: text)
                }
                return
            } else {
                // Cleanup stale mapping
                OpenAgentsLog.bridgeServer.warning("Stale setup session mapping for session=\(sidStr), removing")
                setupSessionById.removeValue(forKey: sidStr)
            }
        }

        // Get mode (defaults to server's preferred default if not set)
        let mode = modeBySession[sidStr] ?? self.preferredDefaultMode
        OpenAgentsLog.bridgeServer.info("handleSessionPrompt session=\(sidStr) using mode=\(mode.rawValue)")

        // Get provider from registry
        guard let provider = await agentRegistry.provider(for: mode) else {
            JsonRpcRouter.sendError(id: id, code: -32601, message: "No agent provider for mode: \(mode.rawValue)") { text in
                client.send(text: text)
            }
            return
        }

        // Check if provider is available
        guard await provider.isAvailable() else {
            JsonRpcRouter.sendError(id: id, code: -32601, message: "\(provider.displayName) is not available. Please install the required CLI.") { text in
                client.send(text: text)
            }
            return
        }

        // Build context
        let context = AgentContext(
            workingDirectory: workingDirectory,
            mcpServers: nil,
            client: client,
            server: self,
            metadata: [:]
        )

        // Get update hub
        guard let updateHub = self.updateHub else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Update hub not initialized") { text in
                client.send(text: text)
            }
            return
        }

        // Check if we have an existing handle (resume scenario)
        if let existingHandle = await agentRegistry.handle(for: sessionId) {
            do {
                try await provider.resume(
                    sessionId: sessionId,
                    prompt: promptText,
                    handle: existingHandle,
                    context: context,
                    updateHub: updateHub
                )
                JsonRpcRouter.sendResponse(id: id, result: ["status": "resumed"]) { text in
                    client.send(text: text)
                }
            } catch {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to resume: \(error.localizedDescription)") { text in
                    client.send(text: text)
                }
            }
        } else {
            do {
                let handle = try await provider.start(
                    sessionId: sessionId,
                    prompt: promptText,
                    context: context,
                    updateHub: updateHub
                )
                await agentRegistry.setHandle(handle, for: sessionId)
                JsonRpcRouter.sendResponse(id: id, result: ["status": "started"]) { text in
                    client.send(text: text)
                }
            } catch {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to start: \(error.localizedDescription)") { text in
                    client.send(text: text)
                }
            }
        }
    }

    func handleSessionSetMode(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
        let sid = ACPSessionId(sidStr)
        let modeStr = (params?["mode_id"] as? String) ?? ACPSessionModeId.default_mode.rawValue
        var modeId = ACPSessionModeId(rawValue: modeStr) ?? .default_mode
        // If client requests default_mode but a preferred agent is available, use it transparently
        if modeId == .default_mode && self.preferredDefaultMode != .default_mode {
            modeId = self.preferredDefaultMode
            OpenAgentsLog.bridgeServer.info("session/set_mode requested default_mode; overriding to \(self.preferredDefaultMode.rawValue) (preferred)")
        }
        self.modeBySession[sidStr] = modeId

        let update = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: modeId))
        await sendSessionUpdate(sessionId: sid, update: update)

        let result = ACP.Agent.SetSessionModeResponse()
        JsonRpcRouter.sendResponse(id: id, result: result) { responseText in
            OpenAgentsLog.bridgeServer.debug("send rpc result method=\(ACPRPC.sessionSetMode) id=\(id.value) text=\(responseText, privacy: .public)")
            client.send(text: responseText)
        }
    }

    func handleSessionCancel(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        guard let sessionIdStr = params?["session_id"] as? String else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing session_id") { text in
                client.send(text: text)
            }
            return
        }

        let sessionId = ACPSessionId(sessionIdStr)
        guard let handle = await agentRegistry.handle(for: sessionId) else {
            JsonRpcRouter.sendResponse(id: id, result: ["status": "no_active_session"]) { text in
                client.send(text: text)
            }
            return
        }
        guard let provider = await agentRegistry.provider(for: handle.mode) else {
            JsonRpcRouter.sendError(id: id, code: -32601, message: "Provider not found for mode: \(handle.mode.rawValue)") { text in
                client.send(text: text)
            }
            return
        }

        await provider.cancel(sessionId: sessionId, handle: handle)
        await agentRegistry.removeHandle(for: sessionId)
        JsonRpcRouter.sendResponse(id: id, result: ["status": "cancelled"]) { text in
            client.send(text: text)
        }
    }
}
#endif
