import Foundation

/*!
 Session types mirroring ACP Rust SDK session lifecycle (subset).
*/

public struct ACPSessionId: Codable, Hashable, Equatable {
    public let value: String
    public init(_ value: String) { self.value = value }
    public init(from decoder: Decoder) throws {
        let c = try decoder.singleValueContainer()
        let s = try c.decode(String.self)
        self.value = s
    }
    public func encode(to encoder: Encoder) throws {
        var c = encoder.singleValueContainer()
        try c.encode(value)
    }
}

public enum ACPSessionModeId: String, Codable, CaseIterable {
    case default_mode = "default_mode"
    case claude_code = "claude-code"
    case codex = "codex"
}

public extension ACP.Agent {
    struct SessionNewRequest: Codable {
        public var cwd: String?
        public var mcp_servers: [ACP.Agent.McpServer]?
        public var mode_id: ACPSessionModeId?
        public init(cwd: String? = nil, mcp_servers: [ACP.Agent.McpServer]? = nil, mode_id: ACPSessionModeId? = nil) {
            self.cwd = cwd; self.mcp_servers = mcp_servers; self.mode_id = mode_id
        }
    }
    struct SessionNewResponse: Codable {
        public var session_id: ACPSessionId
        public init(session_id: ACPSessionId) { self.session_id = session_id }
    }

    struct SessionPromptRequest: Codable {
        public var session_id: ACPSessionId
        public var content: [ACP.Client.ContentBlock]
        enum CodingKeys: String, CodingKey { case session_id, prompt }
        public init(session_id: ACPSessionId, content: [ACP.Client.ContentBlock]) {
            self.session_id = session_id; self.content = content
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(session_id, forKey: .session_id)
            try c.encode(content, forKey: .prompt)
        }
        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            self.session_id = try c.decode(ACPSessionId.self, forKey: .session_id)
            self.content = (try? c.decode([ACP.Client.ContentBlock].self, forKey: .prompt)) ?? []
        }
    }

    struct SessionLoadRequest: Codable {
        public var mcp_servers: [ACP.Agent.McpServer]
        public var cwd: String
        public var session_id: ACPSessionId
        public var _meta: [String: AnyEncodable]?
        public init(mcp_servers: [ACP.Agent.McpServer], cwd: String, session_id: ACPSessionId, _meta: [String: AnyEncodable]? = nil) {
            self.mcp_servers = mcp_servers; self.cwd = cwd; self.session_id = session_id; self._meta = _meta
        }
    }
    struct SessionLoadResponse: Codable {
        public var _meta: [String: AnyEncodable]?
        public init(_meta: [String: AnyEncodable]? = nil) { self._meta = _meta }
    }
}

public extension ACP.Client {
    public struct SessionNotificationWire: Codable {
        public var session_id: ACPSessionId
        public var update: SessionUpdate
        public var _meta: [String: AnyEncodable]?
        public init(session_id: ACPSessionId, update: SessionUpdate, _meta: [String: AnyEncodable]? = nil) { self.session_id = session_id; self.update = update; self._meta = _meta }
    }
}
