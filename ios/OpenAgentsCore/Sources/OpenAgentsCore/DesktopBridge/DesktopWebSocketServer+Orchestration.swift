#if os(macOS)
import Foundation

// MARK: - Response Types

private struct ConfigGetResponse: Codable {
    let config: OrchestrationConfig
}

private struct ConfigSetResponse: Codable {
    let success: Bool
    let config_id: String
    let updated_at: Int64
}

private struct ConfigListResponse: Codable {
    let configs: [OrchestrationConfig]
}

private struct ConfigActivateResponse: Codable {
    let success: Bool
    let active_config_id: String
}

private struct SchedulerReloadResponse: Codable {
    let success: Bool
    let message: String
}

private struct SchedulerStatusResponse: Codable {
    let running: Bool
    let active_config_id: String?
    let next_wake_time: Int?
    let message: String
}

extension DesktopWebSocketServer {
    // MARK: - Orchestration Handlers

    func registerOrchestrationHandler() {
        router.register(method: ACPRPC.orchestrateExploreStart) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            // Gate on negotiated extension capability
            if self.advertisedExtCapabilities.orchestrate_explore == false {
                OpenAgentsLog.bridgeServer.warning("orchestrate.explore.* called but not advertised; gating with error")
                JsonRpcRouter.sendError(id: id, code: -32601, message: "orchestrate.explore not supported") { text in
                    client.send(text: text)
                }
                return
            }
            await self.handleOrchestrationStart(id: id, params: params, rawDict: rawDict, client: client)
        }

