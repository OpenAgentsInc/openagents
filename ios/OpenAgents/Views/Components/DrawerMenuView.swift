import SwiftUI
import OSLog
import OpenAgentsCore

#if os(iOS)

struct DrawerMenuView: View {
    @EnvironmentObject var bridge: BridgeManager
    var onNavigateToNewChat: () -> Void
    var onNavigateToSetup: () -> Void

    @State private var isLoading = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header spacing
            Color.clear
                .frame(height: 60)

            // Navigation items
            Button(action: onNavigateToNewChat) {
                HStack(spacing: 16) {
                    Image(systemName: "plus.bubble")
                        .font(.system(size: 20))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .frame(width: 24)
                    Text("New Chat")
                        .font(OAFonts.ui(.body, 16))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Spacer()
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 16)
            }
            .buttonStyle(.plain)

            Button(action: onNavigateToSetup) {
                HStack(spacing: 16) {
                    Image(systemName: "gear")
                        .font(.system(size: 20))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .frame(width: 24)
                    Text("Setup")
                        .font(OAFonts.ui(.body, 16))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Spacer()
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 16)
            }
            .buttonStyle(.plain)

            Divider()
                .background(OATheme.Colors.textTertiary.opacity(0.2))
                .padding(.vertical, 8)

            // Recent Sessions Header
            HStack(spacing: 8) {
                Image(systemName: "clock")
                    .font(.system(size: 14))
                    .foregroundStyle(OATheme.Colors.textTertiary)
                Text("Recent Sessions")
                    .font(OAFonts.ui(.caption, 14))
                    .foregroundStyle(OATheme.Colors.textTertiary)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 8)

            // Recent sessions list
            ScrollView {
                if isLoading && bridge.recentSessions.isEmpty {
                    HStack {
                        ProgressView().scaleEffect(0.8)
                        Text("Loading...")
                            .font(OAFonts.ui(.caption, 12))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                    .padding(.horizontal, 24)
                    .padding(.vertical, 8)
                } else if bridge.recentSessions.isEmpty {
                    Text("No recent sessions")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                        .padding(.horizontal, 24)
                        .padding(.vertical, 8)
                } else {
                    ForEach(bridge.recentSessions.prefix(10)) { session in
                        Button(action: {
                            bridge.loadSessionTimeline(sessionId: session.session_id)
                            onNavigateToNewChat()
                        }) {
                            VStack(alignment: .leading, spacing: 4) {
                                HStack(spacing: 8) {
                                    Text(session.session_id.prefix(8) + "...")
                                        .font(OAFonts.ui(.body, 14))
                                        .foregroundStyle(OATheme.Colors.textPrimary)
                                        .lineLimit(1)
                                    if let mode = session.mode {
                                        Text(modeBadgeText(mode))
                                            .font(OAFonts.ui(.caption2, 10))
                                            .foregroundStyle(OATheme.Colors.textSecondary)
                                            .padding(.horizontal, 6)
                                            .padding(.vertical, 2)
                                            .background(
                                                RoundedRectangle(cornerRadius: 4)
                                                    .fill(OATheme.Colors.border.opacity(0.3))
                                            )
                                    }
                                }
                                HStack(spacing: 8) {
                                    Text(relativeTime(session.last_ts))
                                        .font(OAFonts.ui(.caption2, 11))
                                        .foregroundStyle(OATheme.Colors.textTertiary)
                                    Text("•").foregroundStyle(OATheme.Colors.textTertiary)
                                    Text("\(session.message_count) msgs")
                                        .font(OAFonts.ui(.caption2, 11))
                                        .foregroundStyle(OATheme.Colors.textTertiary)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 24)
                            .padding(.vertical, 12)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            Spacer()
        }
        .frame(maxHeight: .infinity)
        .background(Color.black)
        .onAppear { loadSessionsIfConnected() }
        .onChange(of: bridge.status) { _, newStatus in
            if case .connected = newStatus { loadSessionsIfConnected() }
        }
    }

    private func loadSessionsIfConnected() {
        guard case .connected = bridge.status else {
            OpenAgentsLog.ui.warning("DrawerMenu Skipping session load - bridge not connected yet")
            return
        }
        guard !isLoading else { return }
        isLoading = true
        bridge.fetchRecentSessions()
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { isLoading = false }
    }

    private func relativeTime(_ ms: Int64) -> String {
        let now = Int64(Date().timeIntervalSince1970 * 1000)
        let diff = max(0, now - ms)
        let sec = diff / 1000
        if sec < 5 { return "now" }
        if sec < 60 { return "\(sec)s" }
        let min = sec / 60
        if min < 60 { return "\(min)m" }
        let hr = min / 60
        if hr < 24 { return "\(hr)h" }
        let day = hr / 24
        if day < 7 { return "\(day)d" }
        let week = day / 7
        return "\(week)w"
    }

    private func modeBadgeText(_ mode: String) -> String {
        switch mode {
        case "codex": return "Codex"
        case "claude-code", "claude_code": return "Claude"
        case "default_mode": return "Claude"
        default: return mode.capitalized
        }
    }
}
#endif
