import SwiftUI
import OpenAgentsCore

#if os(iOS)
import UIKit

// MARK: - Navigation Container

struct SimplifiedNavigationView: View {
    @EnvironmentObject var bridge: BridgeManager
    @State private var navigationPath = NavigationPath()
    @State private var isMenuPresented = false
    @State private var selectedAgent: String = "Codex"
    @State private var detectedAgents: [String] = []

    var body: some View {
        ZStack {
            NavigationStack(path: $navigationPath) {
                NewChatView(
                    isMenuPresented: $isMenuPresented,
                    selectedAgent: $selectedAgent,
                    detectedAgents: detectedAgents,
                    onNavigateToSetup: {
                        navigationPath.append("setup")
                    }
                )
                .navigationDestination(for: String.self) { destination in
                    if destination == "setup" {
                        SimplifiedIOSView(
                            isMenuPresented: $isMenuPresented,
                            onNavigateToNewChat: {
                                navigationPath.removeLast()
                            }
                        )
                    }
                }
                .onAppear {
                    setupAgents()
                }
            }

            // Overlay when drawer is open
            Color.gray.opacity(isMenuPresented ? 0.3 : 0)
                .ignoresSafeArea()
                .allowsHitTesting(isMenuPresented)
                .onTapGesture {
                    withAnimation(.easeInOut(duration: 0.3)) {
                        isMenuPresented = false
                    }
                }
                .animation(.easeInOut(duration: 0.3), value: isMenuPresented)

            // Drawer from left
            HStack(spacing: 0) {
                DrawerMenuView(
                    onNavigateToNewChat: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            isMenuPresented = false
                        }
                        // Go back to root (New Chat) - no animation
                        var transaction = Transaction()
                        transaction.disablesAnimations = true
                        withTransaction(transaction) {
                            navigationPath = NavigationPath()
                        }
                    },
                    onNavigateToSetup: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            isMenuPresented = false
                        }
                        // Navigate to setup - no animation
                        var transaction = Transaction()
                        transaction.disablesAnimations = true
                        withTransaction(transaction) {
                            navigationPath.append("setup")
                        }
                    }
                )
                .frame(width: UIScreen.main.bounds.width * 0.75)
                .offset(x: isMenuPresented ? 0 : -UIScreen.main.bounds.width * 0.75)
                .animation(.easeInOut(duration: 0.3), value: isMenuPresented)

                Spacer()
            }
        }
    }

    private func setupAgents() {
        // Agents are managed on macOS, so we just show both options
        // User can select which one they want to use for this chat
        detectedAgents = ["Codex", "Claude Code"]

        // Default to Codex
        selectedAgent = "Codex"
    }
}

// MARK: - Setup View

struct SimplifiedIOSView: View {
    @EnvironmentObject var bridge: BridgeManager
    @Binding var isMenuPresented: Bool
    var onNavigateToNewChat: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            // Custom header - no glass
            VStack(spacing: 0) {
                Color.clear
                    .frame(height: 0)
                    .frame(maxWidth: .infinity)
                    .background(.black)
                    .ignoresSafeArea(edges: .top)

                HStack(spacing: 16) {
                    // Bare hamburger menu icon
                    Button(action: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            isMenuPresented.toggle()
                        }
                    }) {
                        Image(systemName: "line.3.horizontal")
                            .foregroundStyle(.white)
                            .font(.system(size: 18))
                    }
                    .buttonStyle(.plain)

