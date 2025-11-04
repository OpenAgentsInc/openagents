import Foundation

/*!
 Extension request/notification mirrors ACP Rust SDK `ext.rs`.
 Used for implementation-specific messages negotiated by capabilities.
*/

public struct ACPExtRequestWire: Codable {
    public var method: String
    public var params: [String: AnyEncodable]?
    public init(method: String, params: [String: AnyEncodable]? = nil) { self.method = method; self.params = params }
}

public struct ACPExtNotificationWire: Codable {
    public var method: String
    public var params: [String: AnyEncodable]?
    public init(method: String, params: [String: AnyEncodable]? = nil) { self.method = method; self.params = params }
}

