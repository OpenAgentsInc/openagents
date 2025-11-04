import Foundation

/*!
 JSON-RPC error object and ACP-specific error codes, mirroring Rust `error.rs`.
*/

public struct ACPError: Codable, Equatable, Error {
    public var code: Int
    public var message: String
    public var data: AnyEncodable?
    public init(code: Int, message: String, data: AnyEncodable? = nil) {
        self.code = code; self.message = message; self.data = data
    }
}

public enum ACPErrorCode {
    public static let parseError = (-32700, "Parse error")
    public static let invalidRequest = (-32600, "Invalid Request")
    public static let methodNotFound = (-32601, "Method not found")
    public static let invalidParams = (-32602, "Invalid params")
    public static let internalError = (-32603, "Internal error")
    // ACP-specific reserved range (-32000..-32099)
    public static let authRequired = (-32000, "Authentication required")
    public static let resourceNotFound = (-32002, "Resource not found")
}

