import Foundation
import OpenAgentsCore

#if os(macOS)
final class LocalJsonRpcClient: JSONRPCSending {
    private struct OkResp: Codable { let ok: Bool }
    private struct TitleResp: Codable { let title: String? }
    private struct ConfigSetResp: Codable { let success: Bool; let config_id: String; let updated_at: Int64 }
    private struct ConfigActivateResp: Codable { let success: Bool; let active_config_id: String }
    private struct ReloadResp: Codable { let success: Bool; let message: String }
    private struct BindResp: Codable { let success: Bool; let active_config_id: String }
    private struct SetupStartResp: Codable { let status: String; let session_id: String; let conversation_id: String }
    private let server: DesktopWebSocketServer

    init(server: DesktopWebSocketServer) {
        self.server = server
    }

    func sendJSONRPC<P: Codable, R: Codable>(method: String, params: P, id: String, completion: @escaping (R?) -> Void) {
        Task {
            let result: R?
            switch method {
            case ACPRPC.sessionNew:
                let resp = server.localSessionNew()
                result = Self.bridge(resp, as: R.self)

            case ACPRPC.sessionSetMode:
                let (sid, mode) = Self.decodeSetMode(params)
                if let sid = sid, let mode = mode {
                    await server.localSessionSetMode(sessionId: sid, mode: mode)
                    let resp = ACP.Agent.SetSessionModeResponse()
                    result = Self.bridge(resp, as: R.self)
                } else {
                    result = nil
                }

            case ACPRPC.sessionPrompt:
                if let req: ACP.Agent.SessionPromptRequest = Self.reencode(params) {
                    try? await server.localSessionPrompt(request: req)
                    // Acknowledge with empty object for callers expecting a body
                    result = Self.empty(as: R.self)
                } else {
                    result = nil
                }

            case "tinyvex/history.recentSessions":
                let items = (try? await server.localHistoryRecentSessions()) ?? []
                result = Self.bridge(items, as: R.self)

            case "tinyvex/history.sessionTimeline":
                let (sessionId, limit) = Self.decodeTimeline(params)
                let arr = (try? await server.localHistorySessionTimeline(sessionId: sessionId ?? "", limit: limit)) ?? []
                result = Self.bridge(arr, as: R.self)

            case "tinyvex/history.setSessionTitle":
                if let obj = Self.asDict(params),
                   let sid = obj["session_id"] as? String,
                   let title = obj["title"] as? String {
                    let now = Int64(Date().timeIntervalSince1970 * 1000)
                    await server.localSetSessionTitle(sessionId: sid, title: title, updatedAt: now)
                    result = Self.bridge(OkResp(ok: true), as: R.self)
                } else {
                    result = nil
                }

            case "tinyvex/history.getSessionTitle":
                if let obj = Self.asDict(params), let sid = obj["session_id"] as? String {
                    let t = await server.localGetSessionTitle(sessionId: sid)
                    result = Self.bridge(TitleResp(title: t ?? nil), as: R.self)
                } else {
                    result = nil
                }
            case "tinyvex/history.clearSessionTitle":
                if let obj = Self.asDict(params), let sid = obj["session_id"] as? String {
                    await server.localClearSessionTitle(sessionId: sid)
                    result = Self.bridge(OkResp(ok: true), as: R.self)
                } else {
                    result = nil
                }

            case ACPRPC.orchestrateExploreStart:
                // Not implemented locally yet; allow caller to treat as unavailable
                result = nil
            case ACPRPC.orchestrateSetupStart:
                // { workspace_root?: String, session_id?: String }
                if let obj = Self.asDict(params) {
                    let ws = obj["workspace_root"] as? String
                    let sid = (obj["session_id"] as? String).map { ACPSessionId($0) }
                    let out = await server.localSetupStart(workspaceRoot: ws, sessionId: sid)
                    result = Self.bridge(out, as: R.self)
                } else {
                    let out = await server.localSetupStart(workspaceRoot: nil, sessionId: nil)
                    result = Self.bridge(out, as: R.self)
                }
            case ACPRPC.orchestrateSetupStatus:
                // Local status can be mapped via handleSetupStatus if needed later; return nil for now
                result = nil
            case ACPRPC.orchestrateSetupAbort:
                // Not implemented in local client for now
                result = nil
            case ACPRPC.orchestrateConfigSet:
                if let data = try? JSONEncoder().encode(params),
                   let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let cfgObj = obj["config"],
                   let cfgData = try? JSONSerialization.data(withJSONObject: cfgObj),
                   let cfg = try? JSONDecoder().decode(OrchestrationConfig.self, from: cfgData) {
                    let ok = await server.localConfigSet(cfg)
                    result = Self.bridge(ConfigSetResp(success: ok, config_id: cfg.id, updated_at: cfg.updatedAt), as: R.self)
                } else { result = nil }
            case ACPRPC.orchestrateConfigActivate:
                if let data = try? JSONEncoder().encode(params),
                   let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                   let id = obj["id"] as? String,
                   let root = obj["workspace_root"] as? String {
                    let ok = await server.localConfigActivate(id: id, workspaceRoot: root)
                    result = Self.bridge(ConfigActivateResp(success: ok, active_config_id: id), as: R.self)
                } else { result = nil }
            case ACPRPC.orchestrateSchedulerStatus:
                let status = await server.localSchedulerStatus()
                result = Self.bridge(status, as: R.self)
            case ACPRPC.orchestrateSchedulerReload:
                let out = await server.localSchedulerReload()
                result = Self.bridge(ReloadResp(success: out.success, message: out.message), as: R.self)
            case ACPRPC.orchestrateSchedulerRunNow, ACPRPC.orchestrateSchedulerAdvance:
                let out = await server.localSchedulerRunNow()
                result = Self.bridge(out, as: R.self)

            case ACPRPC.orchestrateCoordinatorRunOnce:
                // Support optional inline config
                if let obj = Self.asDict(params) {
                    var cfg: OrchestrationConfig? = nil
                    if let inline = obj["config_inline"],
                       let data = try? JSONSerialization.data(withJSONObject: inline),
                       let dec = try? JSONDecoder().decode(OrchestrationConfig.self, from: data) {
                        cfg = dec
                    }
                    let out = await server.localCoordinatorRunOnce(config: cfg)
                    result = Self.bridge(out, as: R.self)
                } else {
                    let out = await server.localCoordinatorRunOnce(config: nil)
                    result = Self.bridge(out, as: R.self)
                }
            case ACPRPC.orchestrateCoordinatorStatus:
                let out = await server.localCoordinatorStatus()
                result = Self.bridge(out, as: R.self)
            case ACPRPC.orchestrateSchedulerBind:
                if let obj = Self.asDict(params),
                   let id = obj["config_id"] as? String,
                   let root = obj["workspace_root"] as? String {
                    let ok = await server.localConfigActivate(id: id, workspaceRoot: root)
                    result = Self.bridge(BindResp(success: ok, active_config_id: id), as: R.self)
                } else {
                    result = nil
                }

            default:
                result = nil
            }
            DispatchQueue.main.async { completion(result) }
        }
    }

