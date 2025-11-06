import SwiftUI
import OpenAgentsCore

struct PlanStateView: View {
    let state: ACPPlanState
    // Optional per-step statuses to render checkmarks/icons
    var stepStatuses: [String: ACPPlanEntryStatus]? = nil
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Circle()
                    .fill(colorFor(state.status))
                    .frame(width: 8, height: 8)
                Text(titleFor(state.status))
                    .font(OAFonts.ui(.subheadline, 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }
            if let summary = state.summary, !summary.isEmpty {
                Text(summary)
                    .font(OAFonts.ui(.footnote, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }
            if let steps = state.steps, !steps.isEmpty {
                VStack(alignment: .leading, spacing: 4) {
                    ForEach(steps.indices, id: \.self) { idx in
                        HStack(alignment: .top, spacing: 6) {
                            if let st = stepStatuses?[steps[idx]] {
                                statusIcon(for: st)
                            } else {
                                Image(systemName: "circle")
                                    .imageScale(.small)
                                    .foregroundStyle(OATheme.Colors.textTertiary)
                            }
                            Text("\(idx+1).")
                                .font(OAFonts.ui(.footnote, 12))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                            Text(steps[idx])
                                .font(OAFonts.ui(.footnote, 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                                .textSelection(.enabled)
                        }
                    }
                }
            }
        }
    }

    private func colorFor(_ status: ACPPlanStatus) -> Color {
        switch status {
        case .idle: return .gray.opacity(0.6)
        case .running: return .yellow.opacity(0.8)
        case .completed: return OATheme.Colors.success
        case .failed: return OATheme.Colors.danger
        }
    }
    private func titleFor(_ status: ACPPlanStatus) -> String {
        switch status {
        case .idle: return "Plan Idle"
        case .running: return "Plan Running"
        case .completed: return "Plan Complete"
        case .failed: return "Plan Failed"
        }
    }
}

private extension PlanStateView {
    @ViewBuilder
    func statusIcon(for st: ACPPlanEntryStatus) -> some View {
        switch st {
        case .completed:
            Image(systemName: "checkmark.circle.fill")
                .imageScale(.small)
                .foregroundStyle(OATheme.Colors.success)
        case .in_progress:
            Image(systemName: "clock.circle")
                .imageScale(.small)
                .foregroundStyle(.yellow)
        case .pending:
            Image(systemName: "circle")
                .imageScale(.small)
                .foregroundStyle(OATheme.Colors.textTertiary)
        }
    }
}
