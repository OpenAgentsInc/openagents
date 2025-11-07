import SwiftUI

#if os(iOS)

struct SimplifiedIOSView: View {
    @EnvironmentObject var bridge: BridgeManager

    var body: some View {
        NavigationStack {
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
                    }
                    .padding(.horizontal)

                    // Working Directory Section
                    VStack(alignment: .leading, spacing: 12) {
                        Label("Working Directory", systemImage: "folder")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)

                        VStack(spacing: 12) {
                            Image(systemName: "folder")
                                .font(.system(size: 32))
                                .foregroundStyle(OATheme.Colors.textSecondary)

                            Text("Managed on macOS")
                                .font(OAFonts.ui(.body, 14))
                                .foregroundStyle(OATheme.Colors.textSecondary)

                            Text("Set working directory on your Mac")
                                .font(OAFonts.ui(.caption, 12))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                        .frame(maxWidth: .infinity)
                        .padding(24)
                        .background(
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(OATheme.Colors.border.opacity(0.3))
                        )
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
            .navigationTitle("")
            .toolbarTitleDisplayMode(.inline)
        }
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

#Preview {
    SimplifiedIOSView()
        .environmentObject(BridgeManager())
}

#endif
