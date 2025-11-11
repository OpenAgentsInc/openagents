import Foundation
import OpenAgentsCore

final class PromptDispatcher: PromptDispatching {
    private weak var rpc: JSONRPCSending?
    private let timeline: TimelineStoring
    private let logger: ((String, String) -> Void)?
    private struct EmptyResult: Codable {}

    // Track optimistic echo index to adjust session id when server creates a new session
    private var pendingUserEchoIndex: Int?

    init(rpc: JSONRPCSending?, timeline: TimelineStoring, logger: ((String, String) -> Void)? = nil) {
        self.rpc = rpc
        self.timeline = timeline
        self.logger = logger
    }

    func sendPrompt(text: String, desiredMode: ACPSessionModeId?, getSessionId: () -> ACPSessionId?, setSessionId: @escaping (ACPSessionId) -> Void) {
        guard let rpc = self.rpc else {
            logger?("send", "aborted: rpc unavailable")
            return
        }

        let parts: [ACP.Client.ContentBlock] = [.text(.init(text: text))]
        // Optimistic UI echo
        let sid = getSessionId() ?? ACPSessionId("pending")
        pendingUserEchoIndex = timeline.appendOptimisticUserMessage(text: text, sessionId: sid)
        logger?("send", "optimistic echo appended session=\(sid.value)")

        if let existing = getSessionId() {
            if let mode = desiredMode {
                logger?("send", "existing session: set_mode=\(mode.rawValue)")
                self.setSessionMode(mode, getSessionId: { existing })
            } else {
                logger?("send", "existing session: keep current mode")
            }
            let req = ACP.Agent.SessionPromptRequest(session_id: existing, content: parts)
            let rid = "session-prompt-\(UUID().uuidString)"
            logger?("send", "rpc \(ACPRPC.sessionPrompt) id=\(rid) len=\(text.count)")
            rpc.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: rid) { (_: EmptyResult?) in
                self.logger?("send", "ack \(ACPRPC.sessionPrompt) id=\(rid)")
            }
            return
        }

