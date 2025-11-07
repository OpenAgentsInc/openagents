#if os(macOS)
import Foundation

/// Protocol that all agent providers must implement.
/// Agents can be CLI-based (Claude Code, Codex), native Swift (OpenAgents Coder),
/// or remote APIs. This protocol abstracts the execution model while ensuring
/// all agents speak ACP.
public protocol AgentProvider: Sendable {
    /// Unique identifier for this agent (maps to ACPSessionModeId)
    var id: ACPSessionModeId { get }

    /// Human-readable display name for UI
    var displayName: String { get }

    /// Capabilities supported by this agent
    var capabilities: AgentCapabilities { get }

    /// Check if this agent is available (CLI installed, APIs accessible, etc.)
    func isAvailable() async -> Bool

    /// Start a new session with this agent
    /// - Parameters:
    ///   - sessionId: The ACP session ID to bind to
    ///   - prompt: The initial user prompt
    ///   - context: Execution context (working directory, client, etc.)
    ///   - updateHub: Canonical path for broadcasting ACP updates
    /// - Returns: Handle to the running session
    /// - Throws: AgentProviderError if the agent cannot be started
    func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle

    /// Resume an existing session with a new prompt
    /// - Parameters:
    ///   - sessionId: The ACP session ID
    ///   - prompt: The new user prompt
    ///   - handle: Handle from previous start() call
    ///   - context: Execution context
    ///   - updateHub: Canonical path for broadcasting ACP updates
    /// - Throws: AgentProviderError if the agent cannot resume
    func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws

    /// Cancel a running session
    /// - Parameters:
    ///   - sessionId: The ACP session ID
    ///   - handle: Handle to the running session
    func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async
}

/// Errors that can occur when working with agent providers
public enum AgentProviderError: Error, CustomStringConvertible {
    /// Agent binary not found (for CLI agents)
    case binaryNotFound(String)

    /// Agent is not available (not installed, API down, etc.)
    case notAvailable(String)

    /// Agent failed to start
    case startFailed(String)

    /// Agent failed to resume
    case resumeFailed(String)

    /// Invalid configuration
    case invalidConfiguration(String)

    /// Unsupported operation for this agent
    case unsupported(String)

    public var description: String {
        switch self {
        case .binaryNotFound(let msg):
            return "Binary not found: \(msg)"
        case .notAvailable(let msg):
            return "Agent not available: \(msg)"
        case .startFailed(let msg):
            return "Failed to start agent: \(msg)"
        case .resumeFailed(let msg):
            return "Failed to resume agent: \(msg)"
        case .invalidConfiguration(let msg):
            return "Invalid configuration: \(msg)"
        case .unsupported(let msg):
            return "Unsupported operation: \(msg)"
        }
    }
}
#endif
