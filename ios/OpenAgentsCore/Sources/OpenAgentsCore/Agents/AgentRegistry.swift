#if os(macOS)
import Foundation

/// Central registry for agent providers.
/// Manages registration, discovery, and lookup of available agents.
/// Thread-safe via actor isolation.
public actor AgentRegistry {
    // MARK: - State

    /// Registered providers by mode ID
    private var providers: [ACPSessionModeId: AgentProvider] = [:]

    /// Active session handles
    private var activeHandles: [String: AgentHandle] = [:]

    // MARK: - Initialization

    public init() {}

    // MARK: - Registration

    /// Register an agent provider
    /// - Parameter provider: The provider to register
    public func register(_ provider: AgentProvider) {
        providers[provider.id] = provider
        OpenAgentsLog.orchestration.info("AgentRegistry Registered provider: \(provider.displayName) (mode=\(provider.id.rawValue))")
    }

    /// Unregister an agent provider
    /// - Parameter modeId: The mode ID to unregister
    public func unregister(_ modeId: ACPSessionModeId) {
        if let provider = providers.removeValue(forKey: modeId) {
            OpenAgentsLog.orchestration.info("AgentRegistry Unregistered provider: \(provider.displayName)")
        }
    }

    // MARK: - Lookup

    /// Get a provider for a specific mode
    /// - Parameter modeId: The mode ID to look up
    /// - Returns: The provider, or nil if not registered
    public func provider(for modeId: ACPSessionModeId) -> AgentProvider? {
        return providers[modeId]
    }

    /// Get all registered providers
    /// - Returns: Array of all registered providers
    public func allProviders() -> [AgentProvider] {
        return Array(providers.values)
    }

    /// Get all available providers (registered AND currently available)
    /// - Returns: Array of providers that are currently available
    public func availableProviders() async -> [AgentProvider] {
        var available: [AgentProvider] = []
        for provider in providers.values {
            if await provider.isAvailable() {
                available.append(provider)
            }
        }
        return available
    }

    /// Check if a specific provider is available
    /// - Parameter modeId: The mode ID to check
    /// - Returns: True if the provider is registered and available
    public func isAvailable(_ modeId: ACPSessionModeId) async -> Bool {
        guard let provider = providers[modeId] else { return false }
        return await provider.isAvailable()
    }

    // MARK: - Session Handle Management

    /// Store a handle for an active session
    /// - Parameters:
    ///   - sessionId: The session ID
    ///   - handle: The agent handle
    public func setHandle(_ handle: AgentHandle, for sessionId: ACPSessionId) {
        activeHandles[sessionId.value] = handle
    }

    /// Get the handle for an active session
    /// - Parameter sessionId: The session ID
    /// - Returns: The handle, or nil if no active session
    public func handle(for sessionId: ACPSessionId) -> AgentHandle? {
        return activeHandles[sessionId.value]
    }

    /// Remove the handle for a session
    /// - Parameter sessionId: The session ID
    public func removeHandle(for sessionId: ACPSessionId) {
        activeHandles.removeValue(forKey: sessionId.value)
    }

    /// Get all active session handles
    /// - Returns: Dictionary of session ID → handle
    public func allHandles() -> [String: AgentHandle] {
        return activeHandles
    }

    // MARK: - Convenience Methods

    /// Get the display names of all available providers
    /// - Returns: Array of display names
    public func availableDisplayNames() async -> [String] {
        let available = await availableProviders()
        return available.map { $0.displayName }
    }

    /// Get the mode ID for a provider by display name
    /// - Parameter displayName: The display name to search for
    /// - Returns: The mode ID, or nil if not found
    public func modeId(forDisplayName displayName: String) -> ACPSessionModeId? {
        for provider in providers.values {
            if provider.displayName.lowercased() == displayName.lowercased() {
                return provider.id
            }
        }
        return nil
    }

    /// Get capabilities for a specific mode
    /// - Parameter modeId: The mode ID
    /// - Returns: The capabilities, or nil if provider not found
    public func capabilities(for modeId: ACPSessionModeId) -> AgentCapabilities? {
        return providers[modeId]?.capabilities
    }
}
#endif