    func sendJSONRPCNotification<P: Codable>(method: String, params: P) {
        Task {
            switch method {
            case ACPRPC.sessionCancel:
                if let sid = Self.decodeCancel(params) {
                    await server.localSessionCancel(sessionId: sid)
                }
            default:
                break
            }
        }
    }

    // MARK: - Helpers
    private static func bridge<T: Codable, U: Codable>(_ value: T, as: U.Type) -> U? {
        guard let data = try? JSONEncoder().encode(value) else { return nil }
        return try? JSONDecoder().decode(U.self, from: data)
    }

    private static func empty<U: Codable>(as: U.Type) -> U? {
        return try? JSONDecoder().decode(U.self, from: Data("{}".utf8))
    }

    private static func reencode<P: Codable, T: Decodable>(_ p: P) -> T? {
        guard let data = try? JSONEncoder().encode(p) else { return nil }
        return try? JSONDecoder().decode(T.self, from: data)
    }

    private static func decodeSetMode<P: Codable>(_ p: P) -> (ACPSessionId?, ACPSessionModeId?) {
        guard let data = try? JSONEncoder().encode(p),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return (nil, nil) }
        let sidStr = obj["session_id"] as? String
        let modeStr = obj["mode_id"] as? String
        let sid = sidStr.map { ACPSessionId($0) }
        let mode = modeStr.map { ACPSessionModeId(rawValue: $0) ?? .default_mode }
        return (sid, mode)
    }

    private static func decodeTimeline<P: Codable>(_ p: P) -> (String?, Int?) {
        guard let data = try? JSONEncoder().encode(p),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return (nil, nil) }
        let sid = obj["session_id"] as? String
        let limit = obj["limit"] as? Int
        return (sid, limit)
    }

    private static func asDict<P: Codable>(_ p: P) -> [String: Any]? {
        guard let data = try? JSONEncoder().encode(p),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return obj
    }

    private static func decodeCancel<P: Codable>(_ p: P) -> ACPSessionId? {
        guard let data = try? JSONEncoder().encode(p),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let sid = obj["session_id"] as? String
        else { return nil }
        return ACPSessionId(sid)
    }
}
#endif
