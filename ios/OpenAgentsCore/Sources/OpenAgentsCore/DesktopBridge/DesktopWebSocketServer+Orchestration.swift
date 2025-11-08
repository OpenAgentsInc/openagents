#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - Orchestration Handlers

    func registerOrchestrationHandler() {
        router.register(method: ACPRPC.orchestrateExploreStart) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            // Gate on negotiated extension capability
            if self.advertisedExtCapabilities.orchestrate_explore == false {
                OpenAgentsLog.server.warning("orchestrate.explore.* called but not advertised; gating with error")
                JsonRpcRouter.sendError(id: id, code: -32601, message: "orchestrate.explore not supported") { text in
                    client.send(text: text)
                }
                return
            }
            await self.handleOrchestrationStart(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    func handleOrchestrationStart(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        OpenAgentsLog.server.info("recv orchestrate.explore.start")
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
            OpenAgentsLog.server.debug("send rpc result method=orchestrate.explore.start id=\(id.value)")
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
            OpenAgentsLog.server.warning("orchestrate.explore.start requires macOS 26.0+")
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
}
#endif
