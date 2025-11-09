import Foundation
import OpenAgentsCore

#if os(macOS)
final class LocalJsonRpcClient: JSONRPCSending {
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

            case ACPRPC.orchestrateExploreStart:
                // Not implemented locally yet; allow caller to treat as unavailable
                result = nil

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

    private static func decodeCancel<P: Codable>(_ p: P) -> ACPSessionId? {
        guard let data = try? JSONEncoder().encode(p),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let sid = obj["session_id"] as? String
        else { return nil }
        return ACPSessionId(sid)
    }
}
#endif

