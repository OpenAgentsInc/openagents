import Foundation
#if canImport(CryptoKit)
import CryptoKit
#endif

// MARK: - Orchestration Configuration

/// Configuration for overnight orchestration runs
///
/// Defines schedule, constraints, goals, agent preferences, and other settings
/// that control how the orchestration system makes decisions and executes tasks.
public struct OrchestrationConfig: Codable, Sendable, Equatable {
    // MARK: - Properties

    /// Unique identifier for this configuration (e.g., "default" or workspace-scoped ID)
    public var id: String

    /// Workspace root path (for workspace-specific configs)
    public var workspaceRoot: String

    /// Schedule configuration (cron expression, time window, jitter)
    public var schedule: Schedule

    /// Execution constraints (power, network)
    public var constraints: Constraints

    /// Time budget per orchestration cycle in seconds (e.g., 1800 = 30 minutes)
    public var timeBudgetSec: Int

    /// Maximum concurrent tasks (1-4)
    public var maxConcurrent: Int

    /// User-specified goals (e.g., ["refactor error handling", "increase test coverage"])
    public var goals: [String]

    /// Agent selection preferences
    public var agentPreferences: AgentPreferences

    /// File focus configuration (include/exclude globs)
    public var focus: Focus

    /// PR automation settings
    public var prAutomation: PRAutomation

    /// Timestamp when config was last updated (milliseconds since epoch)
    public var updatedAt: Int64

    // MARK: - Initialization

    public init(
        id: String = "default",
        workspaceRoot: String,
        schedule: Schedule = Schedule(),
        constraints: Constraints = Constraints(),
        timeBudgetSec: Int = 1800,
        maxConcurrent: Int = 2,
        goals: [String] = [],
        agentPreferences: AgentPreferences = AgentPreferences(),
        focus: Focus = Focus(),
        prAutomation: PRAutomation = PRAutomation(),
        updatedAt: Int64 = Int64(Date().timeIntervalSince1970 * 1000)
    ) {
        self.id = id
        self.workspaceRoot = workspaceRoot
        self.schedule = schedule
        self.constraints = constraints
        self.timeBudgetSec = timeBudgetSec
        self.maxConcurrent = maxConcurrent
        self.goals = goals
        self.agentPreferences = agentPreferences
        self.focus = focus
        self.prAutomation = prAutomation
        self.updatedAt = updatedAt
    }

    // MARK: - Helpers

    /// Compute a stable hash of goals for metadata tracking
    public func goalsHash() -> String {
        let goalsString = goals.sorted().joined(separator: "|")
        return computeHash(goalsString)
    }

    /// Validate the configuration and return errors if any
    public func validate() -> [String] {
        var errors: [String] = []

        // Validate time budget (15 min - 2 hours)
        if timeBudgetSec < 900 || timeBudgetSec > 7200 {
            errors.append("timeBudgetSec must be between 900 (15 min) and 7200 (2 hours)")
        }

        // Validate max concurrent (1-4)
        if maxConcurrent < 1 || maxConcurrent > 4 {
            errors.append("maxConcurrent must be between 1 and 4")
        }

        // Validate schedule
        errors.append(contentsOf: schedule.validate())

        // Validate agent preferences
        errors.append(contentsOf: agentPreferences.validate())

        // Validate focus globs (basic check - full validation happens server-side)
        errors.append(contentsOf: focus.validate())

        return errors
    }

    // MARK: - Default Config

    /// Create a default configuration for a workspace
    public static func createDefault(workspaceRoot: String) -> OrchestrationConfig {
        return OrchestrationConfig(
            id: "default",
            workspaceRoot: workspaceRoot,
            schedule: Schedule(
                type: "cron",
                expression: "*/30 1-5 * * *",  // Every 30 min between 1-5 AM
                windowStart: "01:00",
                windowEnd: "05:00",
                jitterMs: 300000,  // 5 minutes
                onMissed: "catch_up"
            ),
            constraints: Constraints(
                pluggedIn: true,
                wifiOnly: true
            ),
            timeBudgetSec: 1800,  // 30 minutes
            maxConcurrent: 2,
            goals: [],
            agentPreferences: AgentPreferences(),
            focus: Focus(),
            prAutomation: PRAutomation()
        )
    }

