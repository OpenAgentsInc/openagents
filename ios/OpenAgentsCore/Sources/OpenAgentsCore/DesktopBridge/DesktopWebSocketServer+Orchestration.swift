#if os(macOS)
import Foundation

// MARK: - Setup Orchestrator Storage

/// Global actor for storing active setup orchestrators
/// (Extensions cannot have stored properties, so we use a global actor)
actor SetupOrchestratorRegistry {
    static let shared = SetupOrchestratorRegistry()

    private var orchestrators: [String: SetupOrchestrator] = [:]

    func store(_ orchestrator: SetupOrchestrator, for conversationId: String) {
        orchestrators[conversationId] = orchestrator
    }

    func get(_ conversationId: String) -> SetupOrchestrator? {
        return orchestrators[conversationId]
    }

    func remove(_ conversationId: String) {
        orchestrators.removeValue(forKey: conversationId)
    }
}

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

    private struct SchedulerReloadResponse: Codable { let success: Bool; let message: String }

    private struct SchedulerStatusResponse: Codable { let running: Bool; let active_config_id: String?; let next_wake_time: Int?; let message: String }
    
    private struct SchedulerRunNowResponse: Codable {
        let started: Bool
        let message: String
        let session_id: String?
    }

private struct SetupStartRequest: Codable {
    let workspace_root: String?
    let session_id: String?
    let initial_prompt: String?
}

private struct SetupStartResponse: Codable {
    let status: String
    let session_id: String
    let conversation_id: String
}

private struct SetupStatusRequest: Codable {
    let conversation_id: String
}

private struct SetupStatusResponse: Codable {
    let conversation_id: String
    let state: String
    let draft: SetupDraft?
}

private struct SetupAbortRequest: Codable {
    let conversation_id: String
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

