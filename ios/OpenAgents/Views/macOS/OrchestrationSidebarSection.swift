import SwiftUI
import OpenAgentsCore
import os.log

#if os(macOS)
/// Bottom-left sidebar section showing orchestration status
struct OrchestrationSidebarSection: View {
    @ObservedObject var viewModel: OrchestrationViewModel
    @State private var isExpanded: Bool = true

    var body: some View {
        VStack(spacing: 0) {
            Divider()
                .background(OATheme.Colors.border.opacity(0.3))

            VStack(spacing: 8) {
                // Header with expand/collapse
                HStack(spacing: 8) {
                    Image(systemName: "calendar.badge.clock")
                        .foregroundStyle(statusColor)
                        .font(.system(size: 12))

                    Text("Orchestration")
                        .font(OAFonts.mono(.caption, 11))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    Spacer(minLength: 0)

                    // Status indicator
                    Circle()
                        .fill(statusColor)
                        .frame(width: 6, height: 6)

                    // Expand/collapse button
                    Button(action: { isExpanded.toggle() }) {
                        Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                            .foregroundStyle(OATheme.Colors.textTertiary)
                            .font(.system(size: 10))
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 12)
                .padding(.top, 8)

                if isExpanded {
                    expandedContent
                } else {
                    collapsedContent
                }
            }
            .padding(.bottom, 8)
            .background(
                OATheme.Colors.bgQuaternary.opacity(0.5)
            )
        }
    }

    // MARK: - Collapsed View

    private var collapsedContent: some View {
        VStack(spacing: 4) {
            HStack(spacing: 6) {
                Text(stateLabel)
                    .font(OAFonts.mono(.caption, 10))
                    .foregroundStyle(OATheme.Colors.textTertiary)

                Spacer(minLength: 0)

                if let next = viewModel.nextRunTime {
                    Text("Next: \(relativeTime(next))")
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
            }
            .padding(.horizontal, 12)
        }
    }

    // MARK: - Expanded View

    private var expandedContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Current config
            if let config = viewModel.currentConfig {
                HStack(spacing: 6) {
                    Text("Config:")
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                    Text(config.id)
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .lineLimit(1)
                }
                .padding(.horizontal, 12)

                // Schedule expression
                HStack(spacing: 6) {
                    Text("Schedule:")
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                    Text(config.schedule.expression)
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.accent)
                        .lineLimit(1)
                }
                .padding(.horizontal, 12)
            }

            // Next run time
            if let next = viewModel.nextRunTime {
                HStack(spacing: 6) {
                    Text("Next run:")
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                    Text(relativeTime(next))
                        .font(OAFonts.mono(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                    Spacer(minLength: 0)
                    Text(absoluteTime(next))
                        .font(OAFonts.mono(.caption, 9))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                }
                .padding(.horizontal, 12)
            }

            // Recent cycles
            if !viewModel.recentCycles.isEmpty {
                Divider()
                    .background(OATheme.Colors.border.opacity(0.2))
                    .padding(.horizontal, 12)

                Text("Recent Cycles")
                    .font(OAFonts.mono(.caption, 10))
                    .foregroundStyle(OATheme.Colors.textTertiary)
                    .padding(.horizontal, 12)

                VStack(spacing: 4) {
                    ForEach(viewModel.recentCycles.prefix(3)) { cycle in
                        cycleRow(cycle)
                    }
                }
            }

            // Enable/Disable button
            Divider()
                .background(OATheme.Colors.border.opacity(0.2))
                .padding(.horizontal, 12)

            Button(action: { toggleOrchestration() }) {
                HStack(spacing: 8) {
                    Image(systemName: viewModel.isEnabled ? "stop.circle" : "play.circle")
                        .foregroundStyle(viewModel.isEnabled ? OATheme.Colors.danger : OATheme.Colors.accent)
                    Text(viewModel.isEnabled ? "Stop" : "Start")
                        .foregroundStyle(viewModel.isEnabled ? OATheme.Colors.danger : OATheme.Colors.accent)
                }
                .font(OAFonts.mono(.caption, 11))
                .frame(maxWidth: .infinity)
                .padding(.vertical, 6)
                .background(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(OATheme.Colors.bgQuaternary)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .stroke(
                            viewModel.isEnabled ? OATheme.Colors.danger.opacity(0.3) : OATheme.Colors.accent.opacity(0.3),
                            lineWidth: 1
                        )
                )
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 12)
        }
    }

    // MARK: - Cycle Row

    private func cycleRow(_ cycle: OrchestrationViewModel.OrchestrationCycle) -> some View {
        HStack(spacing: 6) {
            Circle()
                .fill(cycleStatusColor(cycle.status))
                .frame(width: 4, height: 4)

            Text(relativeTime(cycle.startTime))
                .font(OAFonts.mono(.caption, 9))
                .foregroundStyle(OATheme.Colors.textTertiary)

            Spacer(minLength: 0)

            if let duration = cycle.duration {
                Text(formatDuration(duration))
                    .font(OAFonts.mono(.caption, 9))
                    .foregroundStyle(OATheme.Colors.textTertiary)
            } else {
                Text("running...")
                    .font(OAFonts.mono(.caption, 9))
                    .foregroundStyle(OATheme.Colors.accent)
            }
        }
        .padding(.horizontal, 12)
    }

    // MARK: - Computed Properties

    private var stateLabel: String {
        switch viewModel.schedulerState {
        case .idle:
            return "Idle"
        case .running:
            return "Running"
        case .paused(let reason):
            return "Paused: \(reason)"
        case .stopped:
            return "Stopped"
        }
    }

    private var statusColor: Color {
        switch viewModel.schedulerState {
        case .idle:
            return OATheme.Colors.textTertiary
        case .running:
            return OATheme.Colors.accent
        case .paused:
            return OATheme.Colors.warning
        case .stopped:
            return OATheme.Colors.danger
        }
    }

    private func cycleStatusColor(_ status: OrchestrationViewModel.OrchestrationCycle.Status) -> Color {
        switch status {
        case .running:
            return OATheme.Colors.accent
        case .completed:
            return OATheme.Colors.success
        case .failed:
            return OATheme.Colors.danger
        case .skipped:
            return OATheme.Colors.textTertiary
        }
    }

    // MARK: - Helpers

    private func relativeTime(_ date: Date) -> String {
        let diff = date.timeIntervalSinceNow
        // If time has passed, don't count up — show "now"
        if diff <= 0 { return "now" }
        if diff < 60 { return "\(Int(diff))s" }
        if diff < 3600 { return "\(Int(diff / 60))m" }
        if diff < 86400 { return "\(Int(diff / 3600))h" }
        return "\(Int(diff / 86400))d"
    }

    private func absoluteTime(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.timeStyle = .short
        formatter.dateStyle = .none
        return formatter.string(from: date)
    }

    private func formatDuration(_ duration: TimeInterval) -> String {
        if duration < 60 {
            return String(format: "%.0fs", duration)
        } else if duration < 3600 {
            let mins = Int(duration / 60)
            let secs = Int(duration.truncatingRemainder(dividingBy: 60))
            return String(format: "%dm %ds", mins, secs)
        } else {
            let hours = Int(duration / 3600)
            let mins = Int((duration.truncatingRemainder(dividingBy: 3600)) / 60)
            return String(format: "%dh %dm", hours, mins)
        }
    }

    private func toggleOrchestration() {
        Task {
            if viewModel.isEnabled {
                await viewModel.stopOrchestration()
            } else {
                // Load test config and start
                let testConfigPath = FileManager.default.homeDirectoryForCurrentUser
                    .appendingPathComponent("code/openagents/docs/overnight/examples/test-every-minute.json")
                    .path

                viewModel.loadConfig(from: testConfigPath)

                // TODO: Wire up actual trigger to bridge/agent coordinator
                await viewModel.startOrchestration {
                    OpenAgentsLog.orchestration.info("Orchestration cycle triggered (stub)")
                }
            }
        }
    }
}
#endif
