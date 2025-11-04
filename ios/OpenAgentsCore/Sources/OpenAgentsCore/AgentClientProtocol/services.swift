import Foundation

/*!
 Client-handled service request/response types mirroring Rust `client.rs` (subset).
 These are used when the agent calls back to the client for filesystem and terminal operations,
 and for permission requests.
*/

public extension ACP.Client {
    // MARK: - Permissions
    struct RequestPermissionRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var permission: String
        public var rationale: String?
        public init(session_id: ACPSessionId, permission: String, rationale: String? = nil) {
            self.session_id = session_id; self.permission = permission; self.rationale = rationale
        }
    }
    struct RequestPermissionResponse: Codable, Equatable {
        public struct Outcome: Codable, Equatable { public var outcome: String }
        public var outcome: Outcome
        public init(outcome: Outcome) { self.outcome = outcome }
    }

    // MARK: - File system
    struct ReadTextFileRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var uri: String
        public init(session_id: ACPSessionId, uri: String) { self.session_id = session_id; self.uri = uri }
    }
    struct ReadTextFileResponse: Codable, Equatable {
        public var text: String
        public init(text: String) { self.text = text }
    }
    struct WriteTextFileRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var uri: String
        public var text: String
        public init(session_id: ACPSessionId, uri: String, text: String) { self.session_id = session_id; self.uri = uri; self.text = text }
    }
    struct WriteTextFileResponse: Codable, Equatable {
        public var ok: Bool
        public init(ok: Bool = true) { self.ok = ok }
    }

    // MARK: - Terminal (minimal)
    struct TerminalRunRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var command: [String]
        public var cwd: String?
        public var env: [String: String]?
        public var output_byte_limit: Int?
        public init(session_id: ACPSessionId, command: [String], cwd: String? = nil, env: [String: String]? = nil, output_byte_limit: Int? = nil) {
            self.session_id = session_id; self.command = command; self.cwd = cwd; self.env = env; self.output_byte_limit = output_byte_limit
        }
    }
    struct TerminalRunResponse: Codable, Equatable {
        public var output: String
        public var truncated: Bool
        public var exit_status: Int32?
        public init(output: String, truncated: Bool, exit_status: Int32? = nil) { self.output = output; self.truncated = truncated; self.exit_status = exit_status }
    }
}