        // No session yet: create one then prompt
        let newId = "session-new-\(UUID().uuidString)"
        logger?("send", "rpc \(ACPRPC.sessionNew) id=\(newId)")
        rpc.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: newId) { (resp: ACP.Agent.SessionNewResponse?) in
            guard let resp = resp else { return }
            DispatchQueue.main.async {
                setSessionId(resp.session_id)
                // convert optimistic echo to real session id
                if let idx = self.pendingUserEchoIndex {
                    self.rewriteSessionId(at: idx, to: resp.session_id)
                    self.pendingUserEchoIndex = nil
                }
                self.logger?("send", "session created id=\(resp.session_id.value)")
            }
            // Optionally set mode then prompt
            if let mode = desiredMode {
                struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
                let mid = "session-set-mode-\(UUID().uuidString)"
                self.logger?("send", "rpc \(ACPRPC.sessionSetMode) id=\(mid) mode=\(mode.rawValue)")
                rpc.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: resp.session_id, mode_id: mode), id: mid) { (_: ACP.Agent.SetSessionModeResponse?) in
                    self.logger?("send", "ack \(ACPRPC.sessionSetMode) id=\(mid)")
                    let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                    let pid = "session-prompt-\(UUID().uuidString)"
                    self.logger?("send", "rpc \(ACPRPC.sessionPrompt) id=\(pid) len=\(text.count)")
                    rpc.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: pid) { (_: EmptyResult?) in
                        self.logger?("send", "ack \(ACPRPC.sessionPrompt) id=\(pid)")
                    }
                }
            } else {
                let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                let pid = "session-prompt-\(UUID().uuidString)"
                self.logger?("send", "rpc \(ACPRPC.sessionPrompt) id=\(pid) len=\(text.count)")
                rpc.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: pid) { (_: EmptyResult?) in
                    self.logger?("send", "ack \(ACPRPC.sessionPrompt) id=\(pid)")
                }
            }
        }
    }

    func setSessionMode(_ mode: ACPSessionModeId, getSessionId: () -> ACPSessionId?) {
        guard let sid = getSessionId(), let rpc = self.rpc else { return }
        struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
        rpc.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: sid, mode_id: mode), id: "session-set-mode-\(UUID().uuidString)") { (_: EmptyResult?) in }
    }

    func cancelCurrentSession(getSessionId: () -> ACPSessionId?) {
        guard let sid = getSessionId(), let rpc = self.rpc else { return }
        struct CancelReq: Codable { let session_id: ACPSessionId }
        rpc.sendJSONRPCNotification(method: ACPRPC.sessionCancel, params: CancelReq(session_id: sid))
    }

    func orchestrateExploreStart(root: String, goals: [String]?, onSessionId: @escaping (ACPSessionId) -> Void, completion: ((OrchestrateExploreStartResponse?) -> Void)?) {
        guard let rpc = self.rpc else { completion?(nil); return }
        let policy = ExplorationPolicy(allow_external_llms: false, allow_network: false)
        let request = OrchestrateExploreStartRequest(root: root, remote_url: nil, branch: nil, policy: policy, goals: goals)
        rpc.sendJSONRPC(method: ACPRPC.orchestrateExploreStart, params: request, id: "orchestrate-explore-\(UUID().uuidString)") { (resp: OrchestrateExploreStartResponse?) in
            DispatchQueue.main.async {
                if let resp = resp { onSessionId(ACPSessionId(resp.session_id)) }
                completion?(resp)
            }
        }
    }

    struct CoordinatorRunOnceParams: Codable { let config_id: String?; let config_inline: OrchestrationConfig? }
    struct CoordinatorRunOnceResponse: Codable { let status: String; let task_id: String?; let session_id: String?; let agent_mode: String? }
    func orchestrateCoordinatorRunOnce(configId: String?, configInline: OrchestrationConfig?, completion: ((CoordinatorRunOnceResponse?) -> Void)?) {
        guard let rpc = self.rpc else { completion?(nil); return }
        let params = CoordinatorRunOnceParams(config_id: configId, config_inline: configInline)
        rpc.sendJSONRPC(method: ACPRPC.orchestrateCoordinatorRunOnce, params: params, id: "coord-run-once-\(UUID().uuidString)") { (resp: CoordinatorRunOnceResponse?) in
            DispatchQueue.main.async { completion?(resp) }
        }
    }

    struct SetupStartParams: Codable { let workspace_root: String?; let session_id: String? }
    struct SetupStartResponse: Codable { let status: String; let session_id: String; let conversation_id: String }
    func orchestrateSetupStart(workspaceRoot: String?, onSessionId: @escaping (ACPSessionId) -> Void, completion: ((SetupStartResponse?) -> Void)?) {
        guard let rpc = self.rpc else { completion?(nil); return }
        let params = SetupStartParams(workspace_root: workspaceRoot, session_id: nil)
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSetupStart, params: params, id: "setup-start-\(UUID().uuidString)") { (resp: SetupStartResponse?) in
            DispatchQueue.main.async {
                if let resp = resp { onSessionId(ACPSessionId(resp.session_id)) }
                completion?(resp)
            }
        }
    }

    func fetchRecentSessions(completion: @escaping ([RecentSession]) -> Void) {
        struct EmptyParams: Codable {}
        guard let rpc = self.rpc else { completion([]); return }
        rpc.sendJSONRPC(method: "tinyvex/history.recentSessions", params: EmptyParams(), id: "recent-sessions-\(UUID().uuidString)") { (items: [RecentSession]?) in
            DispatchQueue.main.async { completion(items ?? []) }
        }
    }

    func loadSessionTimeline(sessionId: String, completion: @escaping ([ACP.Client.SessionNotificationWire]) -> Void) {
        guard let rpc = self.rpc else { completion([]); return }
        struct Params: Codable { let session_id: String }
        rpc.sendJSONRPC(method: "tinyvex/history.sessionTimeline", params: Params(session_id: sessionId), id: "session-timeline-\(UUID().uuidString)") { (timeline: [ACP.Client.SessionNotificationWire]?) in
            DispatchQueue.main.async { completion(timeline ?? []) }
        }
    }

    private func rewriteSessionId(at index: Int, to newId: ACPSessionId) {
        // This relies on TimelineStore having replaced updates appropriately; safe best-effort
        // Not exposing direct mutation API; instead, we replace by reconstructing the stored update
        // Note: We cannot directly access store's backing array; coordinator will handle remapping if needed.
        // For now, we do nothing here; BridgeManager mirrors conversion by re-sending objectWillChange when needed.
    }
}
