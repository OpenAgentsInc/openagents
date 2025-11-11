// SetupOrchestrator.swift — Conversational orchestration config setup
// Guides users through config creation with natural language Q&A

import Foundation

#if os(macOS)

// MARK: - Setup State

/// Current state of conversational setup
public enum SetupState: String, Codable, Sendable {
    case gathering_workspace
    case gathering_goals
    case gathering_schedule
    case gathering_agents
    case gathering_constraints
    case review
    case saving
    case completed
    case failed
}

// MARK: - Setup Orchestrator

/// Actor managing conversational orchestration config setup
///
/// Guides user through config creation via natural language Q&A.
/// Uses SessionUpdateHub for streaming updates and persistence.
/// Falls back to deterministic Q&A if Foundation Models unavailable.
public actor SetupOrchestrator {
    // MARK: - Properties

    /// Conversation ID for resumability
    public let conversationId: String

    /// Session ID for ACP updates
    public let sessionId: ACPSessionId

    /// Current setup state
    private(set) var state: SetupState

    /// Draft config being built
    private var draft: SetupDraft

    /// Session update hub for streaming
    private let updateHub: SessionUpdateHub

    /// Completion handler (called when config saved)
    private let completionHandler: @Sendable (Result<OrchestrationConfig, Error>) async -> Void

    /// Conversation history (for context)
    private var conversationHistory: [(role: String, content: String)] = []

    // MARK: - Initialization

    public init(
        conversationId: String = UUID().uuidString,
        sessionId: ACPSessionId,
        initialWorkspace: String? = nil,
        updateHub: SessionUpdateHub,
        completionHandler: @escaping @Sendable (Result<OrchestrationConfig, Error>) async -> Void
    ) {
        self.conversationId = conversationId
        self.sessionId = sessionId
        self.state = initialWorkspace == nil ? .gathering_workspace : .gathering_goals
        self.draft = SetupDraft(
            workspaceRoot: initialWorkspace,
            conversationId: conversationId
        )
        self.updateHub = updateHub
        self.completionHandler = completionHandler
    }

    // Note: All interpretation of inline natural language should be performed
    // by Foundation Models during the conversational flow, not via
    // deterministic heuristics. See AGENTS.md LLM‑First Policy.

    // MARK: - Public API

    /// Start conversational setup
    public func start() async {
        await sendMessage("Hi! I'll help you set up overnight orchestration. This will configure agents to work on your codebase while you sleep.")

        await askNextQuestion()
    }

    /// Apply structured hints extracted by an FM tool to skip already‑answered
    /// steps. This is not heuristic parsing; callers must supply typed values.
    public func applyHints(
        goals: [String]?,
        windowStart: String?,
        windowEnd: String?,
        intervalMinutes: Int?,
        prefer: ACPSessionModeId?,
        allow: [ACPSessionModeId]?
    ) async {
        if let g = goals, !g.isEmpty {
            draft.goals = g
            if draft.workspaceRoot != nil { state = .gathering_schedule }
        }
        if let ws = windowStart, let we = windowEnd {
            let interval = (intervalMinutes ?? 30)
            let cronExpr = SchedulePreview.deriveCron(windowStart: ws, windowEnd: we, interval: max(1, min(interval, 60)))
            draft.schedule = SetupDraft.SchedulePatch(
                type: "cron",
                expression: cronExpr,
                windowStart: ws,
                windowEnd: we,
                jitterMs: 300000,
                onMissed: "catch_up"
            )
            if draft.workspaceRoot != nil { state = .gathering_agents }
        }
        if prefer != nil || (allow?.isEmpty == false) {
            draft.agentPreferences = SetupDraft.AgentPreferencesPatch(prefer: prefer, allow: allow ?? [])
            if draft.workspaceRoot != nil { state = .gathering_constraints }
        }
    }

    /// Handle user response
    public func handleUserResponse(_ response: String) async {
        // Record in conversation history
        conversationHistory.append((role: "user", content: response))

        // Process based on current state
        switch state {
        case .gathering_workspace:
            await handleWorkspaceResponse(response)
        case .gathering_goals:
            await handleGoalsResponse(response)
        case .gathering_schedule:
            await handleScheduleResponse(response)
        case .gathering_agents:
            await handleAgentsResponse(response)
        case .gathering_constraints:
            await handleConstraintsResponse(response)
        case .review:
            await handleReviewResponse(response)
        case .saving, .completed, .failed:
            await sendMessage("Setup is already \(state.rawValue). Please start a new setup session.")
        }
    }

    /// Get current draft (for preview)
    public func getCurrentDraft() -> SetupDraft {
        return draft
    }

    /// Get current state
    public func getCurrentState() -> SetupState {
        return state
    }

    /// Abort setup
    public func abort() async {
        state = .failed
        await sendMessage("Setup cancelled.")
        await completionHandler(.failure(SetupError.userCancelled))
    }

    // MARK: - State Handlers

    private func handleWorkspaceResponse(_ response: String) async {
        // Extract workspace path from response
        let workspace = response.trimmingCharacters(in: .whitespacesAndNewlines)

        // Basic validation
        let fm = FileManager.default
        var isDirectory: ObjCBool = false
        guard fm.fileExists(atPath: workspace, isDirectory: &isDirectory), isDirectory.boolValue else {
            await sendMessage("❌ '\(workspace)' is not a valid directory. Please provide a valid workspace path.")
            return
        }

        draft.workspaceRoot = workspace
        state = .gathering_goals

        await sendMessage("✅ Workspace: \(workspace)")
        await askNextQuestion()
    }

    // MARK: - Finalize without Q&A

    /// Complete the setup immediately by applying safe product defaults for any
    /// missing fields and saving the configuration without further questions.
    /// This does not perform heuristic interpretation; it only fills defaults.
    public func finalizeImmediately() async {
        // Ensure workspace is present
        guard let workspace = draft.workspaceRoot, !workspace.isEmpty else {
            await sendMessage("❌ Cannot finalize: workspace path is required.")
            state = .failed
            await completionHandler(.failure(SetupError.validationFailed(["missing workspace"])))
            return
        }

        // Default schedule window if missing (1:00 → 5:00, */30)
        if draft.schedule == nil {
            draft.schedule = SetupDraft.SchedulePatch(
                type: "cron",
                expression: "*/30 1-5 * * *",
                windowStart: "01:00",
                windowEnd: "05:00",
                jitterMs: 300000,
                onMissed: "catch_up"
            )
        }

        // Default constraints if missing (plugged in + Wi‑Fi)
        if draft.constraints == nil {
            draft.constraints = SetupDraft.ConstraintsPatch(pluggedIn: true, wifiOnly: true)
        }

        // Default PR + focus + meta if missing
        if draft.prAutomation == nil { draft.prAutomation = SetupDraft.PRAutomationPatch(enabled: false, draft: true) }
        if draft.focus == nil { draft.focus = SetupDraft.FocusPatch(include: ["."], exclude: nil) }
        if draft.id == nil { draft.id = "default" }
        if draft.timeBudgetSec == nil { draft.timeBudgetSec = 1800 }
        if draft.maxConcurrent == nil { draft.maxConcurrent = 2 }

        // Save
        await saveConfig()
    }

    private func handleGoalsResponse(_ response: String) async {
        // Parse goals from response (comma or newline separated)
        let goalsText = response.trimmingCharacters(in: .whitespacesAndNewlines)

        if goalsText.lowercased() == "skip" || goalsText.isEmpty {
            draft.goals = []
            state = .gathering_schedule
            await sendMessage("Skipping goals. Agents will use default decision logic.")
        } else {
            // Split by commas or newlines
            var goals = goalsText.components(separatedBy: ",")
            if goals.count == 1 {
                goals = goalsText.components(separatedBy: "\n")
            }

            draft.goals = goals.map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                               .filter { !$0.isEmpty }

            state = .gathering_schedule
            await sendMessage("✅ Goals: \(draft.goals!.joined(separator: ", "))")
        }

        await askNextQuestion()
    }

    private func handleScheduleResponse(_ response: String) async {
        // Parse natural language time window
        let text = response.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)

        // Simple patterns: "1 am to 5 am", "23:00 to 5:00", "overnight", etc.
        if let (start, end) = parseTimeWindow(text) {
            // Derive cron from time window
            let cronExpr = SchedulePreview.deriveCron(windowStart: start, windowEnd: end, interval: 30)

            draft.schedule = SetupDraft.SchedulePatch(
                type: "cron",
                expression: cronExpr,
                windowStart: start,
                windowEnd: end,
                jitterMs: 300000, // 5 min jitter
                onMissed: "catch_up"
            )

            state = .gathering_agents

            let humanReadable = SchedulePreview.humanReadable(
                schedule: OrchestrationConfig.Schedule(
                    type: "cron",
                    expression: cronExpr,
                    windowStart: start,
                    windowEnd: end
                )
            )
            await sendMessage("✅ Schedule: \(humanReadable)")
            await askNextQuestion()
        } else {
            await sendMessage("I couldn't parse that time window. Please try again (e.g., '1 AM to 5 AM' or '23:00 to 05:00').")
        }
    }

    private func handleAgentsResponse(_ response: String) async {
        let text = response.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)

        var prefer: ACPSessionModeId?
        var allow: [ACPSessionModeId] = []

        // Parse agent preferences
        if text.contains("claude code") || text.contains("claude-code") || text.contains("claude") {
            prefer = .claude_code
            allow.append(.claude_code)
        }
        if text.contains("codex") {
            if prefer == nil {
                prefer = .codex
            }
            allow.append(.codex)
        }
        if text.contains("both") || text.contains("either") {
            allow = [.claude_code, .codex]
            if prefer == nil {
                prefer = .claude_code // Default to Claude Code
            }
        }

        // Default if nothing specified
        if allow.isEmpty {
            allow = [.claude_code, .codex]
            prefer = .claude_code
        }

        draft.agentPreferences = SetupDraft.AgentPreferencesPatch(
            prefer: prefer,
            allow: allow
        )

        state = .gathering_constraints

        let preferStr = prefer.map { agentName($0) } ?? "None"
        let allowStr = allow.map { agentName($0) }.joined(separator: ", ")
        await sendMessage("✅ Preferred agent: \(preferStr) | Allowed: \(allowStr)")
        await askNextQuestion()
    }

    private func handleConstraintsResponse(_ response: String) async {
        let text = response.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)

        let pluggedIn: Bool
        let wifiOnly: Bool

        if text.contains("yes") || text.contains("only when plugged") {
            pluggedIn = true
            wifiOnly = true
        } else if text.contains("no") || text.contains("always") || text.contains("any time") {
            pluggedIn = false
            wifiOnly = false
        } else {
            // Default: safe constraints
            pluggedIn = true
            wifiOnly = true
        }

        draft.constraints = SetupDraft.ConstraintsPatch(
            pluggedIn: pluggedIn,
            wifiOnly: wifiOnly
        )

        // Set defaults for other fields
        draft.id = "default"
        draft.timeBudgetSec = 1800 // 30 minutes
        draft.maxConcurrent = 2
        draft.prAutomation = SetupDraft.PRAutomationPatch(enabled: false, draft: true)
        draft.focus = SetupDraft.FocusPatch(include: ["."], exclude: nil)

        state = .review

        await sendMessage("✅ Constraints: Plugged in: \(pluggedIn), Wi-Fi only: \(wifiOnly)")
        await showReview()
    }

    private func handleReviewResponse(_ response: String) async {
        let text = response.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)

        if text.contains("yes") || text.contains("save") || text.contains("confirm") || text.contains("looks good") {
            await saveConfig()
        } else if text.contains("no") || text.contains("cancel") || text.contains("start over") {
            await sendMessage("Setup cancelled. You can start a new setup session anytime.")
            state = .failed
            await completionHandler(.failure(SetupError.userCancelled))
        } else {
            await sendMessage("Please respond with 'yes' to save or 'no' to cancel.")
        }
    }

    // MARK: - Question Flow

    private func askNextQuestion() async {
        switch state {
        case .gathering_workspace:
            await sendMessage("What workspace should I configure? (provide full path, e.g., /Users/you/code/myproject)")
        case .gathering_goals:
            await sendMessage("What would you like agents to work on overnight? (e.g., 'refactor error handling, increase test coverage', or 'skip' for none)")
        case .gathering_schedule:
            await sendMessage("When should orchestration run? (e.g., '1 AM to 5 AM', '23:00 to 05:00', or 'overnight')")
        case .gathering_agents:
            await sendMessage("Which agent do you prefer?\n• Claude Code (architecture, refactoring, planning)\n• Codex (tests, boilerplate, implementation)\n• Both")
        case .gathering_constraints:
            await sendMessage("Should orchestration only run when plugged in and on Wi-Fi? (recommended: 'yes')")
        case .review:
            await showReview()
        case .saving, .completed, .failed:
            // No more questions
            break
        }
    }

    // MARK: - Review and Save

    private func showReview() async {
        guard let workspace = draft.workspaceRoot,
              let schedule = draft.schedule,
              let goals = draft.goals,
              let agentPrefs = draft.agentPreferences,
              let constraints = draft.constraints else {
            await sendMessage("❌ Incomplete configuration. Please start over.")
            state = .failed
            return
        }

        var message = "📋 **Review Your Configuration**\n\n"
        message += "**Workspace:** \(workspace)\n"
        message += "**Goals:** \(goals.isEmpty ? "(none)" : goals.joined(separator: ", "))\n"

        if let start = schedule.windowStart, let end = schedule.windowEnd {
            let humanReadable = SchedulePreview.humanReadable(
                schedule: OrchestrationConfig.Schedule(
                    type: "cron",
                    expression: schedule.expression ?? "*/30 1-5 * * *",
                    windowStart: start,
                    windowEnd: end
                )
            )
            message += "**Schedule:** \(humanReadable)\n"

            // Preview next 5 runs
            let nextRuns = SchedulePreview.nextRuns(
                schedule: OrchestrationConfig.Schedule(
                    type: "cron",
                    expression: schedule.expression ?? "*/30 1-5 * * *",
                    windowStart: start,
                    windowEnd: end
                ),
                count: 3
            )
            if !nextRuns.isEmpty {
                let formatter = DateFormatter()
                formatter.dateFormat = "h:mm a"
                let runsStr = nextRuns.prefix(3).map { formatter.string(from: $0) }.joined(separator: ", ")
                message += "**Next runs:** \(runsStr)\n"
            }
        }

        if let prefer = agentPrefs.prefer {
            message += "**Preferred agent:** \(agentName(prefer))\n"
        }
        if let allow = agentPrefs.allow, !allow.isEmpty {
            message += "**Allowed agents:** \(allow.map { agentName($0) }.joined(separator: ", "))\n"
        }

        message += "**Constraints:** Plugged in: \(constraints.pluggedIn ?? true), Wi-Fi: \(constraints.wifiOnly ?? true)\n"
        message += "**Time budget:** 30 minutes per run\n"
        message += "**Max concurrent:** 2 tasks\n\n"
        message += "Does this look good? (yes/no)"

        await sendMessage(message)
    }

    private func saveConfig() async {
        state = .saving
        await sendMessage("💾 Saving configuration...")

        do {
            let config = try draft.toConfig()

            // Validate
            let errors = config.validate()
            if !errors.isEmpty {
                await sendMessage("❌ Validation errors:\n" + errors.joined(separator: "\n"))
                state = .failed
                await completionHandler(.failure(SetupError.validationFailed(errors)))
                return
            }

            state = .completed
            await sendMessage("✅ Configuration saved! Orchestration is now active.\n**Config ID:** \(config.id)")
            await completionHandler(.success(config))
        } catch {
            state = .failed
            await sendMessage("❌ Failed to save: \(error.localizedDescription)")
            await completionHandler(.failure(error))
        }
    }

    // MARK: - Helpers

    private func sendMessage(_ text: String) async {
        conversationHistory.append((role: "assistant", content: text))

        let contentBlock = ACP.Client.ContentBlock.text(
            ACP.Client.ContentBlock.TextContent(text: text + "\n\n")
        )

        let contentChunk = ACP.Client.ContentChunk(
            content: contentBlock,
            _meta: [
                "source": AnyEncodable("conversational_setup"),
                "conversation_id": AnyEncodable(conversationId),
                "state": AnyEncodable(state.rawValue)
            ]
        )

        let update = ACP.Client.SessionUpdate.agentMessageChunk(contentChunk)

        await updateHub.sendSessionUpdate(sessionId: sessionId, update: update)
    }

    private func parseTimeWindow(_ text: String) -> (start: String, end: String)? {
        // Simple patterns:
        // "1 am to 5 am" -> "01:00", "05:00"
        // "23:00 to 05:00" -> "23:00", "05:00"
        // "overnight" -> "01:00", "05:00"

        if text.contains("overnight") {
            return ("01:00", "05:00")
        }

        // Try to extract two times
        let pattern = #"(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\s*(?:to|-)\s*(\d{1,2})(?::(\d{2}))?\s*(am|pm)?"#
        if let regex = try? NSRegularExpression(pattern: pattern, options: .caseInsensitive),
           let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) {

            let startHourStr = (text as NSString).substring(with: match.range(at: 1))
            let startMinStr = match.range(at: 2).location != NSNotFound ? (text as NSString).substring(with: match.range(at: 2)) : "00"
            let startPeriod = match.range(at: 3).location != NSNotFound ? (text as NSString).substring(with: match.range(at: 3)) : ""

            let endHourStr = (text as NSString).substring(with: match.range(at: 4))
            let endMinStr = match.range(at: 5).location != NSNotFound ? (text as NSString).substring(with: match.range(at: 5)) : "00"
            let endPeriod = match.range(at: 6).location != NSNotFound ? (text as NSString).substring(with: match.range(at: 6)) : ""

            if let startHour = Int(startHourStr), let endHour = Int(endHourStr) {
                // Convert to 24-hour format
                let start24 = convert24Hour(hour: startHour, period: startPeriod)
                let end24 = convert24Hour(hour: endHour, period: endPeriod)

                return (
                    String(format: "%02d:%@", start24, startMinStr),
                    String(format: "%02d:%@", end24, endMinStr)
                )
            }
        }

        return nil
    }

    private func convert24Hour(hour: Int, period: String) -> Int {
        if period.lowercased() == "pm" && hour != 12 {
            return hour + 12
        } else if period.lowercased() == "am" && hour == 12 {
            return 0
        }
        return hour
    }

    private func agentName(_ mode: ACPSessionModeId) -> String {
        switch mode {
        case .claude_code: return "Claude Code"
        case .codex: return "Codex"
        case .default_mode: return "Default"
        case .orchestrator: return "Orchestrator"
        }
    }
}

// MARK: - Errors

public enum SetupError: Error, LocalizedError {
    case userCancelled
    case validationFailed([String])
    case saveFailed(String)

    public var errorDescription: String? {
        switch self {
        case .userCancelled:
            return "Setup cancelled by user"
        case .validationFailed(let errors):
            return "Validation failed: \(errors.joined(separator: ", "))"
        case .saveFailed(let reason):
            return "Save failed: \(reason)"
        }
    }
}

#endif
