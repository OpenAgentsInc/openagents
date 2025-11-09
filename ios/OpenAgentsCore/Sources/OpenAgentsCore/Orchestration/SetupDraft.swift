import Foundation

// MARK: - Setup Draft

/// Holds partial orchestration config state during conversational setup
///
/// SetupOrchestrator builds this incrementally as it gathers information from the user,
/// then converts it to a full OrchestrationConfig when ready to save.
public struct SetupDraft: Codable, Sendable, Equatable {
    // MARK: - Properties

    /// Unique identifier for this config (default: "default")
    public var id: String?

    /// Workspace root path
    public var workspaceRoot: String?

    /// Schedule patch (optional fields for partial updates)
    public var schedule: SchedulePatch?

    /// Constraints patch
    public var constraints: ConstraintsPatch?

    /// Time budget in seconds (900-7200)
    public var timeBudgetSec: Int?

    /// Maximum concurrent tasks (1-4)
    public var maxConcurrent: Int?

    /// User-specified goals
    public var goals: [String]?

    /// Agent preferences patch
    public var agentPreferences: AgentPreferencesPatch?

    /// Focus patch (inferred from user selections)
    public var focus: FocusPatch?

    /// PR automation patch
    public var prAutomation: PRAutomationPatch?

    /// Conversation ID for resumability
    public var conversationId: String?

    // MARK: - Initialization

    public init(
        id: String? = nil,
        workspaceRoot: String? = nil,
        schedule: SchedulePatch? = nil,
        constraints: ConstraintsPatch? = nil,
        timeBudgetSec: Int? = nil,
        maxConcurrent: Int? = nil,
        goals: [String]? = nil,
        agentPreferences: AgentPreferencesPatch? = nil,
        focus: FocusPatch? = nil,
        prAutomation: PRAutomationPatch? = nil,
        conversationId: String? = nil
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
        self.conversationId = conversationId
    }

    // MARK: - Conversion

    /// Convert draft to full OrchestrationConfig with defaults for missing fields
    public func toConfig() throws -> OrchestrationConfig {
        guard let workspaceRoot = workspaceRoot else {
            throw SetupDraftError.missingRequiredField("workspaceRoot")
        }

        return OrchestrationConfig(
            id: id ?? "default",
            workspaceRoot: workspaceRoot,
            schedule: schedule?.toSchedule() ?? OrchestrationConfig.Schedule(),
            constraints: constraints?.toConstraints() ?? OrchestrationConfig.Constraints(),
            timeBudgetSec: timeBudgetSec ?? 1800,
            maxConcurrent: maxConcurrent ?? 2,
            goals: goals ?? [],
            agentPreferences: agentPreferences?.toAgentPreferences() ?? OrchestrationConfig.AgentPreferences(),
            focus: focus?.toFocus() ?? OrchestrationConfig.Focus(),
            prAutomation: prAutomation?.toPRAutomation() ?? OrchestrationConfig.PRAutomation()
        )
    }
}

// MARK: - Patch Types

extension SetupDraft {
    /// Partial schedule update
    public struct SchedulePatch: Codable, Sendable, Equatable {
        public var type: String?
        public var expression: String?
        public var windowStart: String?
        public var windowEnd: String?
        public var jitterMs: Int?
        public var onMissed: String?

        public init(
            type: String? = nil,
            expression: String? = nil,
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

        func toSchedule() -> OrchestrationConfig.Schedule {
            return OrchestrationConfig.Schedule(
                type: type ?? "cron",
                expression: expression ?? "*/30 1-5 * * *",
                windowStart: windowStart,
                windowEnd: windowEnd,
                jitterMs: jitterMs,
                onMissed: onMissed
            )
        }
    }

    /// Partial constraints update
    public struct ConstraintsPatch: Codable, Sendable, Equatable {
        public var pluggedIn: Bool?
        public var wifiOnly: Bool?

        public init(pluggedIn: Bool? = nil, wifiOnly: Bool? = nil) {
            self.pluggedIn = pluggedIn
            self.wifiOnly = wifiOnly
        }

        func toConstraints() -> OrchestrationConfig.Constraints {
            return OrchestrationConfig.Constraints(
                pluggedIn: pluggedIn ?? true,
                wifiOnly: wifiOnly ?? true
            )
        }
    }

    /// Partial agent preferences update
    public struct AgentPreferencesPatch: Codable, Sendable, Equatable {
        public var prefer: ACPSessionModeId?
        public var allow: [ACPSessionModeId]?

        public init(prefer: ACPSessionModeId? = nil, allow: [ACPSessionModeId]? = nil) {
            self.prefer = prefer
            self.allow = allow
        }

        func toAgentPreferences() -> OrchestrationConfig.AgentPreferences {
            return OrchestrationConfig.AgentPreferences(
                prefer: prefer,
                allow: allow ?? []
            )
        }
    }

    /// Partial focus update (with inferred patterns)
    public struct FocusPatch: Codable, Sendable, Equatable {
        public var include: [String]?
        public var exclude: [String]?
        /// Human-readable summary of focus (for Morning Briefing)
        public var focusResolved: String?

        public init(
            include: [String]? = nil,
            exclude: [String]? = nil,
            focusResolved: String? = nil
        ) {
            self.include = include
            self.exclude = exclude
            self.focusResolved = focusResolved
        }

        func toFocus() -> OrchestrationConfig.Focus {
            return OrchestrationConfig.Focus(
                include: include,
                exclude: exclude
            )
        }
    }

    /// Partial PR automation update
    public struct PRAutomationPatch: Codable, Sendable, Equatable {
        public var enabled: Bool?
        public var draft: Bool?
        public var branchPrefix: String?

        public init(
            enabled: Bool? = nil,
            draft: Bool? = nil,
            branchPrefix: String? = nil
        ) {
            self.enabled = enabled
            self.draft = draft
            self.branchPrefix = branchPrefix
        }

        func toPRAutomation() -> OrchestrationConfig.PRAutomation {
            return OrchestrationConfig.PRAutomation(
                enabled: enabled ?? false,
                draft: draft ?? true,
                branchPrefix: branchPrefix ?? "agent/orchestration/"
            )
        }
    }
}

// MARK: - Errors

public enum SetupDraftError: Error, LocalizedError {
    case missingRequiredField(String)

    public var errorDescription: String? {
        switch self {
        case .missingRequiredField(let field):
            return "Missing required field: \(field)"
        }
    }
}
