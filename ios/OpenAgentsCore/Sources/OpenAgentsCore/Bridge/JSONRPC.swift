import Foundation

public enum JSONRPC {
    public struct ID: Codable, Equatable, Hashable {
        public let value: String
        public init(_ value: String) { self.value = value }
        public init(from decoder: Decoder) throws {
            let c = try decoder.singleValueContainer()
            if let i = try? c.decode(Int.self) { self.value = String(i); return }
            if let s = try? c.decode(String.self) { self.value = s; return }
            throw DecodingError.dataCorruptedError(in: c, debugDescription: "Unsupported id type")
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.singleValueContainer()
            if let i = Int(value) { try c.encode(i) } else { try c.encode(value) }
        }
    }

    public struct Request<P: Codable>: Codable {
        public let jsonrpc: String = "2.0"
        public let id: ID
        public let method: String
        public let params: P
        public init(id: ID, method: String, params: P) {
            self.id = id
            self.method = method
            self.params = params
        }
    }

    public struct Response<R: Codable>: Codable {
        public let jsonrpc: String = "2.0"
        public let id: ID
        public let result: R
        public init(id: ID, result: R) {
            self.id = id
            self.result = result
        }
    }

    public struct Notification<P: Codable>: Codable {
        public let jsonrpc: String = "2.0"
        public let method: String
        public let params: P
        public init(method: String, params: P) {
            self.method = method
            self.params = params
        }
    }

    public struct ErrorObject: Codable {
        public let code: Int
        public let message: String
        public let data: [String: AnyCodable]?
        public init(code: Int, message: String, data: [String: AnyCodable]? = nil) {
            self.code = code
            self.message = message
            self.data = data
        }
    }

    public struct ErrorResponse: Codable {
        public let jsonrpc: String = "2.0"
        public let id: ID?
        public let error: ErrorObject
        public init(id: ID?, error: ErrorObject) {
            self.id = id
            self.error = error
        }
    }
}

// Minimal AnyCodable for JSONRPC error data
public struct AnyCodable: Codable {
    public let value: Any
    public init(_ value: Any) { self.value = value }
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        if let b = try? c.decode(Bool.self) { value = b; return }
        if let i = try? c.decode(Int.self) { value = i; return }
        if let d = try? c.decode(Double.self) { value = d; return }
        if let s = try? c.decode(String.self) { value = s; return }
        if let arr = try? c.decode([AnyCodable].self) { value = arr.map { $0.value }; return }
        if let obj = try? c.decode([String: AnyCodable].self) { value = obj.mapValues { $0.value }; return }
        value = NSNull()
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        switch value {
        case let b as Bool: try c.encode(b)
        case let i as Int: try c.encode(i)
        case let d as Double: try c.encode(d)
        case let s as String: try c.encode(s)
        case let arr as [Any]: try c.encode(arr.map { AnyCodable($0) })
        case let dict as [String: Any]: try c.encode(dict.mapValues { AnyCodable($0) })
        default: try c.encodeNil()
        }
    }
}

// ACP Initialize types now live under `ACP.Agent` (see ACPProtocol/agent.swift)
