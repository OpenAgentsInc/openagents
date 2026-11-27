// Firm reason: 6 (iOS pure-black OLED SwiftUI Usage Dashboard & Full Auto / Voice Settings sheet backed by Coax #authority telemetry and GetUsage RPC; #14068).
import SwiftUI

struct UsageDashboardView: View {
    @ObservedObject var store: AppStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                // 1. Active Session Context & Cost
                Section {
                    if let usage = store.activeUsage, usage.totalTokens > 0 {
                        let pct = max(0, min(100, Int(usage.contextPercent.rounded())))
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Context Window")
                                    .font(.system(size: 13.5, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                Spacer()
                                Text("\(pct)% of 1M")
                                    .font(.system(size: 12.5, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(pct >= 85 ? OATheme.statusError : (pct >= 65 ? OATheme.statusWarn : OATheme.textPrimary))
                            }
                            ProgressView(value: Double(pct), total: 100.0)
                                .tint(pct >= 85 ? OATheme.statusError : (pct >= 65 ? OATheme.statusWarn : .white))
                            Text(usage.summaryLabel)
                                .font(.system(size: 11.5, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                            HStack {
                                Text("Active Model")
                                    .font(.system(size: 12))
                                    .foregroundStyle(OATheme.textMuted)
                                Spacer()
                                Text("\(store.selectedModelOption.label) · \(usage.formattedCost(model: store.selectedModelID))")
                                    .font(.system(size: 12, design: .monospaced))
                                    .foregroundStyle(store.selectedModelOption.isLocalFreeCompute ? OATheme.statusReady : OATheme.textPrimary)
                            }
                        }
                        .padding(.vertical, 4)
                        .listRowBackground(OATheme.surface)
                    } else {
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text("Active Session")
                                    .font(.system(size: 13.5, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                Text("Model: \(store.selectedModelOption.label) (\(store.selectedModelOption.isLocalFreeCompute ? "$0.00 local" : "OpenAgents Cloud"))")
                                    .font(.system(size: 12, design: .monospaced))
                                    .foregroundStyle(OATheme.textSecondary)
                            }
                            Spacer()
                            Text("0% ctx")
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                        }
                        .padding(.vertical, 4)
                        .listRowBackground(OATheme.surface)
                    }
                } header: {
                    Text("Active Session")
                }

                // 2. Daily & Fleet Spend (OpenAgents Cloud retail pricing + $0 local compute)
                Section {
                    let snap = store.usageSnapshot
                    let activeCost = store.activeUsage?.estimatedCostMicros(model: store.selectedModelID) ?? 0
                    let todayCost = max(snap.todayUsdMicros, activeCost)
                    metricRow(label: "Today Spend", value: UsageDashboardSnapshot.formatUsd(micros: todayCost), accent: OATheme.textPrimary)
                    metricRow(label: "Month Spend", value: UsageDashboardSnapshot.formatUsd(micros: max(snap.monthUsdMicros, todayCost)), accent: OATheme.textSecondary)
                    metricRow(label: "Local Compute (Inkling)", value: "$0.00", accent: OATheme.statusReady)
                    if snap.todayTokens > 0 {
                        metricRow(label: "Today Tokens", value: UsageDashboardSnapshot.formatTokens(snap.todayTokens), accent: OATheme.textSecondary)
                    }
                    if snap.creditMsat > 0 {
                        metricRow(label: "Account Credits", value: "\(snap.creditMsat / 1000) sats", accent: OATheme.statusReady)
                    }
                } header: {
                    HStack {
                        Text("OpenAgents Cloud & Local Spend")
                        Spacer()
                        if store.isLoadingUsage {
                            ProgressView().controlSize(.mini)
                        }
                    }
                }

                // 3. Provider Quotas & Reset Windows
                Section {
                    if store.usageSnapshot.quotas.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Circle()
                                    .fill(OATheme.statusReady)
                                    .frame(width: 7, height: 7)
                                Text("OpenAgents Core")
                                    .font(.system(size: 13.5, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                Spacer()
                                Text("Ready")
                                    .font(.system(size: 11.5, design: .monospaced))
                                    .foregroundStyle(OATheme.statusReady)
                            }
                            Text("Cloud retail metering active; on-device Inkling 2 Mini turns always run at $0.00.")
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(.vertical, 4)
                        .listRowBackground(OATheme.surface)
                    } else {
                        ForEach(store.usageSnapshot.quotas) { row in
                            VStack(alignment: .leading, spacing: 6) {
                                HStack {
                                    Text(row.provider.capitalized)
                                        .font(.system(size: 13.5, weight: .semibold))
                                        .foregroundStyle(OATheme.textPrimary)
                                    if !row.plan.isEmpty {
                                        Text(row.plan)
                                            .font(.system(size: 11, design: .monospaced))
                                            .foregroundStyle(OATheme.textMuted)
                                    }
                                    Spacer()
                                    Text("\(row.usedPercent)%")
                                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                        .foregroundStyle(row.usedPercent >= 95 ? OATheme.statusError : (row.usedPercent >= 80 ? OATheme.statusWarn : OATheme.statusReady))
                                }
                                ProgressView(value: Double(row.usedPercent), total: 100.0)
                                    .tint(row.usedPercent >= 95 ? OATheme.statusError : (row.usedPercent >= 80 ? OATheme.statusWarn : OATheme.statusReady))
                                HStack {
                                    Text(row.windowLabel.isEmpty ? row.status : row.windowLabel)
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundStyle(OATheme.textSecondary)
                                    Spacer()
                                    if !row.formattedReset.isEmpty {
                                        Text(row.formattedReset)
                                            .font(.system(size: 11, design: .monospaced))
                                            .foregroundStyle(OATheme.textMuted)
                                    }
                                }
                            }
                            .padding(.vertical, 4)
                            .listRowBackground(OATheme.surface)
                        }
                    }

                    Button {
                        Task { await store.refreshUsage() }
                    } label: {
                        HStack {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 13))
                            Text("Refresh Usage & Quotas")
                                .font(.system(size: 13.5, weight: .semibold))
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .accessibilityIdentifier("usage.refresh")
                    .listRowBackground(OATheme.elevated)
                } header: {
                    Text("Provider Quotas")
                }

                // 4. Full Auto & Unfettered Controls (Desktop Settings parity)
                Section {
                    Toggle("Full Auto Mode (Zero Approval Nags)", isOn: Binding(
                        get: { store.fullAutoEnabled },
                        set: { store.setFullAuto($0) }
                    ))
                    .font(.system(size: 14))
                    .tint(OATheme.statusReady)
                    .accessibilityIdentifier("usage.fullAutoToggle")
                    .listRowBackground(OATheme.surface)

                    Toggle("Auto-Answer Structured Questions", isOn: Binding(
                        get: { store.autoAnswerQuestions },
                        set: { store.setAutoAnswerQuestions($0) }
                    ))
                    .font(.system(size: 14))
                    .tint(OATheme.statusReady)
                    .disabled(!store.fullAutoEnabled)
                    .listRowBackground(OATheme.surface)
                } header: {
                    Text("Full Auto Policy")
                } footer: {
                    Text("When Full Auto is enabled, approvals and plans auto-allow cleanly via Coax #authority policy without pausing active turns.")
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .navigationTitle("Usage & Controls")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .accessibilityIdentifier("usage.done")
                }
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            Task { await store.refreshUsage() }
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