    // MARK: - Private Helpers

    private func computeHash(_ input: String) -> String {
        guard let data = input.data(using: .utf8) else {
            return UUID().uuidString.replacingOccurrences(of: "-", with: "").prefix(16).description
        }

        #if canImport(CryptoKit)
        let digest = SHA256.hash(data: data)
        let hex = digest.compactMap { String(format: "%02x", $0) }.joined()
        return String(hex.prefix(16))
        #else
        // Fallback: djb2 hash
        var hash: UInt64 = 5381
        for byte in data {
            hash = ((hash << 5) &+ hash) &+ UInt64(byte)
        }
        return String(format: "%016llx", hash)
        #endif
    }
}

// MARK: - Schedule

extension OrchestrationConfig {
    /// Schedule configuration for orchestration runs
    public struct Schedule: Codable, Sendable, Equatable {
        /// Schedule type (currently only "cron" supported)
        public var type: String

        /// Cron expression (5-field format: minute hour day month weekday)
        /// Example: "*/30 1-5 * * *" = every 30 minutes between 1 AM and 5 AM
        public var expression: String

        /// Time window start (HH:mm format, optional)
        public var windowStart: String?

        /// Time window end (HH:mm format, optional)
        public var windowEnd: String?

        /// Random jitter in milliseconds (adds randomness to schedule)
        public var jitterMs: Int?

        /// What to do when a scheduled run is missed ("skip" or "catch_up")
        public var onMissed: String?

        public init(
            type: String = "cron",
            expression: String = "*/30 1-5 * * *",
            windowStart: String? = nil,
            windowEnd: String? = nil,
            jitterMs: Int? = nil,
            onMissed: String? = nil
        ) {
            self.type = type
            self.expression = expression
            self.windowStart = windowStart
            self.windowEnd = windowEnd
            self.jitterMs = jitterMs
            self.onMissed = onMissed
        }

        /// Validate schedule configuration
        func validate() -> [String] {
            var errors: [String] = []

            // Validate type
            if type != "cron" {
                errors.append("schedule.type must be 'cron' (only supported type)")
            }

            // Validate cron expression (basic check - full parsing happens server-side)
            let parts = expression.split(separator: " ")
            if parts.count != 5 {
                errors.append("schedule.expression must have 5 fields (minute hour day month weekday)")
            }

            // Validate time window format if present
            if let start = windowStart {
                if !isValidTimeFormat(start) {
                    errors.append("schedule.windowStart must be in HH:mm format")
                }
            }

            if let end = windowEnd {
                if !isValidTimeFormat(end) {
                    errors.append("schedule.windowEnd must be in HH:mm format")
                }
            }

            // Validate window start < end if both present
            if let start = windowStart, let end = windowEnd {
                if start >= end {
                    errors.append("schedule.windowStart must be before windowEnd")
                }
            }

            // Validate jitter range
            if let jitter = jitterMs {
                if jitter < 0 || jitter > 3600000 {  // Max 1 hour jitter
                    errors.append("schedule.jitterMs must be between 0 and 3600000 (1 hour)")
                }
            }

            // Validate onMissed value
            if let onMissed = onMissed {
                if onMissed != "skip" && onMissed != "catch_up" {
                    errors.append("schedule.onMissed must be 'skip' or 'catch_up'")
                }
            }

            return errors
        }

        private func isValidTimeFormat(_ time: String) -> Bool {
            let pattern = "^([0-1][0-9]|2[0-3]):[0-5][0-9]$"
            return time.range(of: pattern, options: .regularExpression) != nil
        }
    }
}

// MARK: - Constraints

extension OrchestrationConfig {
    /// Execution constraints for orchestration runs
    public struct Constraints: Codable, Sendable, Equatable {
        /// Only run when device is plugged into power
        public var pluggedIn: Bool

        /// Only run when device is connected to Wi-Fi (not cellular)
        public var wifiOnly: Bool

        public init(
            pluggedIn: Bool = true,
            wifiOnly: Bool = true
        ) {
            self.pluggedIn = pluggedIn
            self.wifiOnly = wifiOnly
        }
    }
}

// MARK: - Agent Preferences

