import Foundation

/*!
 Client-handled service request/response types mirroring Rust `client.rs` (subset).
 These are used when the agent calls back to the client for filesystem and terminal operations,
 and for permission requests.
*/

public extension ACP.Client {
    // MARK: - Permissions (parity with Rust client.rs)
    struct PermissionOptionId: Codable, Equatable, Hashable { public let value: String; public init(_ value: String) { self.value = value }
        public init(from decoder: Decoder) throws { let c = try decoder.singleValueContainer(); self.value = try c.decode(String.self) }
        public func encode(to encoder: Encoder) throws { var c = encoder.singleValueContainer(); try c.encode(value) }
    }
    enum PermissionOptionKind: String, Codable { case allow_once = "allow_once", allow_always = "allow_always", reject_once = "reject_once", reject_always = "reject_always" }
    struct PermissionOption: Codable, Equatable {
        public var id: PermissionOptionId
        public var name: String
        public var kind: PermissionOptionKind
        public var _meta: [String: AnyEncodable]?
        enum CodingKeys: String, CodingKey { case id = "optionId", name, kind, _meta }
        public init(id: PermissionOptionId, name: String, kind: PermissionOptionKind, _meta: [String: AnyEncodable]? = nil) {
            self.id = id; self.name = name; self.kind = kind; self._meta = _meta
        }
    }
    struct RequestPermissionRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var tool_call: ACPToolCallUpdateWire
        public var options: [PermissionOption]
        public var _meta: [String: AnyEncodable]?
        public init(session_id: ACPSessionId, tool_call: ACPToolCallUpdateWire, options: [PermissionOption], _meta: [String: AnyEncodable]? = nil) {
            self.session_id = session_id; self.tool_call = tool_call; self.options = options; self._meta = _meta
        }
    }
    enum RequestPermissionOutcome: Codable, Equatable {
        case cancelled
        case selected(option_id: PermissionOptionId)
        enum CodingKeys: String, CodingKey { case outcome, option_id }
        enum Kind: String, Codable { case cancelled, selected }
        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            switch try c.decode(Kind.self, forKey: .outcome) {
            case .cancelled: self = .cancelled
            case .selected: self = .selected(option_id: try c.decode(PermissionOptionId.self, forKey: .option_id))
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .cancelled:
                try c.encode(Kind.cancelled, forKey: .outcome)
            case .selected(let option_id):
                try c.encode(Kind.selected, forKey: .outcome)
                try c.encode(option_id, forKey: .option_id)
            }
        }
    }
    struct RequestPermissionResponse: Codable, Equatable {
        public var outcome: RequestPermissionOutcome
        public var _meta: [String: AnyEncodable]?
        public init(outcome: RequestPermissionOutcome, _meta: [String: AnyEncodable]? = nil) { self.outcome = outcome; self._meta = _meta }
    }

    // MARK: - File system
    struct ReadTextFileRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var path: String
        public var line: UInt32?
        public var limit: UInt32?
        public var _meta: [String: AnyEncodable]?
        public init(session_id: ACPSessionId, path: String, line: UInt32? = nil, limit: UInt32? = nil, _meta: [String: AnyEncodable]? = nil) {
            self.session_id = session_id; self.path = path; self.line = line; self.limit = limit; self._meta = _meta
        }
    }
    struct ReadTextFileResponse: Codable, Equatable {
        public var content: String
        public var _meta: [String: AnyEncodable]?
        public init(content: String, _meta: [String: AnyEncodable]? = nil) { self.content = content; self._meta = _meta }
    }
    struct WriteTextFileRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var path: String
        public var content: String
        public var _meta: [String: AnyEncodable]?
        public init(session_id: ACPSessionId, path: String, content: String, _meta: [String: AnyEncodable]? = nil) { self.session_id = session_id; self.path = path; self.content = content; self._meta = _meta }
    }
    struct WriteTextFileResponse: Codable, Equatable {
        public var _meta: [String: AnyEncodable]?
        public init(_meta: [String: AnyEncodable]? = nil) { self._meta = _meta }
    }

    // MARK: - Terminal (minimal)
    struct TerminalId: Codable, Equatable, Hashable { public let value: String; public init(_ v: String) { self.value = v }
        public init(from decoder: Decoder) throws { let c = try decoder.singleValueContainer(); self.value = try c.decode(String.self) }
        public func encode(to encoder: Encoder) throws { var c = encoder.singleValueContainer(); try c.encode(value) }
    }
    struct CreateTerminalRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var command: String
        public var args: [String]
        public var env: [ACP.Agent.EnvVariable]
        public var cwd: String?
        public var output_byte_limit: UInt64?
        public var _meta: [String: AnyEncodable]?
        public init(session_id: ACPSessionId, command: String, args: [String] = [], env: [ACP.Agent.EnvVariable] = [], cwd: String? = nil, output_byte_limit: UInt64? = nil, _meta: [String: AnyEncodable]? = nil) {
            self.session_id = session_id; self.command = command; self.args = args; self.env = env; self.cwd = cwd; self.output_byte_limit = output_byte_limit; self._meta = _meta
        }
    }
    struct CreateTerminalResponse: Codable, Equatable {
        public var terminal_id: TerminalId
        public var _meta: [String: AnyEncodable]?
        public init(terminal_id: TerminalId, _meta: [String: AnyEncodable]? = nil) { self.terminal_id = terminal_id; self._meta = _meta }
    }
    struct TerminalOutputRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var terminal_id: TerminalId
        public var _meta: [String: AnyEncodable]?
        public init(session_id: ACPSessionId, terminal_id: TerminalId, _meta: [String: AnyEncodable]? = nil) { self.session_id = session_id; self.terminal_id = terminal_id; self._meta = _meta }
    }
    struct TerminalExitStatus: Codable, Equatable {
        public var exit_code: UInt32?
        public var signal: String?
        public var _meta: [String: AnyEncodable]?
        public init(exit_code: UInt32? = nil, signal: String? = nil, _meta: [String: AnyEncodable]? = nil) { self.exit_code = exit_code; self.signal = signal; self._meta = _meta }
    }
    struct TerminalOutputResponse: Codable, Equatable {
        public var output: String
        public var truncated: Bool
        public var exit_status: TerminalExitStatus?
        public var _meta: [String: AnyEncodable]?
        public init(output: String, truncated: Bool, exit_status: TerminalExitStatus? = nil, _meta: [String: AnyEncodable]? = nil) { self.output = output; self.truncated = truncated; self.exit_status = exit_status; self._meta = _meta }
    }
    struct ReleaseTerminalRequest: Codable, Equatable { public var session_id: ACPSessionId; public var terminal_id: TerminalId; public var _meta: [String: AnyEncodable]?; public init(session_id: ACPSessionId, terminal_id: TerminalId, _meta: [String: AnyEncodable]? = nil) { self.session_id = session_id; self.terminal_id = terminal_id; self._meta = _meta } }
    struct ReleaseTerminalResponse: Codable, Equatable { public var _meta: [String: AnyEncodable]?; public init(_meta: [String: AnyEncodable]? = nil) { self._meta = _meta } }
    struct KillTerminalCommandRequest: Codable, Equatable { public var session_id: ACPSessionId; public var terminal_id: TerminalId; public var _meta: [String: AnyEncodable]?; public init(session_id: ACPSessionId, terminal_id: TerminalId, _meta: [String: AnyEncodable]? = nil) { self.session_id = session_id; self.terminal_id = terminal_id; self._meta = _meta } }
    struct KillTerminalCommandResponse: Codable, Equatable { public var _meta: [String: AnyEncodable]?; public init(_meta: [String: AnyEncodable]? = nil) { self._meta = _meta } }
    struct WaitForTerminalExitRequest: Codable, Equatable { public var session_id: ACPSessionId; public var terminal_id: TerminalId; public var _meta: [String: AnyEncodable]?; public init(session_id: ACPSessionId, terminal_id: TerminalId, _meta: [String: AnyEncodable]? = nil) { self.session_id = session_id; self.terminal_id = terminal_id; self._meta = _meta } }
    struct WaitForTerminalExitResponse: Codable, Equatable { public var exit_status: TerminalExitStatus; public var _meta: [String: AnyEncodable]?; public init(exit_status: TerminalExitStatus, _meta: [String: AnyEncodable]? = nil) { self.exit_status = exit_status; self._meta = _meta } }
}
