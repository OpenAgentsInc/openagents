import os.log

/// Centralized logging facade for OpenAgents using os.Logger
///
/// # Usage
///
/// Use the appropriate category logger for your component:
/// ```swift
/// OpenAgentsLog.server.info("Connection established")
/// OpenAgentsLog.client.debug("Sending message: \(messageId)")
/// OpenAgentsLog.orchestration.error("Failed to execute tool: \(error)")
/// ```
///
/// # Log Levels
///
/// - **debug**: Verbose diagnostic information (DEBUG builds only in practice)
/// - **info**: General informational messages about normal operation
/// - **notice**: Important events that are part of normal operation (default level)
/// - **warning**: Unexpected but recoverable situations
/// - **error**: Errors that require attention
/// - **fault**: Critical errors that may cause system failure
///
/// # Privacy Annotations
///
/// Use privacy modifiers to protect sensitive data:
/// ```swift
/// OpenAgentsLog.server.info("User path: \(path, privacy: .private)")
/// OpenAgentsLog.client.debug("Session ID: \(sessionId, privacy: .public)")
/// ```
///
/// # Debug vs Release
///
/// Debug builds will show verbose logging. Release builds are configured to be
/// quieter, typically showing only notices, warnings, and errors in Console.app.
/// Use `isDebugLoggingEnabled` to conditionally log expensive operations:
///
/// ```swift
/// if OpenAgentsLog.isDebugLoggingEnabled {
///     let prettyJSON = expensivePrettyPrint(data)
///     OpenAgentsLog.server.debug("Received: \(prettyJSON)")
/// }
/// ```
///
public enum OpenAgentsLog {

    /// The subsystem identifier for all OpenAgents logging
    private static let subsystem = "com.openagents.core"

    // MARK: - Category Loggers

    /// App lifecycle, initialization, state management
    ///
    /// Use for: App startup, shutdown, state transitions, summarization, Foundation Models
    public static let app = Logger(subsystem: subsystem, category: "app")

    /// SwiftUI views, UI interactions, rendering
    ///
    /// Use for: View lifecycle, user interactions, UI state changes, rendering events
    public static let ui = Logger(subsystem: subsystem, category: "ui")

    /// Bridge protocol and coordination
    ///
    /// Use for: Bridge manager, iOS ↔ macOS communication coordination, pairing
    public static let bridge = Logger(subsystem: subsystem, category: "bridge")

    /// Desktop WebSocket server
    ///
    /// Use for: Server lifecycle, JSON-RPC routing, session updates, history API
    public static let server = Logger(subsystem: subsystem, category: "server")

    /// Mobile WebSocket client
    ///
    /// Use for: Client connection, reconnection, message sending/receiving
    public static let client = Logger(subsystem: subsystem, category: "client")

    /// Session management and agent orchestration
    ///
    /// Use for: Session lifecycle, workspace exploration, tool execution, agent providers
    public static let orchestration = Logger(subsystem: subsystem, category: "orchestration")

    // MARK: - Debug Flag

    /// True if debug logging is enabled (DEBUG builds only)
    ///
    /// Use this to gate expensive logging operations that should only run in debug builds:
    /// ```swift
    /// if OpenAgentsLog.isDebugLoggingEnabled {
    ///     // Expensive operation only in DEBUG
    ///     let detailedDump = generateDetailedDebugInfo()
    ///     OpenAgentsLog.server.debug("Details: \(detailedDump)")
    /// }
    /// ```
    #if DEBUG
    public static let isDebugLoggingEnabled = true
    #else
    public static let isDebugLoggingEnabled = false
    #endif
}
