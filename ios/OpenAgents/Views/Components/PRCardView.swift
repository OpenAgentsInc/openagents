import SwiftUI
import OpenAgentsCore

struct PRCardView: View {
    let pr: PRSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            // Title and number
            HStack(alignment: .top, spacing: 8) {
                // Status icon
                statusIcon
                    .font(.system(size: 16))

                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 6) {
                        Text("#\(pr.number)")
                            .font(OAFonts.mono(.body, 14))
                            .foregroundStyle(OATheme.Colors.textSecondary)

                        Text(pr.title)
                            .font(OAFonts.ui(.body, 14))
                            .fontWeight(.medium)
                            .foregroundStyle(OATheme.Colors.textPrimary)
                    }

                    // Metadata row
                    HStack(spacing: 12) {
                        // Status badge
                        statusBadge

                        // Agent badge
                        agentBadge

                        // Duration
                        Text("\(formatDuration(pr.duration)) duration")
                            .font(OAFonts.ui(.caption, 12))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }

                    // File changes
                    Text("+\(pr.linesAdded) / -\(pr.linesRemoved) lines in \(pr.filesChanged) files")
                        .font(OAFonts.mono(.caption, 11))
                        .foregroundStyle(OATheme.Colors.textSecondary.opacity(0.8))
                }

                Spacer()
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(OATheme.Colors.border.opacity(0.2))
        )
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch pr.status {
        case .merged:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(Color.purple)
        case .open:
            Image(systemName: "clock.fill")
                .foregroundStyle(Color.orange)
        case .closed:
            Image(systemName: "xmark.circle.fill")
                .foregroundStyle(Color.red)
        case .draft:
            Image(systemName: "pencil.circle.fill")
                .foregroundStyle(Color.gray)
        }
    }

    @ViewBuilder
    private var statusBadge: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(statusColor)
                .frame(width: 6, height: 6)

            Text(statusText)
                .font(OAFonts.ui(.caption, 11))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(
            Capsule()
                .fill(statusColor.opacity(0.15))
        )
    }

    @ViewBuilder
    private var agentBadge: some View {
        Text(agentName)
            .font(OAFonts.ui(.caption, 11))
            .foregroundStyle(agentColor)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(
                Capsule()
                    .fill(agentColor.opacity(0.15))
            )
    }

    private var statusColor: Color {
        switch pr.status {
        case .merged: return Color.purple
        case .open: return Color.orange
        case .closed: return Color.red
        case .draft: return Color.gray
        }
    }

    private var statusText: String {
        switch pr.status {
        case .merged: return "Merged"
        case .open: return "Awaiting review"
        case .closed: return "Closed"
        case .draft: return "Draft"
        }
    }

    private var agentName: String {
        switch pr.agent {
        case .claude_code: return "Claude Code"
        case .codex: return "Codex"
        case .default_mode: return "Default"
        case .orchestrator: return "Orchestrator"
        case .gptoss_20b: return "GPTOSS 20B"
        case .llama_cpp: return "Llama.cpp"
        }
    }

    private var agentColor: Color {
        switch pr.agent {
        case .claude_code: return Color.purple
        case .codex: return Color.teal
        case .default_mode: return Color.blue
        case .orchestrator: return Color.green
        case .gptoss_20b: return Color.indigo
        case .llama_cpp: return Color.orange
        }
    }

    private func formatDuration(_ duration: TimeInterval) -> String {
        let minutes = Int(duration / 60)
        if minutes < 60 {
            return "\(minutes)m"
        } else {
            let hours = minutes / 60
            let remainingMinutes = minutes % 60
            return "\(hours)h \(remainingMinutes)m"
        }
    }
}
