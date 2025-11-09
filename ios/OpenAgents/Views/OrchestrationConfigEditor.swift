import SwiftUI
import OpenAgentsCore

#if os(macOS)

/// Config editor for creating/editing orchestration configs
struct OrchestrationConfigEditor: View {
    let config: OrchestrationConfig?
    let onSave: (OrchestrationConfig) async -> Void

    @Environment(\.dismiss) private var dismiss

    // Config fields
    @State private var configId: String
    @State private var workspaceRoot: String
    @State private var goals: String
    @State private var cronExpression: String
    @State private var windowStart: String
    @State private var windowEnd: String
    @State private var timeBudgetMinutes: Int
    @State private var maxConcurrent: Int

    // Agent preferences
    @State private var preferredAgent: ACPSessionModeId?
    @State private var allowedAgents: Set<ACPSessionModeId>

    // Validation
    @State private var validationErrors: [String] = []
    @State private var showingValidationErrors = false

    init(config: OrchestrationConfig?, onSave: @escaping (OrchestrationConfig) async -> Void) {
        self.config = config
        self.onSave = onSave

        if let config = config {
            _configId = State(initialValue: config.id)
            _workspaceRoot = State(initialValue: config.workspaceRoot)
            _goals = State(initialValue: config.goals.joined(separator: "\n"))
            _cronExpression = State(initialValue: config.schedule.expression)
            _windowStart = State(initialValue: config.schedule.windowStart ?? "")
            _windowEnd = State(initialValue: config.schedule.windowEnd ?? "")
            _timeBudgetMinutes = State(initialValue: config.timeBudgetSec / 60)
            _maxConcurrent = State(initialValue: config.maxConcurrent)
            _preferredAgent = State(initialValue: config.agentPreferences.prefer)
            _allowedAgents = State(initialValue: Set(config.agentPreferences.allow))
        } else {
            _configId = State(initialValue: "default")
            _workspaceRoot = State(initialValue: "")
            _goals = State(initialValue: "")
            _cronExpression = State(initialValue: "0 2 * * *")
            _windowStart = State(initialValue: "02:00")
            _windowEnd = State(initialValue: "06:00")
            _timeBudgetMinutes = State(initialValue: 30)
            _maxConcurrent = State(initialValue: 1)
            _preferredAgent = State(initialValue: nil)
            _allowedAgents = State(initialValue: [.claude_code, .codex])
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            header

            Divider()

            // Form
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    basicInfoSection
                    scheduleSection
                    resourcesSection
                    goalsSection
                    agentPreferencesSection

                    if !validationErrors.isEmpty {
                        validationErrorsView
                    }
                }
                .padding(24)
            }

            Divider()

