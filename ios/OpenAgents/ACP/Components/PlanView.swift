import SwiftUI
import OpenAgentsCore

/// Modular component for displaying ACP plan with todos
struct PlanView: View {
    let plan: ACPPlan

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(plan.entries.indices, id: \.self) { index in
                let entry = plan.entries[index]
                HStack(spacing: 8) {
                    // Status indicator
                    Image(systemName: statusIcon(for: entry.status))
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(statusColor(for: entry.status))
                        .frame(width: 16)

                    // Entry content
                    Text(entry.content)
                        .font(OAFonts.ui(.body, 13))
                        .foregroundStyle(
                            entry.status == .completed
                                ? OATheme.Colors.textSecondary.opacity(0.6)
                                : OATheme.Colors.textPrimary
                        )
                        .strikethrough(entry.status == .completed)
                }
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.black.opacity(0.2))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(OATheme.Colors.border.opacity(0.3), lineWidth: 1)
        )
    }

    private func statusIcon(for status: ACPPlanEntryStatus) -> String {
        switch status {
        case .pending: return "circle"
        case .in_progress: return "circle.dotted"
        case .completed: return "checkmark.circle.fill"
        }
    }

    private func statusColor(for status: ACPPlanEntryStatus) -> Color {
        switch status {
        case .pending: return OATheme.Colors.textSecondary.opacity(0.5)
        case .in_progress: return Color.blue
        case .completed: return Color.green
        }
    }
}
