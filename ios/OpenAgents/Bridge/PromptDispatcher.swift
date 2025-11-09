import Foundation
import OpenAgentsCore

final class PromptDispatcher: PromptDispatching {
    private weak var rpc: JSONRPCSending?
    private let timeline: TimelineStoring
    private struct EmptyResult: Codable {}

    // Track optimistic echo index to adjust session id when server creates a new session
    private var pendingUserEchoIndex: Int?

    init(rpc: JSONRPCSending?, timeline: TimelineStoring) {
        self.rpc = rpc
        self.timeline = timeline
    }

    func sendPrompt(text: String, desiredMode: ACPSessionModeId?, getSessionId: () -> ACPSessionId?, setSessionId: @escaping (ACPSessionId) -> Void) {
        guard let rpc = self.rpc else { return }

        let parts: [ACP.Client.ContentBlock] = [.text(.init(text: text))]
        // Optimistic UI echo
        let sid = getSessionId() ?? ACPSessionId("pending")
        pendingUserEchoIndex = timeline.appendOptimisticUserMessage(text: text, sessionId: sid)

        if let existing = getSessionId() {
            if let mode = desiredMode { self.setSessionMode(mode, getSessionId: { existing }) }
            let req = ACP.Agent.SessionPromptRequest(session_id: existing, content: parts)
            rpc.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)") { (_: EmptyResult?) in }
            return
        }

        // No session yet: create one then prompt
        rpc.sendJSONRPC(method: ACPRPC.sessionNew, params: ACP.Agent.SessionNewRequest(), id: "session-new-\(UUID().uuidString)") { (resp: ACP.Agent.SessionNewResponse?) in
            guard let resp = resp else { return }
            DispatchQueue.main.async {
                setSessionId(resp.session_id)
                // convert optimistic echo to real session id
                if let idx = self.pendingUserEchoIndex {
                    self.rewriteSessionId(at: idx, to: resp.session_id)
                    self.pendingUserEchoIndex = nil
                }
            }
            // Optionally set mode then prompt
            if let mode = desiredMode {
                struct SetModeReq: Codable { let session_id: ACPSessionId; let mode_id: ACPSessionModeId }
                rpc.sendJSONRPC(method: ACPRPC.sessionSetMode, params: SetModeReq(session_id: resp.session_id, mode_id: mode), id: "session-set-mode-\(UUID().uuidString)") { (_: ACP.Agent.SetSessionModeResponse?) in
                    let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                    rpc.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)") { (_: EmptyResult?) in }
                }
            } else {
                let req = ACP.Agent.SessionPromptRequest(session_id: resp.session_id, content: parts)
                rpc.sendJSONRPC(method: ACPRPC.sessionPrompt, params: req, id: "session-prompt-\(UUID().uuidString)") { (_: EmptyResult?) in }
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