            // Footer
            footer
        }
        .frame(width: 600, height: 700)
        .background(OATheme.Colors.background)
    }

    // MARK: - Header

    @ViewBuilder
    private var header: some View {
        HStack {
            Text(config == nil ? "New Orchestration Config" : "Edit Config")
                .font(OAFonts.ui(.title2, 18))
                .fontWeight(.semibold)
                .foregroundStyle(OATheme.Colors.textPrimary)

            Spacer()

            Button {
                dismiss()
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 20))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            .buttonStyle(.plain)
        }
        .padding(20)
    }

    // MARK: - Sections

    @ViewBuilder
    private var basicInfoSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Basic Info")
                .font(OAFonts.ui(.headline, 14))
                .foregroundStyle(OATheme.Colors.textPrimary)

            VStack(alignment: .leading, spacing: 8) {
                Text("Config ID")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                TextField("e.g., default, nightly, weekend", text: $configId)
                    .textFieldStyle(.roundedBorder)
                    .disabled(config != nil)
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Workspace Root")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                HStack {
                    TextField("/path/to/workspace", text: $workspaceRoot)
                        .textFieldStyle(.roundedBorder)

                    Button {
                        selectWorkspaceDirectory()
                    } label: {
                        Text("Browse...")
                            .font(OAFonts.ui(.caption, 12))
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var scheduleSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Schedule")
                .font(OAFonts.ui(.headline, 14))
                .foregroundStyle(OATheme.Colors.textPrimary)

            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Cron Expression")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    Spacer()

                    Text(cronPreview())
                        .font(OAFonts.ui(.caption, 11))
                        .foregroundStyle(OATheme.Colors.accent)
                }

                TextField("0 2 * * *", text: $cronExpression)
                    .textFieldStyle(.roundedBorder)
                    .font(OAFonts.mono(.body, 13))
            }

            HStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Window Start")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    TextField("HH:mm", text: $windowStart)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Window End")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    TextField("HH:mm", text: $windowEnd)
                        .textFieldStyle(.roundedBorder)
                }
            }
        }
    }

    @ViewBuilder
    private var resourcesSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Resources")
                .font(OAFonts.ui(.headline, 14))
                .foregroundStyle(OATheme.Colors.textPrimary)

            HStack(spacing: 16) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Time Budget (minutes)")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    Stepper("\(timeBudgetMinutes) min", value: $timeBudgetMinutes, in: 15...120, step: 15)
                        .font(OAFonts.ui(.body, 14))
                }

                VStack(alignment: .leading, spacing: 8) {
                    Text("Max Concurrent Tasks")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    Stepper("\(maxConcurrent)", value: $maxConcurrent, in: 1...5)
                        .font(OAFonts.ui(.body, 14))
                }
            }
        }
    }

    @ViewBuilder
    private var goalsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Goals (one per line)")
                .font(OAFonts.ui(.headline, 14))
                .foregroundStyle(OATheme.Colors.textPrimary)

            TextEditor(text: $goals)
                .font(OAFonts.ui(.body, 13))
                .frame(height: 120)
                .scrollContentBackground(.hidden)
                .padding(8)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(OATheme.Colors.card)
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .stroke(OATheme.Colors.border, lineWidth: 1)
                        )
                )

            Text("Examples: \"refactor error handling\", \"increase test coverage\", \"add documentation\"")
                .font(OAFonts.ui(.caption, 11))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
    }

    @ViewBuilder
    private var agentPreferencesSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Agent Preferences")
                .font(OAFonts.ui(.headline, 14))
                .foregroundStyle(OATheme.Colors.textPrimary)

            VStack(alignment: .leading, spacing: 8) {
                Text("Allowed Agents")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                VStack(alignment: .leading, spacing: 6) {
                    Toggle("Claude Code", isOn: Binding(
                        get: { allowedAgents.contains(.claude_code) },
                        set: { if $0 { allowedAgents.insert(.claude_code) } else { allowedAgents.remove(.claude_code) } }
                    ))
                    .font(OAFonts.ui(.body, 14))

                    Toggle("Codex", isOn: Binding(
                        get: { allowedAgents.contains(.codex) },
                        set: { if $0 { allowedAgents.insert(.codex) } else { allowedAgents.remove(.codex) } }
                    ))
                    .font(OAFonts.ui(.body, 14))
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Preferred Agent (optional)")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                Picker("", selection: $preferredAgent) {
                    Text("None").tag(nil as ACPSessionModeId?)
                    Text("Claude Code").tag(ACPSessionModeId.claude_code as ACPSessionModeId?)
                    Text("Codex").tag(ACPSessionModeId.codex as ACPSessionModeId?)
                }
                .pickerStyle(.menu)
                .font(OAFonts.ui(.body, 14))
            }
        }
    }

    @ViewBuilder
    private var validationErrorsView: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundStyle(Color.red)

                Text("Validation Errors")
                    .font(OAFonts.ui(.headline, 14))
                    .foregroundStyle(Color.red)
            }

            ForEach(validationErrors, id: \.self) { error in
                Text("• \(error)")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(Color.red.opacity(0.9))
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.red.opacity(0.1))
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(Color.red.opacity(0.3), lineWidth: 1)
                )
        )
    }

    // MARK: - Footer

    @ViewBuilder
    private var footer: some View {
        HStack {
            Button {
                dismiss()
            } label: {
                Text("Cancel")
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 8)
                    .background(
                        Capsule()
                            .stroke(OATheme.Colors.border, lineWidth: 1)
                    )
            }
            .buttonStyle(.plain)

            Spacer()

            Button {
                Task {
                    await saveConfig()
                }
            } label: {
                Text("Save")
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 8)
                    .background(
                        Capsule()
                            .fill(validationErrors.isEmpty ? OATheme.Colors.accent : Color.gray)
                    )
            }
            .buttonStyle(.plain)
            .disabled(!validationErrors.isEmpty)
        }
        .padding(20)
    }

    // MARK: - Helpers

    private func cronPreview() -> String {
        // Simple cron preview (could be enhanced)
        let parts = cronExpression.split(separator: " ")
        guard parts.count == 5 else { return "Invalid cron" }

        let hour = parts[1]
        let minute = parts[0]

        if parts[2] == "*" && parts[3] == "*" && parts[4] == "*" {
            return "Daily at \(hour):\(minute)"
        } else if parts[2] == "*" && parts[3] == "*" {
            return "Every \(parts[4]) at \(hour):\(minute)"
        }

        return "Custom schedule"
    }

    private func selectWorkspaceDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false

        if panel.runModal() == .OK, let url = panel.url {
            workspaceRoot = url.path
        }
    }

    private func saveConfig() async {
        // Build config
        let goalsArray = goals
            .split(separator: "\n")
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        let newConfig = OrchestrationConfig(
            id: configId,
            workspaceRoot: workspaceRoot,
            schedule: OrchestrationConfig.Schedule(
                type: "cron",
                expression: cronExpression,
                windowStart: windowStart.isEmpty ? nil : windowStart,
                windowEnd: windowEnd.isEmpty ? nil : windowEnd,
                jitterMs: 0,
                onMissed: "skip"
            ),
            constraints: OrchestrationConfig.Constraints(
                pluggedIn: false,
                wifiOnly: false
            ),
            timeBudgetSec: timeBudgetMinutes * 60,
            maxConcurrent: maxConcurrent,
            goals: goalsArray,
            agentPreferences: OrchestrationConfig.AgentPreferences(
                prefer: preferredAgent,
                allow: Array(allowedAgents)
            ),
            focus: OrchestrationConfig.Focus(
                include: [],
                exclude: []
            ),
            prAutomation: OrchestrationConfig.PRAutomation(
                enabled: false,
                draft: true,
                branchPrefix: "agent/overnight/"
            ),
            updatedAt: Int64(Date().timeIntervalSince1970 * 1000)
        )

        // Validate
        validationErrors = newConfig.validate()

        if validationErrors.isEmpty {
            await onSave(newConfig)
        } else {
            showingValidationErrors = true
        }
    }
}

#endif
