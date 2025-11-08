import SwiftUI
import OpenAgentsCore

struct AgentPerformanceView: View {
    let summary: OvernightRunSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            Text("Agent Performance")
                .font(OAFonts.ui(.headline, 18))
                .foregroundStyle(OATheme.Colors.textPrimary)

            // Agent breakdown
            VStack(alignment: .leading, spacing: 12) {
                if let claudeStats = summary.agentBreakdown["claude_code"] {
                    agentBar(
                        name: "Claude Code",
                        stats: claudeStats,
                        color: Color.purple,
                        percentage: agentPercentage(for: "claude_code")
                    )
                }

                if let codexStats = summary.agentBreakdown["codex"] {
                    agentBar(
                        name: "Codex",
                        stats: codexStats,
                        color: Color.teal,
                        percentage: agentPercentage(for: "codex")
                    )
                }
            }

            // Overall stats
            VStack(alignment: .leading, spacing: 8) {
                statsRow(
                    label: "Files Changed:",
                    value: "\(summary.filesChanged) files"
                )

                statsRow(
                    label: "Lines Modified:",
                    value: "+\(summary.linesAdded) / -\(summary.linesRemoved) lines"
                )

                statsRow(
                    label: "Tool Calls:",
                    value: "\(summary.toolCallsTotal) total (\(toolCallBreakdown))"
                )
            }
            .font(OAFonts.ui(.body, 14))
            .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .padding(20)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(OATheme.Colors.border.opacity(0.3))
        )
    }

    @ViewBuilder
    private func agentBar(name: String, stats: AgentStats, color: Color, percentage: Double) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(name)
                    .font(OAFonts.ui(.body, 14))
                    .fontWeight(.medium)
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Spacer()

                Text("\(Int(percentage))%")
                    .font(OAFonts.mono(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }

            // Progress bar
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    // Background
                    RoundedRectangle(cornerRadius: 4, style: .continuous)
                        .fill(OATheme.Colors.border.opacity(0.3))
                        .frame(height: 8)

                    // Filled portion
                    RoundedRectangle(cornerRadius: 4, style: .continuous)
                        .fill(color)
                        .frame(width: geometry.size.width * percentage / 100, height: 8)
                }
            }
            .frame(height: 8)

            // Stats
            HStack(spacing: 16) {
                Text("\(stats.tasksCompleted) tasks")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                Text("Avg: \(formatDuration(stats.averageDuration))")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
        }
    }

    @ViewBuilder
    private func statsRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
            Spacer()
            Text(value)
                .font(OAFonts.mono(.body, 14))
        }
    }

    private func agentPercentage(for agentKey: String) -> Double {
        guard let stats = summary.agentBreakdown[agentKey] else { return 0 }
        let total = summary.agentBreakdown.values.reduce(0) { $0 + $1.tasksCompleted }
        guard total > 0 else { return 0 }
        return Double(stats.tasksCompleted) / Double(total) * 100
    }

    private func formatDuration(_ duration: TimeInterval) -> String {
        let minutes = Int(duration / 60)
        return "\(minutes) min"
    }

    private var toolCallBreakdown: String {
        let sorted = summary.toolCallsByType.sorted { $0.value > $1.value }
        let top3 = sorted.prefix(3)
        return top3.map { "\($0.value) \($0.key)" }.joined(separator: ", ")
    }
}
