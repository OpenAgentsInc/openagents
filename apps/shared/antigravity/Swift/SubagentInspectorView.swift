// Firm reason: 6 (cross-Apple pure-black OLED Subagent Inspector view shared by OpenAgents iOS and OpenAgents macOS).
import SwiftUI

struct SubagentInspectorView: View {
    @ObservedObject var store: AntigravityStore
    let subagent: TranscriptItem

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                Image(systemName: "person.2.wave.2")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.textSecondary)
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(subagent.subagentType.isEmpty ? (subagent.toolName.isEmpty ? "Subagent" : subagent.toolName) : subagent.subagentType)
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(OATheme.textPrimary)
                        if subagent.isBackground {
                            Text("bg")
                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                                .padding(.horizontal, 5)
                                .padding(.vertical, 1)
                                .background(OATheme.surface, in: Capsule())
                        }
                        if !subagent.worktreeBranch.isEmpty {
                            Text(subagent.worktreeBranch)
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                                .lineLimit(1)
                        }
                    }
                    if !subagent.subagentNow.isEmpty || subagent.subagentTokens > 0 {
                        let meta = subagent.subagentTokens > 0
                            ? (subagent.subagentNow.isEmpty ? "\(subagent.subagentTokens) tok" : "\(subagent.subagentNow) · \(subagent.subagentTokens) tok")
                            : subagent.subagentNow
                        Text(meta)
                            .font(.system(size: 11))
                            .foregroundStyle(OATheme.textMuted)
                            .lineLimit(1)
                    }
                }
                Spacer()
                if !subagent.complete {
                    Button {
                        Task { await store.stopSubagent(subagent) }
                    } label: {
                        Label("Stop", systemImage: "stop.fill")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(Color.black)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(Color.white, in: Capsule())
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("subagent.stop")
                }
                Button {
                    Task { await store.refreshSubagent() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                        .frame(width: 28, height: 28)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(OATheme.background)

            Divider().overlay(OATheme.hairline)

            if store.isLoadingSubagent && store.subagentItems.isEmpty {
                VStack(spacing: 10) {
                    Spacer()
                    ProgressView().tint(OATheme.textPrimary)
                    Text("Loading subagent transcript…")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if store.subagentItems.isEmpty {
                VStack(spacing: 8) {
                    Spacer()
                    Text("No live subagent items available.")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                    if !subagent.text.isEmpty {
                        Text(subagent.text)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .padding(12)
                    }
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        if store.hasOlderSubagentItems {
                            Button {
                                Task { await store.loadOlderSubagentItems() }
                            } label: {
                                HStack(spacing: 6) {
                                    if store.isLoadingOlderSubagentItems {
                                        ProgressView().controlSize(.small).tint(OATheme.textSecondary)
                                    }
                                    Text("Load earlier steps")
                                        .font(.system(size: 12, weight: .medium))
                                }
                                .foregroundStyle(OATheme.textSecondary)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 7)
                                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                            .disabled(store.isLoadingOlderSubagentItems)
                        }
                        ForEach(store.subagentItems) { child in
                            TranscriptRowView(item: child)
                        }
                    }
                    .padding(14)
                }
            }
        }
        .background(OATheme.background)
    }
}
