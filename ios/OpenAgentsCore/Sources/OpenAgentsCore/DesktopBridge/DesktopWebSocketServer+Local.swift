#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // Create a new session ID (local path)
    public func localSessionNew() -> ACP.Agent.SessionNewResponse {
        let sid = ACPSessionId(UUID().uuidString)

        // Set preferred default mode for this session
        Task { [weak self] in
            guard let self = self else { return }
            let sidStr = sid.value
            self.modeBySession[sidStr] = self.preferredDefaultMode
            let update = ACP.Client.SessionUpdate.currentModeUpdate(.init(current_mode_id: self.preferredDefaultMode))
            await self.sendSessionUpdate(sessionId: sid, update: update)
            OpenAgentsLog.bridgeServer.info("localSessionNew: defaulting mode to \(self.preferredDefaultMode.rawValue) for session=\(sidStr)")
        }

        return ACP.Agent.SessionNewResponse(session_id: sid)
    }

    // Set the current session mode and broadcast an update (local path)
    public func localSessionSetMode(sessionId: ACPSessionId, mode: ACPSessionModeId) async {
        var actualMode = mode

        // If client requests default_mode but a preferred agent is available, use it transparently
        if mode == .default_mode && self.preferredDefaultMode != .default_mode {
            actualMode = self.preferredDefaultMode
            OpenAgentsLog.bridgeServer.info("localSessionSetMode requested default_mode; overriding to \(self.preferredDefaultMode.rawValue) (preferred)")
        }

        modeBySession[sessionId.value] = actualMode
        await sendSessionUpdate(sessionId: sessionId, update: .currentModeUpdate(.init(current_mode_id: actualMode)))
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
        guard let provider = await agentRegistry.provider(for: mode) else {
            // No provider for the chosen mode → inform UI
            if let hub = self.updateHub {
                let msg = "No agent provider for mode: \(mode.rawValue). Choose an agent in the sidebar."
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ \(msg)")))
                await hub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
            }
            return
        }
        guard await provider.isAvailable() else {
            // Provider not available (CLI missing) → inform UI
            if let hub = self.updateHub {
                let msg = "\(provider.displayName) is not available. Please install the \((provider as? CLIAgentProvider)?.binaryName ?? provider.displayName) CLI."
                let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ \(msg)")))
                await hub.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(chunk))
            }
            return
        }

        guard let updateHub = self.updateHub else { return }
        let context = AgentContext(
            workingDirectory: workingDirectory,
            mcpServers: nil,
            client: nil,
            server: self,
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

    public func localClearSessionTitle(sessionId: String) async {
        try? await self.tinyvexDb?.clearSessionTitle(sessionId: sessionId)
    }

    // MARK: - Orchestration Scheduler (local helpers)
    public struct LocalSchedulerStatus: Codable { public let running: Bool; public let active_config_id: String?; public let next_wake_time: Int?; public let message: String }

    public func localSchedulerStatus() -> LocalSchedulerStatus {
        var next: Int? = nil
        var msg = ""
        if let cfg = self.activeOrchestrationConfig,
           let candidate = SchedulePreview.nextRuns(schedule: cfg.schedule, count: 1, from: Date()).first {
            next = Int(candidate.timeIntervalSince1970)
            msg = SchedulePreview.humanReadable(schedule: cfg.schedule)
        } else {
            msg = "No active orchestration config"
        }
        return .init(running: false, active_config_id: self.activeOrchestrationConfig?.id, next_wake_time: next, message: msg)
    }

    public struct LocalRunNowResult: Codable { public let started: Bool; public let session_id: String? }

    public func localSchedulerRunNow() async -> LocalRunNowResult {
        guard let cfg = self.activeOrchestrationConfig else { return .init(started: false, session_id: nil) }
        guard let updateHub = self.updateHub else { return .init(started: false, session_id: nil) }
        let sessionId = ACPSessionId(UUID().uuidString)
        if #available(macOS 26.0, *) {
            Task.detached { [weak self] in
                guard let self = self else { return }
                let policy = ExplorationPolicy(allow_external_llms: false, allow_network: false, use_native_tool_calling: false)
                let streamHandler: ACPUpdateStreamHandler = { [weak self] update in
                    await self?.sendSessionUpdate(sessionId: sessionId, update: update)
                }
                // Immediate heartbeat so tests and ops see a quick update
                let startChunk = ACP.Client.ContentChunk(content: .text(.init(text: "⏱️ Orchestration run starting…")))
                await self.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(startChunk))
                let orchestrator = ExploreOrchestrator(
                    workspaceRoot: cfg.workspaceRoot,
                    goals: cfg.goals.isEmpty ? ["Automated maintenance run"] : cfg.goals,
                    policy: policy,
                    streamHandler: streamHandler
                )
                do {
                    let summary = try await orchestrator.startExploration()
                    // Emit a simple summary message like runOrchestration
                    var sections: [String] = []
                    sections.append("**Repository:** \(summary.repo_name)")
                    let summaryText = sections.joined(separator: "\n")
                    let summaryChunk = ACP.Client.ContentChunk(content: .text(.init(text: summaryText)))
                    await self.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(summaryChunk))
                } catch {
                    let errorChunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ Orchestration failed: \(error.localizedDescription)")))
                    await self.sendSessionUpdate(sessionId: sessionId, update: .agentMessageChunk(errorChunk))
                }
            }
            return .init(started: true, session_id: sessionId.value)
        } else {
            return .init(started: false, session_id: nil)
        }
    }

    // MARK: - Orchestration Config (local helpers)
    @discardableResult
    public func localConfigSet(_ config: OrchestrationConfig) async -> Bool {
        guard let db = self.tinyvexDb, let json = try? String(data: JSONEncoder().encode(config), encoding: .utf8) else { return false }
        do {
            try await db.insertOrUpdateOrchestrationConfig(json ?? "{}", id: config.id, workspaceRoot: config.workspaceRoot, updatedAt: config.updatedAt)
            return true
        } catch { return false }
    }

    @discardableResult
    public func localConfigActivate(id: String, workspaceRoot: String) async -> Bool {
        guard let db = self.tinyvexDb else { return false }
        do {
            guard let json = try await db.getOrchestrationConfig(id: id, workspaceRoot: workspaceRoot),
                  let data = json.data(using: .utf8),
                  let cfg = try? JSONDecoder().decode(OrchestrationConfig.self, from: data) else { return false }
            self.activeOrchestrationConfig = cfg
            return true
        } catch { return false }
    }

    // MARK: - Coordinator (local helpers)
    public struct LocalCoordinatorRunOnceResult: Codable { public let status: String; public let task_id: String?; public let session_id: String?; public let agent_mode: String? }
    public struct LocalCoordinatorStatus: Codable { public let cycles_run: Int; public let tasks_executed: Int; public let tasks_completed: Int; public let tasks_failed: Int; public let tasks_cancelled: Int; public let last_cycle_ts: Int64? }

    public func localCoordinatorRunOnce(config: OrchestrationConfig? = nil) async -> LocalCoordinatorRunOnceResult {
        guard let coord = await ensureCoordinator() else {
            return .init(status: "error", task_id: nil, session_id: nil, agent_mode: nil)
        }
        let cfg = config ?? self.activeOrchestrationConfig
        let result = await coord.runCycle(config: cfg ?? OrchestrationConfig.createDefault(workspaceRoot: workingDirectory?.path ?? NSHomeDirectory()), workingDirectory: self.workingDirectory)
        switch result {
        case .taskExecuted(let taskId, let sessionId):
            var modeStr: String? = nil
            if let db = self.tinyvexDb, let tq = try? await TaskQueue(db: db) {
                do {
                    let taskOpt = try await tq.get(taskId)
                    if let task = taskOpt { modeStr = task.decision.agentMode.rawValue }
                } catch { /* ignore */ }
            }
            return .init(status: "executing", task_id: taskId, session_id: sessionId, agent_mode: modeStr)
        case .decisionMade(let taskId):
            var modeStr: String? = nil
            if let db = self.tinyvexDb, let tq = try? await TaskQueue(db: db) {
                do {
                    let taskOpt = try await tq.get(taskId)
                    if let task = taskOpt { modeStr = task.decision.agentMode.rawValue }
                } catch { /* ignore */ }
            }
            return .init(status: "enqueued", task_id: taskId, session_id: nil, agent_mode: modeStr)
        case .noAgentsAvailable:
            return .init(status: "no_agents", task_id: nil, session_id: nil, agent_mode: nil)
        case .taskFailed(let taskId, _):
            return .init(status: "failed", task_id: taskId, session_id: nil, agent_mode: nil)
        case .idle:
            return .init(status: "idle", task_id: nil, session_id: nil, agent_mode: nil)
        }
    }

    public func localCoordinatorStatus() async -> LocalCoordinatorStatus {
        guard let coord = await ensureCoordinator() else {
            return .init(cycles_run: 0, tasks_executed: 0, tasks_completed: 0, tasks_failed: 0, tasks_cancelled: 0, last_cycle_ts: nil)
        }
        let m = await coord.metrics()
        let ts = m.lastCycleTimestamp.map { Int64($0.timeIntervalSince1970 * 1000) }
        return .init(cycles_run: m.cyclesRun, tasks_executed: m.tasksExecuted, tasks_completed: m.tasksCompleted, tasks_failed: m.tasksFailed, tasks_cancelled: m.tasksCancelled, last_cycle_ts: ts)
    }

    // MARK: - Scheduler Reload (local helper)
    public struct LocalSchedulerReloadResult: Codable { public let success: Bool; public let message: String }
    public func localSchedulerReload() async -> LocalSchedulerReloadResult {
        guard let cfg = self.activeOrchestrationConfig else {
            return .init(success: false, message: "No active orchestration config")
        }
        if let svc = self.schedulerService { await svc.stop() }
        let svc = SchedulerService()
        await svc.configure(config: cfg) { [weak self] in
            guard let self = self else { return }
            guard let coord = await self.ensureCoordinator() else { return }
            _ = await coord.runCycle(config: cfg, workingDirectory: self.workingDirectory)
        }
        await svc.start()
        self.schedulerService = svc
        return .init(success: true, message: "Scheduler started")
    }

    // MARK: - Orchestration Setup (local helpers)
    public struct LocalSetupStartResponse: Codable { public let status: String; public let session_id: String; public let conversation_id: String }

    /// Start the conversational orchestration setup locally (no JSON-RPC envelope)
    /// - Parameters:
    ///   - workspaceRoot: Optional workspace path to seed the conversation
    ///   - sessionId: Optional session to stream messages into (defaults to new)
    /// - Returns: Start response including chosen session and conversation id
    public func localSetupStart(
        workspaceRoot: String?,
        goals: [String]? = nil,
        windowStart: String? = nil,
        windowEnd: String? = nil,
        preferAgent: String? = nil,
        allowAgents: [String]? = nil,
        sessionId: ACPSessionId? = nil
    ) async -> LocalSetupStartResponse {
        let sid = sessionId ?? ACPSessionId(UUID().uuidString)
        guard let updateHub = self.updateHub else {
            return .init(status: "error", session_id: sid.value, conversation_id: "")
        }

        let orchestrator = SetupOrchestrator(
            conversationId: UUID().uuidString,
            sessionId: sid,
            initialWorkspace: workspaceRoot,
            updateHub: updateHub,
            completionHandler: { [weak self] result in
                guard let self = self else { return }
                switch result {
                case .success(let config):
                    OpenAgentsLog.bridgeServer.info("Setup completed: \(config.id)")
                    if let db = self.tinyvexDb,
                       let jsonData = try? JSONEncoder().encode(config),
                       let jsonString = String(data: jsonData, encoding: .utf8) {
                        do {
                            try await db.insertOrUpdateOrchestrationConfig(
                                jsonString,
                                id: config.id,
                                workspaceRoot: config.workspaceRoot,
                                updatedAt: config.updatedAt
                            )
                            OpenAgentsLog.bridgeServer.info("Saved completed config: \(config.id)")
                            // Activate and (re)start scheduler on new config (local path)
                            self.activeOrchestrationConfig = config
                            if let svc = self.schedulerService { await svc.stop() }
                            let svc = SchedulerService()
                            await svc.configure(config: config) { [weak self] in
                                guard let self = self else { return }
                                guard let coord = await self.ensureCoordinator() else { return }
                                _ = await coord.runCycle(config: config, workingDirectory: self.workingDirectory)
                            }
                            await svc.start()
                            self.schedulerService = svc
                            OpenAgentsLog.bridgeServer.info("Scheduler restarted with new config (local): \(config.id)")
                        } catch {
                            OpenAgentsLog.bridgeServer.error("Failed to save config: \(error.localizedDescription)")
                        }
                    } else {
                        OpenAgentsLog.bridgeServer.error("Cannot save config: database not available or encode failed")
                    }
                case .failure(let err):
                    OpenAgentsLog.bridgeServer.error("Setup failed: \(err.localizedDescription)")
                }
                // Cleanup mapping
                if let convId = self.setupSessionById[sid.value] {
                    self.setupSessionById.removeValue(forKey: sid.value)
                    await SetupOrchestratorRegistry.shared.remove(convId)
                }
            }
        )

        let convId = await orchestrator.conversationId
        await SetupOrchestratorRegistry.shared.store(orchestrator, for: convId)
        self.setupSessionById[sid.value] = convId
        // Apply typed hints provided by FM tool
        let preferId = preferAgent.flatMap { ACPSessionModeId(rawValue: $0) }
        let allowIds = allowAgents?.compactMap { ACPSessionModeId(rawValue: $0) }
        await orchestrator.applyHints(
            goals: goals,
            windowStart: windowStart,
            windowEnd: windowEnd,
            prefer: preferId,
            allow: allowIds
        )
        await orchestrator.start()
        return .init(status: "started", session_id: sid.value, conversation_id: convId)
    }
}
#endif
