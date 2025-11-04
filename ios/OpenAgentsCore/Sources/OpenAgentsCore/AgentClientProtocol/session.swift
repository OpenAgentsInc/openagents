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

public enum ACPSessionModeId: String, Codable, CaseIterable { case default_mode }

public extension ACP.Agent {
    struct SessionNewRequest: Codable {
        public var mode_id: ACPSessionModeId?
        public init(mode_id: ACPSessionModeId? = nil) { self.mode_id = mode_id }
    }
    struct SessionNewResponse: Codable {
        public var session_id: ACPSessionId
        public init(session_id: ACPSessionId) { self.session_id = session_id }
    }

    struct SessionPromptRequest: Codable {
        public var session_id: ACPSessionId
        public var content: [ACP.Client.ContentBlock]
        public init(session_id: ACPSessionId, content: [ACP.Client.ContentBlock]) {
            self.session_id = session_id; self.content = content
        }
    }
}

public extension ACP.Client {
    struct SessionNotificationWire: Codable {
        public var session_id: ACPSessionId
        public var update: SessionUpdate
        public init(session_id: ACPSessionId, update: SessionUpdate) { self.session_id = session_id; self.update = update }
    }
}