        // Scheduler Handlers
        router.register(method: ACPRPC.orchestrateSchedulerReload) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSchedulerReload(id: id, client: client)
        }

        router.register(method: ACPRPC.orchestrateSchedulerStatus) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSchedulerStatus(id: id, client: client)
        }

        // Immediate trigger (ops/testing): run scheduled orchestration now
        router.register(method: ACPRPC.orchestrateSchedulerRunNow) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSchedulerRunNow(id: id, client: client)
        }

        // Test alias: advance time (implemented as run_now)
        router.register(method: ACPRPC.orchestrateSchedulerAdvance) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSchedulerRunNow(id: id, client: client)
        }

        // Setup Handlers (conversational config creation)
        router.register(method: ACPRPC.orchestrateSetupStart) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSetupStart(id: id, params: params, rawDict: rawDict, client: client)
        }

        router.register(method: ACPRPC.orchestrateSetupStatus) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSetupStatus(id: id, params: params, rawDict: rawDict, client: client)
        }

        router.register(method: ACPRPC.orchestrateSetupAbort) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleSetupAbort(id: id, params: params, rawDict: rawDict, client: client)
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

            // Cache active config in memory for scheduler RPCs
            if let json = try await db.getOrchestrationConfig(id: configId, workspaceRoot: workspaceRoot),
               let data = json.data(using: .utf8),
               let cfg = try? JSONDecoder().decode(OrchestrationConfig.self, from: data) {
                self.activeOrchestrationConfig = cfg
            }

            OpenAgentsLog.bridgeServer.info("Activated config: \(configId)")

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
        guard let cfg = self.activeOrchestrationConfig else {
            JsonRpcRouter.sendError(id: id, code: -32000, message: "No active orchestration config") { text in client.send(text: text) }
            return
        }
        // Stop any running service and start a fresh one
        if let svc = self.schedulerService { await svc.stop() }
        let svc = SchedulerService()
        await svc.configure(config: cfg) { [weak self] in
            guard let self = self else { return }
            _ = await self.localSchedulerRunNow() // triggers a run immediately at wake
        }
        await svc.start()
        self.schedulerService = svc
        let response = SchedulerReloadResponse(success: true, message: "Scheduler started")
        JsonRpcRouter.sendResponse(id: id, result: response) { text in client.send(text: text) }
    }

    func handleSchedulerStatus(id: JSONRPC.ID, client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/scheduler.status")

        // Compute status via scheduler or fallback to preview
        var next: Int? = nil
        var running = false
        var msg = ""
        if let svc = self.schedulerService {
            running = true
            if case let .running(nextWake) = await svc.status(), let nw = nextWake { next = Int(nw.timeIntervalSince1970) }
        }
        if next == nil, let cfg = self.activeOrchestrationConfig, let candidate = SchedulePreview.nextRuns(schedule: cfg.schedule, count: 1, from: Date()).first {
            next = Int(candidate.timeIntervalSince1970)
        }
        msg = self.activeOrchestrationConfig.map { SchedulePreview.humanReadable(schedule: $0.schedule) } ?? "No active orchestration config"

        let response = SchedulerStatusResponse(running: running, active_config_id: self.activeOrchestrationConfig?.id, next_wake_time: next, message: msg)
        JsonRpcRouter.sendResponse(id: id, result: response) { text in
            OpenAgentsLog.bridgeServer.debug("send orchestrate/scheduler.status result")
            client.send(text: text)
        }
    }

    // MARK: - Run Now (immediate trigger)
    func handleSchedulerRunNow(id: JSONRPC.ID, client: Client) async {
        guard let cfg = self.activeOrchestrationConfig else {
            JsonRpcRouter.sendError(id: id, code: -32000, message: "No active orchestration config") { text in
                client.send(text: text)
            }
            return
        }

        let sessionId = ACPSessionId(UUID().uuidString)
        let planId = UUID().uuidString

        // Respond immediately
        let resp = SchedulerRunNowResponse(started: true, message: "Triggered run_now", session_id: sessionId.value)
        JsonRpcRouter.sendResponse(id: id, result: resp) { text in client.send(text: text) }

        // Kick off orchestration asynchronously
        if #available(macOS 26.0, *) {
            Task.detached { [weak self] in
                guard let self = self else { return }
                let req = OrchestrateExploreStartRequest(
                    root: cfg.workspaceRoot,
                    remote_url: nil,
                    branch: nil,
                    policy: cfg.agentPreferences.prefer == .codex
                        ? ExplorationPolicy(allow_external_llms: false, allow_network: false, use_native_tool_calling: false)
                        : ExplorationPolicy(allow_external_llms: false, allow_network: false, use_native_tool_calling: false),
                    goals: cfg.goals.isEmpty ? ["Automated maintenance run"] : cfg.goals
                )
                await self.runOrchestration(request: req, sessionId: sessionId, planId: planId, client: client)
            }
        } else {
            OpenAgentsLog.bridgeServer.warning("run_now requires macOS 26.0+")
        }
    }

    // MARK: - Setup Handlers (Conversational Config Creation)
    /// Active setup orchestrators (conversation_id -> orchestrator)
    func handleSetupStart(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/setup.start")

        // Parse params
        guard let p = rawDict["params"],
              let data = try? JSONSerialization.data(withJSONObject: p),
              let req = try? JSONDecoder().decode(SetupStartRequest.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        // Create session ID
        let sessionId = req.session_id.map { ACPSessionId($0) } ?? ACPSessionId(UUID().uuidString)

        // Ensure updateHub is available
        guard let updateHub = self.updateHub else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "SessionUpdateHub not available") { text in
                client.send(text: text)
            }
            return
        }

        // Create SetupOrchestrator
        let orchestrator = SetupOrchestrator(
            conversationId: UUID().uuidString,
            sessionId: sessionId,
            initialWorkspace: req.workspace_root,
            updateHub: updateHub,
            completionHandler: { [weak self] result in
                guard let self = self else { return }
                switch result {
                case .success(let config):
                    OpenAgentsLog.bridgeServer.info("Setup completed: \(config.id)")
                    // Save config via existing config.set handler
                    await self.saveCompletedConfig(config)
                    // Cleanup registries
                    if let conversationId = self.setupSessionById[sessionId.value] {
                        self.setupSessionById.removeValue(forKey: sessionId.value)
                        await SetupOrchestratorRegistry.shared.remove(conversationId)
                    }
                case .failure(let error):
                    OpenAgentsLog.bridgeServer.error("Setup failed: \(error.localizedDescription)")
                    // Cleanup registries
                    if let conversationId = self.setupSessionById[sessionId.value] {
                        self.setupSessionById.removeValue(forKey: sessionId.value)
                        await SetupOrchestratorRegistry.shared.remove(conversationId)
                    }
                }
            }
        )

        // Store orchestrator
        let conversationId = await orchestrator.conversationId
        await SetupOrchestratorRegistry.shared.store(orchestrator, for: conversationId)

        // Store session→conversation mapping for routing session/prompt
        self.setupSessionById[sessionId.value] = conversationId

        // Start conversational setup
        await orchestrator.start()

        // Return response
        let response = SetupStartResponse(
            status: "started",
            session_id: sessionId.value,
            conversation_id: conversationId
        )
        JsonRpcRouter.sendResponse(id: id, result: response) { text in
            OpenAgentsLog.bridgeServer.debug("send orchestrate/setup.start result")
            client.send(text: text)
        }
    }

    func handleSetupStatus(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/setup.status")

        // Parse params
        guard let p = rawDict["params"],
              let data = try? JSONSerialization.data(withJSONObject: p),
              let req = try? JSONDecoder().decode(SetupStatusRequest.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        // Find orchestrator
        guard let orchestrator = await SetupOrchestratorRegistry.shared.get(req.conversation_id) else {
            JsonRpcRouter.sendError(id: id, code: -32000, message: "Conversation not found: \(req.conversation_id)") { text in
                client.send(text: text)
            }
            return
        }

        // Get current state
        let state = await orchestrator.getCurrentState()
        let draft = await orchestrator.getCurrentDraft()

        let response = SetupStatusResponse(
            conversation_id: req.conversation_id,
            state: state.rawValue,
            draft: draft
        )
        JsonRpcRouter.sendResponse(id: id, result: response) { text in
            OpenAgentsLog.bridgeServer.debug("send orchestrate/setup.status result")
            client.send(text: text)
        }
    }

    func handleSetupAbort(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.bridgeServer.info("recv orchestrate/setup.abort")

        // Parse params
        guard let p = rawDict["params"],
              let data = try? JSONSerialization.data(withJSONObject: p),
              let req = try? JSONDecoder().decode(SetupAbortRequest.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        // Find and abort orchestrator
        if let orchestrator = await SetupOrchestratorRegistry.shared.get(req.conversation_id) {
            await orchestrator.abort()

            // Cleanup registries
            await SetupOrchestratorRegistry.shared.remove(req.conversation_id)
            // Find and remove session mapping
            if let sessionId = setupSessionById.first(where: { $0.value == req.conversation_id })?.key {
                setupSessionById.removeValue(forKey: sessionId)
            }

            let response = ["status": "aborted"]
            JsonRpcRouter.sendResponse(id: id, result: response) { text in
                OpenAgentsLog.bridgeServer.debug("send orchestrate/setup.abort result")
                client.send(text: text)
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32000, message: "Conversation not found: \(req.conversation_id)") { text in
                client.send(text: text)
            }
        }
    }

    /// Save completed config to database
    private func saveCompletedConfig(_ config: OrchestrationConfig) async {
        guard let db = self.tinyvexDb else {
            OpenAgentsLog.bridgeServer.error("Cannot save config: database not available")
            return
        }

        do {
            let jsonData = try JSONEncoder().encode(config)
            guard let jsonString = String(data: jsonData, encoding: .utf8) else {
                OpenAgentsLog.bridgeServer.error("Failed to encode config as JSON string")
                return
            }

            try await db.insertOrUpdateOrchestrationConfig(
                jsonString,
                id: config.id,
                workspaceRoot: config.workspaceRoot,
                updatedAt: config.updatedAt
            )
            OpenAgentsLog.bridgeServer.info("Saved completed config: \(config.id)")

            // TODO: Trigger scheduler reload when SchedulerService is implemented
            // This will re-read configs and apply the new schedule
            OpenAgentsLog.bridgeServer.info("Config saved, scheduler reload would be triggered here")
        } catch {
            OpenAgentsLog.bridgeServer.error("Failed to save config: \(error.localizedDescription)")
        }
    }
}
#endif
