#if os(macOS)
import Foundation

/// Describes how an agent executes (CLI process, native Swift, remote API, etc.)
public enum AgentExecutionMode: String, Codable, Sendable {
    /// Agent runs as an external CLI process
    case cli
    /// Agent runs as native Swift code (in-process)
    case native
    /// Agent runs as a remote API call
    case remote
}

/// Describes how an agent streams output
public enum AgentStreamingMode: String, Codable, Sendable {
    /// Agent outputs JSONL (one JSON object per line)
    case jsonl
    /// Agent outputs ACP messages directly
    case acp
    /// Agent outputs plain text
    case text
}

/// Capabilities supported by an agent provider
public struct AgentCapabilities: Codable, Sendable {
    /// How the agent executes
    public var executionMode: AgentExecutionMode

    /// How the agent streams output
    public var streamingMode: AgentStreamingMode

    /// Whether the agent supports resuming previous sessions
    public var supportsResume: Bool

    /// Whether the agent supports working directory configuration
    public var supportsWorkingDirectory: Bool

    /// Whether the agent requires external binaries to be installed
    public var requiresExternalBinary: Bool

    /// Whether the agent supports MCP servers
    public var supportsMCP: Bool

    public init(
        executionMode: AgentExecutionMode,
        streamingMode: AgentStreamingMode,
        supportsResume: Bool = false,
        supportsWorkingDirectory: Bool = false,
        requiresExternalBinary: Bool = false,
        supportsMCP: Bool = false
    ) {
        self.executionMode = executionMode
        self.streamingMode = streamingMode
        self.supportsResume = supportsResume
        self.supportsWorkingDirectory = supportsWorkingDirectory
        self.requiresExternalBinary = requiresExternalBinary
        self.supportsMCP = supportsMCP
    }
}

/// Context passed to agents when starting or resuming a session
public struct AgentContext: Sendable {
    /// Working directory for the agent (if supported)
    public var workingDirectory: URL?

    /// MCP servers to configure (if supported)
    public var mcpServers: [ACP.Agent.McpServer]?

    /// Client that initiated the request (for sending responses)
    public var client: DesktopWebSocketServer.Client?

    /// Additional metadata
    public var metadata: [String: String]

    public init(
        workingDirectory: URL? = nil,
        mcpServers: [ACP.Agent.McpServer]? = nil,
        client: DesktopWebSocketServer.Client? = nil,
        metadata: [String: String] = [:]
    ) {
        self.workingDirectory = workingDirectory
        self.mcpServers = mcpServers
        self.client = client
        self.metadata = metadata
    }
}

/// Handle to a running agent session
public struct AgentHandle: Sendable {
    /// The session ID
    public let sessionId: ACPSessionId

    /// The agent mode
    public let mode: ACPSessionModeId

    /// Process ID (for CLI agents)
    public let processId: Int32?

    /// Thread ID (provider-specific identifier for resume)
    public let threadId: String?

    /// Whether this session has been started (vs just created)
    public let isStarted: Bool

    /// Additional metadata
    public let metadata: [String: String]

    public init(
        sessionId: ACPSessionId,
        mode: ACPSessionModeId,
        processId: Int32? = nil,
        threadId: String? = nil,
        isStarted: Bool = false,
        metadata: [String: String] = [:]
    ) {
        self.sessionId = sessionId
        self.mode = mode
        self.processId = processId
        self.threadId = threadId
        self.isStarted = isStarted
        self.metadata = metadata
    }
}
#endif
