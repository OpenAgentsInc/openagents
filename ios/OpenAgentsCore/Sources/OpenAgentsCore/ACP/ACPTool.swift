import Foundation

// MARK: - Tool calls and results

public struct ACPToolCall: Equatable, Codable {
    public let type: String = "tool_call"
    public var id: String
    public var tool_name: String
    public var arguments: JSONValue // structured args, typed without Any
    public var ts: Int64?

    public init(id: String, tool_name: String, arguments: JSONValue, ts: Int64? = nil) {
        self.id = id
        self.tool_name = tool_name
        self.arguments = arguments
        self.ts = ts
    }
}

public struct ACPToolResult: Equatable, Codable {
    public let type: String = "tool_result"
    public var call_id: String
    public var ok: Bool
    public var result: JSONValue?
    public var error: String?
    public var ts: Int64?

    public init(call_id: String, ok: Bool, result: JSONValue? = nil, error: String? = nil, ts: Int64? = nil) {
        self.call_id = call_id
        self.ok = ok
        self.result = result
        self.error = error
        self.ts = ts
    }
}

