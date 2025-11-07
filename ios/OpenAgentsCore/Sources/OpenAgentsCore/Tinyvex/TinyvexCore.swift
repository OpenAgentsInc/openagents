import Foundation

public enum TinyvexError: Error, Codable {
    case server(code: String, message: String)
    case decoding(String)
    case transport(String)
}

public struct TinyvexFeatures: Codable {
    public let chunks: Bool
    public let maxChunkBytes: Int
    public init(chunks: Bool = true, maxChunkBytes: Int = 64 * 1024) {
        self.chunks = chunks
        self.maxChunkBytes = maxChunkBytes
    }
}

public enum TinyvexMethods {
    // ACP methods (pass-through contract)
    public static let initialize = ACPRPC.initialize
    public static let sessionNew = ACPRPC.sessionNew
    public static let sessionPrompt = ACPRPC.sessionPrompt
    public static let sessionCancel = ACPRPC.sessionCancel
    public static let sessionUpdate = ACPRPC.sessionUpdate

    // Tinyvex utility methods
    public static let connect = "tinyvex/connect"
    public static let subscribe = "tinyvex/subscribe"
    public static let unsubscribe = "tinyvex/unsubscribe"
    public static let mutation = "tinyvex/mutation"
    public static let action = "tinyvex/action"
    public static let authSetToken = "tinyvex/auth.setToken"
    public static let ping = "tinyvex/ping"
    public static let historySessionUpdates = "tinyvex/history.sessionUpdates"
}

// Utility types
public struct TinyvexConnectParams: Codable { public struct Client: Codable { public let name: String; public let version: String }; public let client: Client; public let clockSkewHint: Int64? }
public struct TinyvexConnectResult: Codable { public let serverVersion: String; public let nowTs: Int64; public let features: TinyvexFeatures; public let sessionId: String? }

public struct TinyvexHistoryParams: Codable { public let session_id: String; public let since_seq: Int64?; public let since_ts: Int64?; public let limit: Int? }

public struct TinyvexDataNotification<T: Codable>: Codable { public let subId: String; public let seq: Int64; public let value: T }