extension OrchestrationConfig {
    /// Agent selection preferences
    public struct AgentPreferences: Codable, Sendable, Equatable {
        /// Preferred agent (if available and allowed)
        /// If prefer is not in allow, it will be ignored
        public var prefer: ACPSessionModeId?

        /// Allowed agents (must be non-empty)
        /// If empty, defaults to all currently registered/available providers
        public var allow: [ACPSessionModeId]

        public init(
            prefer: ACPSessionModeId? = nil,
            allow: [ACPSessionModeId] = []
        ) {
            self.prefer = prefer
            self.allow = allow
        }

        /// Validate agent preferences
        func validate() -> [String] {
            var errors: [String] = []

            // If prefer is set and allow is non-empty, prefer must be in allow
            if let preferredAgent = prefer, !allow.isEmpty {
                if !allow.contains(preferredAgent) {
                    errors.append("agentPreferences.prefer must be in allow list if both are specified")
                }
            }

            return errors
        }

        /// Get effective prefer value (nil if not in allow list)
        public func effectivePrefer() -> ACPSessionModeId? {
            guard let preferred = prefer else { return nil }

            // If allow is empty, prefer is valid (will use available providers)
            if allow.isEmpty { return preferred }

            // If allow is non-empty, prefer must be in it
            return allow.contains(preferred) ? preferred : nil
        }

        /// Get effective allow list (defaults to provided available agents if empty)
        public func effectiveAllow(availableAgents: [ACPSessionModeId]) -> [ACPSessionModeId] {
            return allow.isEmpty ? availableAgents : allow
        }
    }
}

// MARK: - Focus

extension OrchestrationConfig {
    /// File focus configuration (include/exclude globs)
    public struct Focus: Codable, Sendable, Equatable {
        /// Include glob patterns (e.g., ["ios/**", "packages/*/src/**"])
        /// If empty, defaults to ["."] (entire workspace)
        public var include: [String]?

        /// Exclude glob patterns (e.g., ["**/node_modules/**", "**/.git/**"])
        public var exclude: [String]?

        public init(
            include: [String]? = nil,
            exclude: [String]? = nil
        ) {
            self.include = include
            self.exclude = exclude
        }

        /// Validate focus configuration
        func validate() -> [String] {
            var errors: [String] = []

            // Check for obviously invalid globs (path traversal attempts)
            if let includePatterns = include {
                for pattern in includePatterns {
                    if pattern.contains("..") || pattern.hasPrefix("/") {
                        errors.append("focus.include pattern '\(pattern)' contains path traversal or absolute path")
                    }
                }
            }

            if let excludePatterns = exclude {
                for pattern in excludePatterns {
                    if pattern.contains("..") || pattern.hasPrefix("/") {
                        errors.append("focus.exclude pattern '\(pattern)' contains path traversal or absolute path")
                    }
                }
            }

            return errors
        }

        /// Get effective include patterns (defaults to ["."] if empty)
        public func effectiveInclude() -> [String] {
            guard let patterns = include, !patterns.isEmpty else {
                return ["."]
            }
            return patterns
        }

        /// Get effective exclude patterns (defaults to empty if nil)
        public func effectiveExclude() -> [String] {
            return exclude ?? []
        }
    }
}

// MARK: - PR Automation

extension OrchestrationConfig {
    /// PR automation settings
    public struct PRAutomation: Codable, Sendable, Equatable {
        /// Whether PR automation is enabled
        public var enabled: Bool

        /// Whether to create PRs as drafts
        public var draft: Bool

        /// Branch name prefix for automated PRs (e.g., "agent/orchestration/")
        public var branchPrefix: String

        public init(
            enabled: Bool = false,
            draft: Bool = true,
            branchPrefix: String = "agent/orchestration/"
        ) {
            self.enabled = enabled
            self.draft = draft
            self.branchPrefix = branchPrefix
        }

        /// Validate PR automation settings
        func validate() -> [String] {
            var errors: [String] = []

            // Validate branch prefix (safe characters only)
            let allowedPattern = "^[a-zA-Z0-9/_-]+$"
            if branchPrefix.range(of: allowedPattern, options: .regularExpression) == nil {
                errors.append("prAutomation.branchPrefix must contain only alphanumeric, /, _, or - characters")
            }

            return errors
        }
    }
}
