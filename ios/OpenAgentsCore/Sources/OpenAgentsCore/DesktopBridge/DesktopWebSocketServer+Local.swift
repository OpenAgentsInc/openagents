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
            if let db = self.tinyvexDb, let tq = try? await TaskQueue(db: db), let task = try? await tq.get(taskId) {
                modeStr = task?.decision.agentMode.rawValue
            }
            return .init(status: "executing", task_id: taskId, session_id: sessionId, agent_mode: modeStr)
        case .decisionMade(let taskId):
            var modeStr: String? = nil
            if let db = self.tinyvexDb, let tq = try? await TaskQueue(db: db), let task = try? await tq.get(taskId) {
                modeStr = task?.decision.agentMode.rawValue
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
}
#endif
