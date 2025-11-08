import SwiftUI
import OpenAgentsCore

struct MorningBriefingDemoView: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    // Hero stats
                    MorningBriefingStatsView(summary: MockOrchestrationData.overnightRun)

                    // Agent performance
                    AgentPerformanceView(summary: MockOrchestrationData.overnightRun)

                    // Pull requests section
                    prListSection

                    // Decisions section
                    decisionsSection

                    // Issues/Alerts section
                    issuesSection
                }
                .padding(20)
            }
            .navigationTitle("Morning Briefing")
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") {
                        dismiss()
                    }
                }
            }
            .preferredColorScheme(.dark)
            .background(OATheme.Colors.background)
        }
    }

    @ViewBuilder
    private var prListSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Pull Requests")
                    .font(OAFonts.ui(.headline, 18))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Spacer()

                Text("\(MockOrchestrationData.prs.count)")
                    .font(OAFonts.ui(.caption, 14))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(
                        Capsule()
                            .fill(OATheme.Colors.border.opacity(0.3))
                    )
            }

            ForEach(MockOrchestrationData.prs.prefix(6)) { pr in
                PRCardView(pr: pr)
            }
        }
    }

    @ViewBuilder
    private var decisionsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Orchestration Decisions")
                    .font(OAFonts.ui(.headline, 18))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Spacer()

                Text("\(MockOrchestrationData.tasks.count)")
                    .font(OAFonts.ui(.caption, 14))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(
                        Capsule()
                            .fill(OATheme.Colors.border.opacity(0.3))
                    )
            }

            ForEach(MockOrchestrationData.tasks.prefix(4)) { task in
                DecisionCardView(task: task)
            }
        }
    }

    @ViewBuilder
    private var issuesSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Issues & Alerts")
                .font(OAFonts.ui(.headline, 18))
                .foregroundStyle(OATheme.Colors.textPrimary)

            // Failed task alert
            if let failedTask = MockOrchestrationData.tasks.first(where: { $0.status == .failed }) {
                failedTaskAlert(task: failedTask)
            }

            // Skipped task info
            if let skippedTask = MockOrchestrationData.tasks.first(where: { $0.status == .skipped }) {
                skippedTaskInfo(task: skippedTask)
            }
        }
    }

    @ViewBuilder
    private func failedTaskAlert(task: OrchestrationTask) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 16))
                    .foregroundStyle(Color.red)

                Text("1 Task Failed")
                    .font(OAFonts.ui(.body, 14))
                    .fontWeight(.medium)
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Task: \(task.decision.task)")
                    .font(OAFonts.ui(.body, 13))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                Text("Agent: \(agentName(task.decision.agent))")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                if let error = task.error {
                    Text("Error: \(error)")
                        .font(OAFonts.mono(.caption, 11))
                        .foregroundStyle(Color.red.opacity(0.9))
                        .padding(.top, 4)
                }
            }

            HStack(spacing: 8) {
                Button {
                    // Retry action (demo)
                } label: {
                    Text("Retry Tonight")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(
                            Capsule()
                                .fill(OATheme.Colors.accent)
                        )
                }
                .buttonStyle(.plain)

                Button {
                    // View logs action (demo)
                } label: {
                    Text("View Session Logs")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.accent)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(
                            Capsule()
                                .stroke(OATheme.Colors.accent, lineWidth: 1)
                        )
                }
                .buttonStyle(.plain)
            }
            .padding(.top, 4)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.red.opacity(0.1))
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(Color.red.opacity(0.3), lineWidth: 1)
                )
        )
    }

    @ViewBuilder
    private func skippedTaskInfo(task: OrchestrationTask) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "info.circle.fill")
                    .font(.system(size: 16))
                    .foregroundStyle(Color.orange)

                Text("1 Task Skipped")
                    .font(OAFonts.ui(.body, 14))
                    .fontWeight(.medium)
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }

            VStack(alignment: .leading, spacing: 6) {
                Text("Task: \(task.decision.task)")
                    .font(OAFonts.ui(.body, 13))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                if let reason = task.metadata["skip_reason"] {
                    Text("Reason: \(reason)")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.orange.opacity(0.1))
                .overlay(
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(Color.orange.opacity(0.3), lineWidth: 1)
                )
        )
    }

    private func agentName(_ agent: ACPSessionModeId) -> String {
        switch agent {
        case .claude_code: return "Claude Code"
        case .codex: return "Codex"
        case .default_mode: return "Default"
        }
    }
}
