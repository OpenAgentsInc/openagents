import SwiftUI
import OpenAgentsCore

struct DecisionCardView: View {
    let task: OrchestrationTask
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header with icon and timestamp
            HStack(spacing: 8) {
                Text("💡")
                    .font(.system(size: 18))

                Text(formatTime(task.createdAt))
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                Spacer()

                // Status indicator
                statusIndicator
            }

            // Task description
            Text(task.decision.task)
                .font(OAFonts.ui(.body, 14))
                .fontWeight(.medium)
                .foregroundStyle(OATheme.Colors.textPrimary)

            // Metadata row
            HStack(spacing: 12) {
                // Priority badge
                priorityBadge

                // Agent badge
                agentBadge

                // Confidence
                confidenceView
            }

            // Expandable rationale
            if isExpanded {
                VStack(alignment: .leading, spacing: 8) {
                    Divider()
                        .background(OATheme.Colors.border.opacity(0.3))

                    Text("Rationale:")
                        .font(OAFonts.ui(.caption, 11))
                        .fontWeight(.semibold)
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    Text(task.decision.rationale)
                        .font(OAFonts.ui(.body, 13))
                        .foregroundStyle(OATheme.Colors.textSecondary.opacity(0.9))
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            // Expand/collapse button
            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    isExpanded.toggle()
                }
            } label: {
                HStack(spacing: 4) {
                    Text(isExpanded ? "Show Less" : "Show Rationale")
                        .font(OAFonts.ui(.caption, 11))
                        .foregroundStyle(OATheme.Colors.accent)

                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .font(.system(size: 10))
                        .foregroundStyle(OATheme.Colors.accent)
                }
            }
            .buttonStyle(.plain)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(OATheme.Colors.border.opacity(0.2))
        )
    }

    @ViewBuilder
    private var statusIndicator: some View {
        switch task.status {
        case .completed:
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 14))
                .foregroundStyle(OATheme.Colors.success)
        case .failed:
            Image(systemName: "xmark.circle.fill")
                .font(.system(size: 14))
                .foregroundStyle(OATheme.Colors.danger)
        case .in_progress:
            ProgressView()
                .scaleEffect(0.7)
        case .skipped:
            Image(systemName: "arrow.uturn.forward.circle.fill")
                .font(.system(size: 14))
                .foregroundStyle(Color.orange)
        default:
            Image(systemName: "circle")
                .font(.system(size: 14))
                .foregroundStyle(OATheme.Colors.textSecondary.opacity(0.5))
        }
    }

    @ViewBuilder
    private var priorityBadge: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(priorityColor)
                .frame(width: 6, height: 6)

            Text(task.decision.priority.rawValue.capitalized)
                .font(OAFonts.ui(.caption, 11))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(
            Capsule()
                .fill(priorityColor.opacity(0.15))
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

    @ViewBuilder
    private var confidenceView: some View {
        HStack(spacing: 4) {
            Text("\(Int(task.decision.confidence * 100))%")
                .font(OAFonts.mono(.caption, 11))
                .foregroundStyle(OATheme.Colors.textSecondary)

            // Confidence bar
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 2, style: .continuous)
                        .fill(OATheme.Colors.border.opacity(0.3))
                        .frame(height: 4)

                    RoundedRectangle(cornerRadius: 2, style: .continuous)
                        .fill(confidenceColor)
                        .frame(width: geometry.size.width * task.decision.confidence, height: 4)
                }
            }
            .frame(width: 40, height: 4)
        }
    }

    private var priorityColor: Color {
        switch task.decision.priority {
        case .high: return Color.red
        case .medium: return Color.orange
        case .low: return Color.gray
        }
    }

    private var agentName: String {
        switch task.decision.agent {
        case .claude_code: return "Claude Code"
        case .codex: return "Codex"
        case .default_mode: return "Default"
        }
    }

    private var agentColor: Color {
        switch task.decision.agent {
        case .claude_code: return Color.purple
        case .codex: return Color.teal
        case .default_mode: return Color.blue
        }
    }

    private var confidenceColor: Color {
        let confidence = task.decision.confidence
        if confidence >= 0.8 {
            return OATheme.Colors.success
        } else if confidence >= 0.6 {
            return Color.orange
        } else {
            return Color.red
        }
    }

    private func formatTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "h:mm a"
        return formatter.string(from: date)
    }
}
