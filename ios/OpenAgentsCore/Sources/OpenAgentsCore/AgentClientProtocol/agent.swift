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
    struct ClientCapabilities: Codable, Equatable {
        public init() {}
    }

    /// Capabilities supported by the agent (placeholder surface).
    /// Mirrors `AgentCapabilities` in Rust `agent.rs`.
    struct AgentCapabilities: Codable, Equatable {
        public init() {}
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

