import Foundation

// MARK: - Minimal JSON-RPC 2.0 envelope types

public struct JSONRPCRequest<P: Decodable>: Decodable {
    public let jsonrpc: String
    public let id: JSONRPCId?
    public let method: String
    public let params: P?
}

public struct JSONRPCNotification<P: Decodable>: Decodable {
    public let jsonrpc: String
    public let method: String
    public let params: P?
}

public enum JSONRPCId: Codable, Hashable {
    case int(Int)
    case string(String)

    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if let i = try? c.decode(Int.self) { self = .int(i) }
        else { self = .string(try c.decode(String.self)) }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch self { case .int(let i): try c.encode(i); case .string(let s): try c.encode(s) }
    }
}

public struct JSONRPCResponse<R: Encodable>: Encodable {
    public let jsonrpc: String = "2.0"
    public let id: JSONRPCId
    public let result: R
}

public struct JSONRPCErrorResponse: Encodable {
    public struct ErrorObj: Encodable { public let code: Int; public let message: String; public let data: AnyEncodable? }
    public let jsonrpc: String = "2.0"
    public let id: JSONRPCId
    public let error: ErrorObj
}

public enum JSONRPCDecode {
    public static func decode<T: Decodable>(_ type: T.Type, from data: Data) throws -> T {
        let dec = JSONDecoder()
        return try dec.decode(T.self, from: data)
    }
}

