import Foundation

/*!
 Tool call and updates mirroring the ACP Rust SDK `tool_call.rs`.
*/

public struct ACPToolCallWire: Codable {
    public var call_id: String
    public var name: String
    public var arguments: [String: AnyEncodable]? // structured args
    public var _meta: [String: AnyEncodable]? = nil
    public init(call_id: String, name: String, arguments: [String: AnyEncodable]? = nil, _meta: [String: AnyEncodable]? = nil) {
        self.call_id = call_id
        self.name = name
        self.arguments = arguments
        self._meta = _meta
    }
}

public struct ACPToolCallUpdateWire: Codable {
    public enum Status: String, Codable { case started, completed, error }
    public var call_id: String
    public var status: Status
    public var output: AnyEncodable?
    public var error: String?
    public var _meta: [String: AnyEncodable]? = nil
    public init(call_id: String, status: Status, output: AnyEncodable? = nil, error: String? = nil, _meta: [String: AnyEncodable]? = nil) {
        self.call_id = call_id
        self.status = status
        self.output = output
        self.error = error
        self._meta = _meta
    }
}

