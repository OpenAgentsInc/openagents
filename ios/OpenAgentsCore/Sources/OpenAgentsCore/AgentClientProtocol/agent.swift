import Foundation

/*!
 Methods and notifications the agent handles/receives.

 This file mirrors the ACP Rust SDK `agent.rs` where the Agent trait and
 associated request/response types are defined. The Swift code below models
 the same request/response payloads (initialize/authenticate, etc.).
*/

public extension ACP.Agent {
    // MARK: - Initialize

    /// Describes the name and version of an implementation with an optional
    /// title for UI representation.
    /// Mirrors `Implementation` in Rust `agent.rs`.
    struct Implementation: Codable, Equatable {
        /// Intended for programmatic or logical use, can be used as display
        /// fallback if title isn’t present.
        var name: String
        /// Intended for UI and end-user contexts — optimized to be
        /// human‑readable and easily understood.
        var title: String?
        /// Version of the implementation (UI/debugging/metrics).
        var version: String
        public init(name: String, title: String? = nil, version: String) {
            self.name = name; self.title = title; self.version = version
        }
    }

    /// Capabilities supported by the client (placeholder surface).
    /// Mirrors `ClientCapabilities` in Rust `agent.rs`.
    struct ClientCapabilities: Codable {
        public var fs: FileSystemCapability
        public var terminal: Bool
        public var _meta: [String: AnyEncodable]?
        public init(fs: FileSystemCapability = .init(), terminal: Bool = false, _meta: [String: AnyEncodable]? = nil) {
            self.fs = fs; self.terminal = terminal; self._meta = _meta
        }
    }

    /// File system capabilities supported by the client
    struct FileSystemCapability: Codable {
        public var read_text_file: Bool
        public var write_text_file: Bool
        public var _meta: [String: AnyEncodable]?
        public init(read_text_file: Bool = false, write_text_file: Bool = false, _meta: [String: AnyEncodable]? = nil) {
            self.read_text_file = read_text_file; self.write_text_file = write_text_file; self._meta = _meta
        }
    }

    /// Capabilities supported by the agent (placeholder surface).
    /// Mirrors `AgentCapabilities` in Rust `agent.rs`.
    struct AgentCapabilities: Codable {
        public var load_session: Bool
        public var prompt_capabilities: PromptCapabilities
        public var mcp_capabilities: McpCapabilities
        public var _meta: [String: AnyEncodable]?
        public init(load_session: Bool = false,
                    prompt_capabilities: PromptCapabilities = .init(),
                    mcp_capabilities: McpCapabilities = .init(),
                    _meta: [String: AnyEncodable]? = nil) {
            self.load_session = load_session
            self.prompt_capabilities = prompt_capabilities
            self.mcp_capabilities = mcp_capabilities
            self._meta = _meta
        }
    }

    struct PromptCapabilities: Codable {
        public var image: Bool
        public var audio: Bool
        public var embedded_context: Bool
        public var _meta: [String: AnyEncodable]?
        public init(image: Bool = false, audio: Bool = false, embedded_context: Bool = false, _meta: [String: AnyEncodable]? = nil) {
            self.image = image; self.audio = audio; self.embedded_context = embedded_context; self._meta = _meta
        }
    }

    struct McpCapabilities: Codable {
        public var http: Bool
        public var sse: Bool
        public var _meta: [String: AnyEncodable]?
        public init(http: Bool = false, sse: Bool = false, _meta: [String: AnyEncodable]? = nil) { self.http = http; self.sse = sse; self._meta = _meta }
    }

    /// Request parameters for the `initialize` method.
    /// Sent by the client to establish connection and negotiate capabilities.
    /// Mirrors `InitializeRequest` in Rust `agent.rs`.
    struct InitializeRequest: Codable {
        /// The latest protocol version supported by the client, e.g. "0.7.0".
        var protocol_version: String
        /// Capabilities supported by the client.
        var client_capabilities: ClientCapabilities
        /// Information about the Client name and version.
        var client_info: Implementation?
        /// Extension point for implementations.
        var _meta: [String: AnyEncodable]? = nil
        public init(protocol_version: String,
                    client_capabilities: ClientCapabilities = .init(),
                    client_info: Implementation? = nil,
                    _meta: [String: AnyEncodable]? = nil) {
            self.protocol_version = protocol_version
            self.client_capabilities = client_capabilities
            self.client_info = client_info
            self._meta = _meta
        }
    }