                    Text("Setup")
                        .font(OAFonts.ui(.headline, 16))
                        .foregroundStyle(.white)

                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(.black)
            }

            ScrollView {
                VStack(spacing: 32) {
                    // App title
                    VStack(spacing: 8) {
                        Text("OpenAgents")
                            .font(OAFonts.ui(.largeTitle, 32))
                            .fontWeight(.bold)
                            .foregroundStyle(OATheme.Colors.textPrimary)

                        Text("Mobile Command Center")
                            .font(OAFonts.ui(.body, 15))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                    .padding(.top, 40)

                    // Bridge Connection Status Section
                    VStack(alignment: .leading, spacing: 12) {
                        Label("Bridge Connection", systemImage: "network")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)

                        HStack(spacing: 16) {
                            // Status indicator
                            if case .connected = bridge.status {
                                // Static checkmark when connected
                                Image(systemName: "checkmark.circle.fill")
                                    .font(.system(size: 20))
                                    .foregroundStyle(OATheme.Colors.success)
                            } else if case .discovering = bridge.status {
                                // Loading spinner while discovering
                                ProgressView()
                                    .scaleEffect(0.8)
                            } else if case .connecting = bridge.status {
                                // Loading spinner while connecting
                                ProgressView()
                                    .scaleEffect(0.8)
                            } else {
                                // Idle or other states
                                Image(systemName: "circle")
                                    .font(.system(size: 20))
                                    .foregroundStyle(OATheme.Colors.textTertiary)
                            }

                            VStack(alignment: .leading, spacing: 4) {
                                Text(bridgeStatusText)
                                    .font(OAFonts.ui(.body, 15))
                                    .fontWeight(.medium)
                                    .foregroundStyle(OATheme.Colors.textPrimary)

                                if case .connected(let host, let port) = bridge.status {
                                    Text("\(host):\(String(port))")
                                        .font(OAFonts.ui(.caption, 12))
                                        .foregroundStyle(OATheme.Colors.textSecondary)
                                }

                                if case .error(let msg) = bridge.status {
                                    Text(msg)
                                        .font(OAFonts.ui(.caption, 11))
                                        .foregroundStyle(OATheme.Colors.danger)
                                        .lineLimit(2)
                                }
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .frame(minHeight: 60)
                        .padding(20)
                        .background(
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(OATheme.Colors.border.opacity(0.3))
                        )

                        // Session indicator row
                        HStack(spacing: 8) {
                            Image(systemName: "number")
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                            if let sid = bridge.currentSessionId?.value, !sid.isEmpty {
                                Text("Session: \(sid.prefix(8))…")
                                    .font(OAFonts.ui(.caption, 12))
                                    .foregroundStyle(OATheme.Colors.textSecondary)
                                Button(action: { UIPasteboard.general.string = sid }) {
                                    Image(systemName: "doc.on.doc")
                                        .font(.system(size: 12))
                                        .foregroundStyle(OATheme.Colors.textTertiary)
                                }
                                .buttonStyle(.plain)
                                .accessibilityLabel("Copy session ID")
                            } else {
                                Text("Session: No session")
                                    .font(OAFonts.ui(.caption, 12))
                                    .foregroundStyle(OATheme.Colors.textTertiary)
                            }
                            Spacer()
                        }
                        .padding(.horizontal, 4)
                    }
                    .padding(.horizontal)

                    // Working Directory Section
                    VStack(alignment: .leading, spacing: 12) {
                        Label("Working Directory", systemImage: "folder")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)

                        if let workingDir = bridge.workingDirectory {
                            // Show working directory from macOS
                            HStack(spacing: 12) {
                                Image(systemName: "folder.fill")
                                    .font(.system(size: 24))
                                    .foregroundStyle(OATheme.Colors.accent)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text(URL(fileURLWithPath: workingDir).lastPathComponent)
                                        .font(OAFonts.ui(.body, 14))
                                        .fontWeight(.medium)
                                        .foregroundStyle(OATheme.Colors.textPrimary)

                                    Text(workingDir)
                                        .font(OAFonts.ui(.caption, 11))
                                        .foregroundStyle(OATheme.Colors.textTertiary)
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                }

                                Spacer()
                            }
                            .frame(maxWidth: .infinity)
                            .padding(16)
                            .background(
                                RoundedRectangle(cornerRadius: 12, style: .continuous)
                                    .fill(OATheme.Colors.border.opacity(0.3))
                            )
                        } else {
                            // No working directory set
                            VStack(spacing: 12) {
                                Image(systemName: "folder.badge.questionmark")
                                    .font(.system(size: 32))
                                    .foregroundStyle(OATheme.Colors.textTertiary)

                                Text("Not set")
                                    .font(OAFonts.ui(.body, 14))
                                    .foregroundStyle(OATheme.Colors.textSecondary)

                                Text("Set working directory on macOS")
                                    .font(OAFonts.ui(.caption, 12))
                                    .foregroundStyle(OATheme.Colors.textTertiary)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(24)
                            .background(
                                RoundedRectangle(cornerRadius: 12, style: .continuous)
                                    .fill(Color.black.opacity(0.1))
                                    .strokeBorder(OATheme.Colors.textTertiary.opacity(0.2), lineWidth: 1)
                            )
                        }
                    }
                    .padding(.horizontal)

                    // Enabled Agents Section
                    VStack(alignment: .leading, spacing: 12) {
                        Label("Enabled Agents", systemImage: "terminal")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)

                        VStack(spacing: 8) {
                            AgentInfoRow(name: "OpenAI Codex")
                            AgentInfoRow(name: "Claude Code")
                        }
                        .frame(maxWidth: .infinity)
                        .padding(16)
                        .background(
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(OATheme.Colors.border.opacity(0.3))
                        )
                    }
                    .padding(.horizontal)

                    Spacer(minLength: 40)
                }
            }
            .background(OATheme.Colors.background)
        }
        .background(OATheme.Colors.background)
        .navigationBarHidden(true)
        .preferredColorScheme(.dark)
    }

    private var bridgeStatusText: String {
        switch bridge.status {
        case .idle: return "Idle"
        case .discovering: return "Discovering Desktop..."
        case .connecting(let h, let p): return "Connecting to \(h):\(p)"
        case .handshaking(let h, let p): return "Handshaking with \(h):\(p)"
        case .connected: return "Connected"
        case .advertising: return "Advertising (Server Mode)"
        case .error: return "Error"
        }
    }
}

// MARK: - Agent Info Row Component

struct AgentInfoRow: View {
    let name: String

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Text("Managed on macOS")
                    .font(OAFonts.ui(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textTertiary)
            }

            Spacer()

            Image(systemName: "checkmark.circle")
                .font(.system(size: 20))
                .foregroundStyle(OATheme.Colors.success)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(OATheme.Colors.border.opacity(0.5))
        )
    }
}

// MARK: - Drawer Menu View

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
                        ProgressView()
                            .scaleEffect(0.8)
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
                                    Text("•")
                                        .foregroundStyle(OATheme.Colors.textTertiary)
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
        .onAppear {
            loadSessionsIfConnected()
        }
        .onChange(of: bridge.status) { _, newStatus in
            // Load sessions when bridge connects
            if case .connected = newStatus {
                loadSessionsIfConnected()
            }
        }
    }

    private func loadSessionsIfConnected() {
        guard case .connected = bridge.status else {
            print("[DrawerMenu] Skipping session load - bridge not connected yet")
            return
        }
        guard !isLoading else { return }
        isLoading = true
        bridge.fetchRecentSessions()
        // Reset loading after a delay since we don't have completion callbacks
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            isLoading = false
        }
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

#Preview {
    SimplifiedNavigationView()
        .environmentObject(BridgeManager())
}

#endif
