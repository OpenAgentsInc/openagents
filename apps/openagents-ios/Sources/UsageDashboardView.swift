// Firm reason: 6 (iOS pure-black OLED SwiftUI Usage & Credits sheet backed by /api/v1/inference/usage and /api/auth/life).
import SwiftUI

struct UsageDashboardView: View {
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                // 1. Active Session Context
                Section {
                    metricRow(label: "Routing", value: "OpenAgents Auto", accent: OATheme.textPrimary)
                    metricRow(label: "Execution Target", value: cloudChat.selectedTarget.displayLabel, accent: OATheme.statusReady)
                    if let proj = cloudChat.selectedProject {
                        metricRow(label: "Active Project", value: proj.name, accent: OATheme.textSecondary)
                    }
                    if account.isLoggedIn {
                        metricRow(label: "GitHub", value: account.formattedGitHubStatus, accent: account.githubConnected ? OATheme.statusReady : OATheme.textSecondary)
                    }
                } header: {
                    Text("Active Session")
                }

                // 2. OpenAgents Cloud Spend & Credits
                Section {
                    let snap = cloudChat.usageSnapshot
                    metricRow(label: "Today Spend", value: CloudUsageSnapshot.formatUsd(micros: snap.todayUsdMicros), accent: OATheme.textPrimary)
                    let totalSpendMicros = snap.totalUsdMicros > 0 ? snap.totalUsdMicros : snap.monthUsdMicros
                    metricRow(label: "Total Spend", value: CloudUsageSnapshot.formatUsd(micros: totalSpendMicros), accent: OATheme.textSecondary)
                    let tokenCount = snap.totalTokens > 0 ? snap.totalTokens : snap.todayTokens
                    metricRow(label: "Cloud Tokens", value: CloudUsageSnapshot.formatTokens(tokenCount), accent: OATheme.textSecondary)
                    metricRow(label: "Enrolled Computer Turns ($0)", value: "\(snap.localTurns)", accent: OATheme.statusReady)
                    if snap.sandboxTurns > 0 {
                        metricRow(label: "Cloud Sandbox Turns", value: "\(snap.sandboxTurns) (\(snap.sandboxSeconds)s)", accent: OATheme.textSecondary)
                    }
                    if account.isLoggedIn {
                        metricRow(label: "Paid Credits", value: account.formattedPaidCredits, accent: OATheme.textPrimary)
                        let promoMicros = max(account.promoMicros, snap.promoMicros)
                        metricRow(label: "Promo Credits", value: CloudUsageSnapshot.formatUsd(micros: promoMicros), accent: promoMicros > 0 ? OATheme.statusReady : OATheme.textSecondary)
                    }

                    Button {
                        Task {
                            await account.refresh()
                            await cloudChat.refreshUsage()
                        }
                    } label: {
                        HStack {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 13))
                            Text("Refresh Cloud Usage")
                                .font(.system(size: 13.5, weight: .semibold))
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .disabled(cloudChat.isLoadingUsage)
                    .accessibilityIdentifier("usage.refresh")
                    .listRowBackground(OATheme.elevated)
                } header: {
                    HStack {
                        Text("OpenAgents Cloud Usage")
                        Spacer()
                        if cloudChat.isLoadingUsage {
                            ProgressView().controlSize(.mini)
                        }
                    }
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .navigationTitle("Usage & Credits")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .accessibilityIdentifier("usage.done")
                }
            }
        }
        .preferredColorScheme(.dark)
        .task {
            if account.isLoggedIn {
                await cloudChat.refreshUsage()
            }
        }
    }

    private func metricRow(label: String, value: String, accent: Color) -> some View {
        HStack {
            Text(label)
                .font(.system(size: 13.5))
                .foregroundStyle(OATheme.textSecondary)
            Spacer()
            Text(value)
                .font(.system(size: 13.5, weight: .semibold, design: .monospaced))
                .foregroundStyle(accent)
        }
        .listRowBackground(OATheme.surface)
    }
}
