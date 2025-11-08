import SwiftUI

struct MorningBriefingStatsView: View {
    let summary: OvernightRunSummary

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            HStack(spacing: 8) {
                Text("🌙")
                    .font(.system(size: 24))
                Text("Overnight Run Complete")
                    .font(OAFonts.ui(.headline, 18))
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }

            // Date range and duration
            Text(dateRangeText)
                .font(OAFonts.ui(.body, 14))
                .foregroundStyle(OATheme.Colors.textSecondary)

            // Key metrics grid
            HStack(spacing: 16) {
                metricCard(
                    value: "\(summary.totalCycles)",
                    label: "Decisions"
                )

                metricCard(
                    value: "\(summary.completedTasks)",
                    label: "Completed"
                )

                metricCard(
                    value: "\(summary.prsCreated)",
                    label: "PRs Created"
                )
            }
        }
        .padding(20)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(OATheme.Colors.border.opacity(0.3))
        )
    }

    private var dateRangeText: String {
        let formatter = DateFormatter()
        formatter.dateFormat = "MMM d, yyyy • h:mm a"
        let start = formatter.string(from: summary.startTime)

        formatter.dateFormat = "h:mm a"
        let end = formatter.string(from: summary.endTime)

        let hours = Int(summary.duration / 3600)
        return "\(start) - \(end) (\(hours) hours)"
    }

    @ViewBuilder
    private func metricCard(value: String, label: String) -> some View {
        VStack(spacing: 4) {
            Text(value)
                .font(OAFonts.ui(.title, 24))
                .fontWeight(.bold)
                .foregroundStyle(OATheme.Colors.textPrimary)

            Text(label)
                .font(OAFonts.ui(.caption, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .frame(maxWidth: .infinity)
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(OATheme.Colors.background)
        )
    }
}