    /// Response from the `initialize` method.
    /// Contains negotiated protocol version and agent capabilities.
    /// Mirrors `InitializeResponse` in Rust `agent.rs`.
    struct InitializeResponse: Codable {
        /// The protocol version selected by the agent (e.g., "0.7.0").
        var protocol_version: String
        /// Capabilities supported by the agent.
        var agent_capabilities: AgentCapabilities
        /// Authentication methods supported by the agent (IDs).
        var auth_methods: [String]
        /// Information about the Agent name and version.
        var agent_info: Implementation?
        /// Extension point for implementations.
        var _meta: [String: AnyEncodable]? = nil
        public init(protocol_version: String,
                    agent_capabilities: AgentCapabilities = .init(),
                    auth_methods: [String] = [],
                    agent_info: Implementation? = nil,
                    _meta: [String: AnyEncodable]? = nil) {
            self.protocol_version = protocol_version
            self.agent_capabilities = agent_capabilities
            self.auth_methods = auth_methods
            self.agent_info = agent_info
            self._meta = _meta
        }
    }
}

// MARK: - Lightweight AnyEncodable for metadata

public struct AnyEncodable: Codable {
    private let _encode: (Encoder) throws -> Void
    public init<T: Encodable>(_ value: T) { self._encode = value.encode }
    public init(from decoder: Decoder) throws { self._encode = { _ in } }
    public func encode(to encoder: Encoder) throws { try _encode(encoder) }
}

// MARK: - Session Mode and Cancel (parity with Rust agent.rs)

public extension ACP.Agent {
    struct SetSessionModeRequest: Codable, Equatable {
        public var session_id: ACPSessionId
        public var mode_id: ACPSessionModeId
        public var _meta: [String: AnyEncodable]? = nil
        public init(session_id: ACPSessionId, mode_id: ACPSessionModeId, _meta: [String: AnyEncodable]? = nil) {
            self.session_id = session_id; self.mode_id = mode_id; self._meta = _meta
        }
    }
    struct SetSessionModeResponse: Codable, Equatable {
        public var _meta: [String: AnyEncodable]? = nil
        public init(_meta: [String: AnyEncodable]? = nil) { self._meta = _meta }
    }
}

// MARK: - Env + headers (for MCP / terminal env)
public extension ACP.Agent {
    struct EnvVariable: Codable {
        public var name: String
        public var value: String
        public var _meta: [String: AnyEncodable]?
        public init(name: String, value: String, _meta: [String: AnyEncodable]? = nil) {
            self.name = name; self.value = value; self._meta = _meta
        }
    }
    struct HttpHeader: Codable {
        public var name: String
        public var value: String
        public var _meta: [String: AnyEncodable]?
        public init(name: String, value: String, _meta: [String: AnyEncodable]? = nil) {
            self.name = name; self.value = value; self._meta = _meta
        }
    }
}
// MARK: - MCP Servers (parity with Rust agent.rs)
public extension ACP.Agent {
    enum McpServer: Codable, Equatable {
        case http(name: String, url: String, headers: [HttpHeader])
        case sse(name: String, url: String, headers: [HttpHeader])
        case stdio(name: String, command: String, args: [String], env: [EnvVariable])

        private enum CodingKeys: String, CodingKey { case type }
        private enum Kind: String, Codable { case http, sse, stdio }

        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            let kind = try c.decode(Kind.self, forKey: .type)
            switch kind {
            case .http:
                struct Http: Codable { let name: String; let url: String; let headers: [HttpHeader] }
                let v = try Http(from: decoder)
                self = .http(name: v.name, url: v.url, headers: v.headers)
            case .sse:
                struct Sse: Codable { let name: String; let url: String; let headers: [HttpHeader] }
                let v = try Sse(from: decoder)
                self = .sse(name: v.name, url: v.url, headers: v.headers)
            case .stdio:
                struct Stdio: Codable { let name: String; let command: String; let args: [String]; let env: [EnvVariable] }
                let v = try Stdio(from: decoder)
                self = .stdio(name: v.name, command: v.command, args: v.args, env: v.env)
            }
        }

        public func encode(to encoder: Encoder) throws {
            switch self {
            case .http(let name, let url, let headers):
                struct Http: Codable { let type: String; let name: String; let url: String; let headers: [HttpHeader] }
                try Http(type: "http", name: name, url: url, headers: headers).encode(to: encoder)
            case .sse(let name, let url, let headers):
                struct Sse: Codable { let type: String; let name: String; let url: String; let headers: [HttpHeader] }
                try Sse(type: "sse", name: name, url: url, headers: headers).encode(to: encoder)
            case .stdio(let name, let command, let args, let env):
                struct Stdio: Codable { let type: String; let name: String; let command: String; let args: [String]; let env: [EnvVariable] }
                try Stdio(type: "stdio", name: name, command: command, args: args, env: env).encode(to: encoder)
            }
        }
    }
}
