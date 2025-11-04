import Foundation

// MARK: - Event wrapper for timeline assembly

public enum ACPEventKind: String, Codable { case message, tool_call, tool_result, plan_state }

public struct ACPEvent: Equatable, Codable {
    public var id: String
    public var ts: Int64
    public var kind: ACPEventKind
    public var message: ACPMessage?
    public var tool_call: ACPToolCall?
    public var tool_result: ACPToolResult?
    public var plan_state: ACPPlanState?

    public init(id: String, ts: Int64, message: ACPMessage) {
        self.id = id
        self.ts = ts
        self.kind = .message
        self.message = message
    }
    public init(id: String, ts: Int64, tool_call: ACPToolCall) {
        self.id = id
        self.ts = ts
        self.kind = .tool_call
        self.tool_call = tool_call
    }
    public init(id: String, ts: Int64, tool_result: ACPToolResult) {
        self.id = id
        self.ts = ts
        self.kind = .tool_result
        self.tool_result = tool_result
    }
    public init(id: String, ts: Int64, plan_state: ACPPlanState) {
        self.id = id
        self.ts = ts
        self.kind = .plan_state
        self.plan_state = plan_state
    }
}