        // Orchestration Config Handlers
        router.register(method: ACPRPC.orchestrateConfigGet) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleConfigGet(id: id, params: params, rawDict: rawDict, client: client)
        }

        router.register(method: ACPRPC.orchestrateConfigSet) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleConfigSet(id: id, params: params, rawDict: rawDict, client: client)
        }

        router.register(method: ACPRPC.orchestrateConfigList) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleConfigList(id: id, params: params, rawDict: rawDict, client: client)
        }

        router.register(method: ACPRPC.orchestrateConfigActivate) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleConfigActivate(id: id, params: params, rawDict: rawDict, client: client)
        }

        // Scheduler Handlers (lightweight stubs for Phase 3)
        router.register(method: ACPRPC.orchestrateSchedulerReload) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSchedulerReload(id: id, client: client)
        }

        router.register(method: ACPRPC.orchestrateSchedulerStatus) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSchedulerStatus(id: id, client: client)
        }
    }

    func handleOrchestrationStart(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate.explore.start")
        guard let p = rawDict["params"],
              let d = try? JSONSerialization.data(withJSONObject: p),
              let req = try? JSONDecoder().decode(OrchestrateExploreStartRequest.self, from: d) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        let sessionId = ACPSessionId(UUID().uuidString)
        let planId = UUID().uuidString
        let response = OrchestrateExploreStartResponse(
            session_id: sessionId.value,
            plan_id: planId,
            status: "started"
        )
        JsonRpcRouter.sendResponse(id: id, result: response) { responseText in
            OpenAgentsLog.bridgeServer.debug("send rpc result method=orchestrate.explore.start id=\(id.value)")
            client.send(text: responseText)
        }

        if #available(macOS 26.0, *) {
            Task.detached {
                await self.runOrchestration(
                    request: req,
                    sessionId: sessionId,
                    planId: planId,
                    client: client
                )
            }
        } else {
            OpenAgentsLog.bridgeServer.warning("orchestrate.explore.start requires macOS 26.0+")
            JsonRpcRouter.sendError(
                id: id,
                code: -32603,
                message: "orchestrate.explore.start requires macOS 26.0+ for Foundation Models"
            ) { text in
                client.send(text: text)
            }
        }
    }

    @available(macOS 26.0, *)
    func runOrchestration(
        request: OrchestrateExploreStartRequest,
        sessionId: ACPSessionId,
        planId: String,
        client: Client
    ) async {
        OpenAgentsLog.orchestration.info("Starting exploration of \(request.root, privacy: .private)")
        let policy = request.policy ?? ExplorationPolicy(allow_external_llms: false, allow_network: false)
        let streamHandler: ACPUpdateStreamHandler = { [weak self] update in
            await self?.sendSessionUpdate(sessionId: sessionId, update: update)
        }
        let orchestrator = ExploreOrchestrator(
            workspaceRoot: request.root,
            goals: request.goals ?? ["Understand workspace structure"],
            policy: policy,
            streamHandler: streamHandler
        )
        do {
            let summary = try await orchestrator.startExploration()
            OpenAgentsLog.orchestration.info("Completed exploration: \(summary.repo_name)")

            var sections: [String] = []
            sections.append("**Repository:** \(summary.repo_name)")

            if false { /* deterministic sections intentionally disabled */ }

            if #available(macOS 26.0, *) {
                let callId = UUID().uuidString
                let toolCall = ACPToolCallWire(
                    call_id: callId,
                    name: "fm.analysis",
                    arguments: nil,
                    _meta: ["stage": AnyEncodable("summary")]
                )
                await sendSessionUpdate(sessionId: sessionId, update: .toolCall(toolCall))
                let started = ACPToolCallUpdateWire(
                    call_id: callId,
                    status: .started,
                    output: nil,
                    error: nil,
                    _meta: ["progress": AnyEncodable(0.0)]
                )
                await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(started))

                if let analysis = await orchestrator.fmAnalysis(), !analysis.text.isEmpty {
                    let heading = (analysis.source == .sessionAnalyze) ? "\n**Intent From Session History:**" : "\n**Inferred Intent (FM):**"
                    sections.append(heading)
                    sections.append(analysis.text)
                    let outPayload: [String: AnyEncodable] = [
                        "summary_bytes": AnyEncodable(analysis.text.utf8.count),
                        "source": AnyEncodable(analysis.source.rawValue)
                    ]
                    let completed = ACPToolCallUpdateWire(
                        call_id: callId,
                        status: .completed,
                        output: AnyEncodable(outPayload),
                        error: nil,
                        _meta: nil
                    )
                    await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(completed))
                } else {
                    let completed = ACPToolCallUpdateWire(
                        call_id: callId,
                        status: .completed,
                        output: AnyEncodable(["summary_bytes": 0]),
                        error: nil,
                        _meta: nil
                    )
                    await sendSessionUpdate(sessionId: sessionId, update: .toolCallUpdate(completed))
                }
            }

            let summaryText = sections.joined(separator: "\n")
            let summaryChunk = ACP.Client.ContentChunk(content: .text(.init(text: summaryText)))
            await sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(summaryChunk))
        } catch {
            OpenAgentsLog.orchestration.error("Error: \(error)")
            let errorChunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ Orchestration failed: \(error.localizedDescription)")))
            await sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(errorChunk))
        }
    }

    // MARK: - Orchestration Config Handlers

    func handleConfigGet(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/config.get")

        // Parse params
        guard let params = params,
              let workspaceRoot = params["workspace_root"] as? String else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing required parameter: workspace_root") { text in
                client.send(text: text)
            }
            return
        }

        let configId = (params["id"] as? String) ?? "default"

        // Get config from database
        guard let db = self.tinyvexDb else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database not available") { text in
                client.send(text: text)
            }
            return
        }

        do {
            if let configJSON = try await db.getOrchestrationConfig(id: configId, workspaceRoot: workspaceRoot) {
                // Parse to validate it's valid JSON
                guard let data = configJSON.data(using: .utf8),
                      let config = try? JSONDecoder().decode(OrchestrationConfig.self, from: data) else {
                    JsonRpcRouter.sendError(id: id, code: -32603, message: "Config data is corrupted") { text in
                        client.send(text: text)
                    }
                    return
                }

                // Return config
                let response = ConfigGetResponse(config: config)
                JsonRpcRouter.sendResponse(id: id, result: response) { text in
                    OpenAgentsLog.bridgeServer.debug("send orchestrate/config.get result")
                    client.send(text: text)
                }
            } else {
                // Config not found
                JsonRpcRouter.sendError(id: id, code: -32000, message: "Config not found: \(configId)") { text in
                    client.send(text: text)
                }
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database error: \(error.localizedDescription)") { text in
                client.send(text: text)
            }
        }
    }

    func handleConfigSet(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/config.set")

        // Parse params
        guard let params = params,
              let configDict = params["config"] as? [String: Any] else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing required parameter: config") { text in
                client.send(text: text)
            }
            return
        }

        // Decode config
        guard let configData = try? JSONSerialization.data(withJSONObject: configDict),
              var config = try? JSONDecoder().decode(OrchestrationConfig.self, from: configData) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid config format") { text in
                client.send(text: text)
            }
            return
        }

        // Validate config
        let errors = config.validate()
        if !errors.isEmpty {
            let errorMessage = "Config validation failed: \(errors.joined(separator: "; "))"
            JsonRpcRouter.sendError(id: id, code: -32602, message: errorMessage) { text in
                client.send(text: text)
            }
            return
        }

        // Update timestamp
        config.updatedAt = Int64(Date().timeIntervalSince1970 * 1000)

        // Save to database
        guard let db = self.tinyvexDb else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database not available") { text in
                client.send(text: text)
            }
            return
        }

        do {
            let jsonData = try JSONEncoder().encode(config)
            let jsonString = String(data: jsonData, encoding: .utf8)!

            try await db.insertOrUpdateOrchestrationConfig(
                jsonString,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )

            OpenAgentsLog.bridgeServer.info("Saved config: \(config.id) for workspace: \(config.workspaceRoot, privacy: .private)")

            let response = ConfigSetResponse(
                success: true,
                config_id: config.id,
                updated_at: config.updatedAt
            )
            JsonRpcRouter.sendResponse(id: id, result: response) { text in
                OpenAgentsLog.bridgeServer.debug("send orchestrate/config.set result")
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Failed to save config: \(error.localizedDescription)") { text in
                client.send(text: text)
            }
        }
    }

    func handleConfigList(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/config.list")

        guard let db = self.tinyvexDb else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database not available") { text in
                client.send(text: text)
            }
            return
        }

        do {
            let configJSONs: [String]
            if let params = params, let workspaceRoot = params["workspace_root"] as? String {
                // List configs for specific workspace
                configJSONs = try await db.listOrchestrationConfigs(workspaceRoot: workspaceRoot)
            } else {
                // List all configs
                configJSONs = try await db.listAllOrchestrationConfigs()
            }

            // Parse each JSON string to config objects
            var failedDecodes = 0
            let configs = configJSONs.compactMap { jsonString -> OrchestrationConfig? in
                guard let data = jsonString.data(using: .utf8) else {
                    OpenAgentsLog.bridgeServer.warning("Failed to convert config JSON to data")
                    failedDecodes += 1
                    return nil
                }

                do {
                    return try JSONDecoder().decode(OrchestrationConfig.self, from: data)
                } catch {
                    OpenAgentsLog.bridgeServer.warning("Failed to decode OrchestrationConfig: \(error.localizedDescription)")
                    failedDecodes += 1
                    return nil
                }
            }

            if failedDecodes > 0 {
                OpenAgentsLog.bridgeServer.warning("orchestrate/config.list: \(failedDecodes) config(s) failed to decode")
            }

            let response = ConfigListResponse(configs: configs)
            JsonRpcRouter.sendResponse(id: id, result: response) { text in
                OpenAgentsLog.bridgeServer.debug("send orchestrate/config.list result (count=\(configs.count))")
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database error: \(error.localizedDescription)") { text in
                client.send(text: text)
            }
        }
    }

    func handleConfigActivate(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/config.activate")

        // Parse params
        guard let params = params,
              let configId = params["id"] as? String,
              let workspaceRoot = params["workspace_root"] as? String else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing required parameters: id, workspace_root") { text in
                client.send(text: text)
            }
            return
        }

        // Verify config exists
        guard let db = self.tinyvexDb else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database not available") { text in
                client.send(text: text)
            }
            return
        }

        do {
            guard let _ = try await db.getOrchestrationConfig(id: configId, workspaceRoot: workspaceRoot) else {
                JsonRpcRouter.sendError(id: id, code: -32000, message: "Config not found: \(configId)") { text in
                    client.send(text: text)
                }
                return
            }

            // TODO: When SchedulerService is implemented, actually activate the config
            // For now, just return success
            OpenAgentsLog.bridgeServer.info("Activated config: \(configId) (no-op for MVP)")

            let response = ConfigActivateResponse(
                success: true,
                active_config_id: configId
            )
            JsonRpcRouter.sendResponse(id: id, result: response) { text in
                OpenAgentsLog.bridgeServer.debug("send orchestrate/config.activate result")
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Database error: \(error.localizedDescription)") { text in
                client.send(text: text)
            }
        }
    }

    // MARK: - Scheduler Handlers (Lightweight Stubs)

    func handleSchedulerReload(id: JSONRPC.ID, client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/scheduler.reload")

        // TODO: When SchedulerService is implemented, re-read active config and apply
        // For now, return success stub
        OpenAgentsLog.bridgeServer.warning("orchestrate/scheduler.reload is a stub (SchedulerService not yet implemented)")

        let response = SchedulerReloadResponse(
            success: true,
            message: "Scheduler reload requested (stub implementation)"
        )
        JsonRpcRouter.sendResponse(id: id, result: response) { text in
            OpenAgentsLog.bridgeServer.debug("send orchestrate/scheduler.reload result")
            client.send(text: text)
        }
    }

    func handleSchedulerStatus(id: JSONRPC.ID, client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/scheduler.status")

        // TODO: When SchedulerService is implemented, return actual status
        // For now, return stub status
        OpenAgentsLog.bridgeServer.warning("orchestrate/scheduler.status is a stub (SchedulerService not yet implemented)")

        let response = SchedulerStatusResponse(
            running: false,
            active_config_id: nil,
            next_wake_time: nil,
            message: "SchedulerService not yet implemented"
        )
        JsonRpcRouter.sendResponse(id: id, result: response) { text in
            OpenAgentsLog.bridgeServer.debug("send orchestrate/scheduler.status result")
            client.send(text: text)
        }
    }
}
#endif
