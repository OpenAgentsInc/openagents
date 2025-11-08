import SwiftUI

#if os(macOS)
import AppKit
import OpenAgentsCore

struct SimplifiedMacOSView: View {
    @EnvironmentObject var bridge: BridgeManager
    @EnvironmentObject var tinyvex: TinyvexManager
    @State private var showInstructions = false

    // Coding agents state
    @State private var claudeDetected = false
    @State private var codexDetected = false
    @State private var claudeEnabled = true
    @State private var codexEnabled = true
    @State private var openagentsEnabled = true
    #if DEBUG
    @StateObject private var nostrRelayManager = NostrRelayManager()
    #endif

    var body: some View {
        NavigationStack {
            ZStack {
                // Background
                OATheme.Colors.background

                // Main content - centered
                VStack(spacing: 40) {
                Spacer()

                // App title
                VStack(spacing: 8) {
                    Text("OpenAgents")
                        .font(OAFonts.ui(.largeTitle, 32))
                        .fontWeight(.bold)
                        .foregroundStyle(OATheme.Colors.textPrimary)

                    Text("Desktop Command Center")
                        .font(OAFonts.ui(.body, 15))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }

                // Two-column layout of cards (less vertical, more horizontal)
                // Independent column stacks so the left column doesn't shift
                // when the right column grows taller (masonry-like layout).
                HStack(alignment: .top, spacing: 24) {
                    VStack(alignment: .leading, spacing: 24) {
                        bridgeStatusCard
                        configureAgentsCard
                        #if DEBUG
                        nostrDevCard
                        morningBriefingDemoCard
                        #endif
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)

                    VStack(alignment: .leading, spacing: 24) {
                        workingDirectoryCard
                        #if DEBUG
                        tinyvexDevCard
                        nostrEventFeedCard
                        #endif
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .frame(maxWidth: 1100)

                #if false
                // Bridge Status Section (legacy vertical layout)
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text("Bridge Status")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Spacer()
                        Button(action: { showInstructions = true }) {
                            HStack(spacing: 8) {
                                Image(systemName: "info.circle")
                                Text("View Setup Instructions")
                            }
                            .font(OAFonts.ui(.body, 14))
                            .foregroundStyle(OATheme.Colors.accent)
                        }
                        .buttonStyle(.plain)
                    }
                    .frame(maxWidth: 500)

                    HStack(spacing: 16) {
                        // Status indicator
                        if case .advertising = bridge.status {
                            // Static checkmark when advertising
                            Image(systemName: "checkmark.circle.fill")
                                .font(.system(size: 20))
                                .foregroundStyle(OATheme.Colors.success)
                        } else {
                            // Loading spinner otherwise
                            ProgressView()
                                .scaleEffect(0.8)
                        }

                        VStack(alignment: .leading, spacing: 4) {
                            Text(bridgeStatusText)
                                .font(OAFonts.ui(.body, 15))
                                .fontWeight(.medium)
                                .foregroundStyle(OATheme.Colors.textPrimary)

                            HStack(spacing: 8) {
                                if case .advertising(let port) = bridge.status {
                                    Text("Port: \(String(port))")
                                        .font(OAFonts.ui(.caption, 12))
                                        .foregroundStyle(OATheme.Colors.textSecondary)

                                    if bridge.connectedClientCount > 0 {
                                        Text("•")
                                            .font(OAFonts.ui(.caption, 12))
                                            .foregroundStyle(OATheme.Colors.textTertiary)
                                        Text("\(bridge.connectedClientCount) client\(bridge.connectedClientCount == 1 ? "" : "s") connected")
                                            .font(OAFonts.ui(.caption, 12))
                                            .foregroundStyle(OATheme.Colors.success)
                                    }
                                }
                            }

                            if case .error(let msg) = bridge.status {
                                Text(msg)
                                    .font(OAFonts.ui(.caption, 11))
                                    .foregroundStyle(OATheme.Colors.danger)
                                    .lineLimit(2)
                            }
                        }
                    }
                    .frame(maxWidth: 500, minHeight: 60)
                    .padding(20)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(OATheme.Colors.border.opacity(0.3))
                    )

                    // Session indicator
                    HStack(spacing: 8) {
                        Image(systemName: "number")
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.Colors.textTertiary)
                        if let sid = bridge.currentSessionId?.value, !sid.isEmpty {
                            Text("Session: \(sid.prefix(8))…")
                                .font(OAFonts.ui(.caption, 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                            Button(action: {
                                let pb = NSPasteboard.general
                                pb.clearContents()
                                pb.setString(sid, forType: .string)
                            }) {
                                Image(systemName: "doc.on.doc")
                                    .font(.system(size: 12))
                                    .foregroundStyle(OATheme.Colors.textTertiary)
                            }
                            .buttonStyle(.plain)
                            .help("Copy session ID")
                        } else {
                            Text("Session: No session")
                                .font(OAFonts.ui(.caption, 12))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                        Spacer()
                    }
                    .frame(maxWidth: 500)
                }
                .frame(maxWidth: 500)

                Divider()
                    .background(OATheme.Colors.border)
                    .frame(maxWidth: 500)

                // Working Directory Section
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text("Working Directory")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Spacer()
                    }
                    .frame(maxWidth: 500)

                    VStack(spacing: 12) {
                        if let dir = bridge.workingDirectory {
                            // Show selected directory
                            HStack(spacing: 12) {
                                Image(systemName: "folder.fill")
                                    .font(.system(size: 24))
                                    .foregroundStyle(OATheme.Colors.accent)

                                VStack(alignment: .leading, spacing: 2) {
                                    Text(dir.lastPathComponent)
                                        .font(OAFonts.ui(.body, 14))
                                        .fontWeight(.medium)
                                        .foregroundStyle(OATheme.Colors.textPrimary)

                                    Text(dir.path)
                                        .font(OAFonts.ui(.caption, 11))
                                        .foregroundStyle(OATheme.Colors.textTertiary)
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                }

                                Spacer()

                                Button(action: {
                                    selectWorkingDirectory()
                                }) {
                                    Text("Change")
                                        .font(OAFonts.ui(.body, 14))
                                }
                                .buttonStyle(.bordered)
                            }
                            .frame(maxWidth: 500)
                            .padding(16)
                            .background(
                                RoundedRectangle(cornerRadius: 12, style: .continuous)
                                    .fill(Color.black.opacity(0.15))
                            )
                        } else {
                            // No directory selected
                            VStack(spacing: 12) {
                                Image(systemName: "folder.badge.questionmark")
                                    .font(.system(size: 32))
                                    .foregroundStyle(OATheme.Colors.textTertiary)

                                Text("No working directory selected")
                                    .font(OAFonts.ui(.body, 14))
                                    .foregroundStyle(OATheme.Colors.textSecondary)

                                Button(action: {
                                    selectWorkingDirectory()
                                }) {
                                    Text("Select Directory")
                                        .font(OAFonts.ui(.body, 16))
                                }
                                .buttonStyle(.borderedProminent)
                                .controlSize(.large)
                            }
                            .frame(maxWidth: 400)
                            .padding(24)
                            .background(
                                RoundedRectangle(cornerRadius: 16, style: .continuous)
                                    .fill(Color.black.opacity(0.1))
                                    .strokeBorder(OATheme.Colors.textTertiary.opacity(0.2), lineWidth: 1)
                            )
                        }
                    }
                }
                .frame(maxWidth: 500)

                Divider()
                    .background(OATheme.Colors.border)
                    .frame(maxWidth: 500)

                // Configure Coding Agents Section
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text("Configure Coding Agents")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Spacer()
                    }
                    .frame(maxWidth: 500)

                    VStack(spacing: 8) {
                        // // OpenAgents Coder (always available)
                        // AgentToggleRow(
                        //     name: "OpenAgents Coder",
                        //     detected: true,
                        //     enabled: $openagentsEnabled,
                        //     showStatus: false
                        // )

                        // OpenAI Codex
                        AgentToggleRow(
                            name: "OpenAI Codex",
                            detected: codexDetected,
                            enabled: $codexEnabled,
                            showStatus: true
                        )

                        // Claude Code
                        AgentToggleRow(
                            name: "Claude Code",
                            detected: claudeDetected,
                            enabled: $claudeEnabled,
                            showStatus: true
                        )
                    }
                    .frame(maxWidth: 500)
                    .padding(16)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(OATheme.Colors.border.opacity(0.3))
                    )
                }
                .frame(maxWidth: 500)
                .onAppear {
                    detectAgents()
                }

                // Developer-only: Tinyvex status
                #if DEBUG
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text("Tinyvex (Dev)")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Spacer()
                    }
                    .frame(maxWidth: 500)

                    HStack(spacing: 16) {
                        Image(systemName: tinyvex.isRunning ? "checkmark.circle.fill" : "xmark.circle.fill")
                            .font(.system(size: 20))
                            .foregroundStyle(tinyvex.isRunning ? OATheme.Colors.success : OATheme.Colors.danger)

                        VStack(alignment: .leading, spacing: 6) {
                            Text(tinyvex.isRunning ? "DB Connected" : "DB Stopped")
                                .font(OAFonts.ui(.body, 15))
                                .foregroundStyle(OATheme.Colors.textPrimary)
                            Text("Path: \(tinyvex.dbPath)")
                                .font(OAFonts.ui(.caption, 11))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                                .lineLimit(2)
                            HStack(spacing: 12) {
                                Text("Tables: \(tinyvex.tableCount)")
                                Text("Rows: \(tinyvex.rowCount)")
                                Text(String(format: "Size: %.2f MB", Double(tinyvex.fileSizeBytes) / (1024.0*1024.0)))
                            }
                            .font(OAFonts.ui(.caption, 11))
                            .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                    }
                    .frame(maxWidth: 500, minHeight: 60)
                    .padding(20)
                    .background(
                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                            .fill(OATheme.Colors.border.opacity(0.3))
                    )
                }
                .frame(maxWidth: 500)

                // Developer-only: Nostr keypair generator
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Text("Nostr (Dev)")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Spacer()
                    }
                    .frame(maxWidth: 500)

                    NostrKeygenCard()
                        .frame(maxWidth: 500)
                        .padding(20)
                        .background(
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(OATheme.Colors.border.opacity(0.3))
                        )
                }
                .frame(maxWidth: 500)
                #endif
                #endif // legacy vertical layout

                Spacer()
            }
            .padding(40)
            }
            .frame(minWidth: 650)
            .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $showInstructions) {
            BridgeSetupInstructionsSheet()
                .environmentObject(bridge)
        }
    }

    private var bridgeStatusText: String {
        BridgeStatusText.text(for: bridge.status, platform: .macos)
    }

    // MARK: - Card builders for grid layout

    @ViewBuilder private var bridgeStatusCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Bridge Status")
                    .font(OAFonts.ui(.headline, 16))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Spacer()
                Button(action: { showInstructions = true }) {
                    HStack(spacing: 8) {
                        Image(systemName: "info.circle")
                        Text("View Setup Instructions")
                    }
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.accent)
                }
                .buttonStyle(.plain)
            }
            HStack(spacing: 16) {
                if case .advertising = bridge.status {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 20))
                        .foregroundStyle(OATheme.Colors.success)
                } else {
                    ProgressView().scaleEffect(0.8)
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text(bridgeStatusText)
                        .font(OAFonts.ui(.body, 15))
                        .fontWeight(.medium)
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    HStack(spacing: 8) {
                        if case .advertising(let port) = bridge.status {
                            Text("Port: \(String(port))")
                                .font(OAFonts.ui(.caption, 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                            if bridge.connectedClientCount > 0 {
                                Text("•").font(OAFonts.ui(.caption, 12)).foregroundStyle(OATheme.Colors.textTertiary)
                                Text("\(bridge.connectedClientCount) client\(bridge.connectedClientCount == 1 ? "" : "s") connected")
                                    .font(OAFonts.ui(.caption, 12))
                                    .foregroundStyle(OATheme.Colors.success)
                            }
                        }
                    }
                    if case .error(let msg) = bridge.status {
                        Text(msg).font(OAFonts.ui(.caption, 12)).foregroundStyle(OATheme.Colors.danger)
                    }
                }
            }
        }
        .padding(20)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(OATheme.Colors.border.opacity(0.3)))
    }

    @ViewBuilder private var workingDirectoryCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Working Directory")
                    .font(OAFonts.ui(.headline, 16))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Spacer()
            }
            if let dir = bridge.workingDirectory {
                HStack(spacing: 12) {
                    Image(systemName: "folder.fill").font(.system(size: 24)).foregroundStyle(OATheme.Colors.accent)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(dir.lastPathComponent)
                            .font(OAFonts.ui(.body, 14)).fontWeight(.medium).foregroundStyle(OATheme.Colors.textPrimary)
                        Text(dir.path)
                            .font(OAFonts.ui(.caption, 11)).foregroundStyle(OATheme.Colors.textTertiary)
                            .lineLimit(1).truncationMode(.middle)
                    }
                    Spacer()
                    Button("Change", action: selectWorkingDirectory).buttonStyle(.bordered)
                }
                .padding(16)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.black.opacity(0.15)))
            } else {
                VStack(spacing: 12) {
                    Image(systemName: "folder.badge.questionmark").font(.system(size: 32)).foregroundStyle(OATheme.Colors.textTertiary)
                    Text("No working directory selected").font(OAFonts.ui(.body, 14)).foregroundStyle(OATheme.Colors.textSecondary)
                    Button("Select Directory", action: selectWorkingDirectory).buttonStyle(.borderedProminent)
                }
                .padding(24)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(Color.black.opacity(0.15)))
            }
        }
    }

    @ViewBuilder private var configureAgentsCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Configure Coding Agents")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(OATheme.Colors.textSecondary)
            VStack(spacing: 8) {
                AgentToggleRow(name: "OpenAI Codex", detected: codexDetected, enabled: $codexEnabled, showStatus: true)
                AgentToggleRow(name: "Claude Code", detected: claudeDetected, enabled: $claudeEnabled, showStatus: true)
            }
            .padding(16)
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(OATheme.Colors.border.opacity(0.3)))
        }
    }

    @ViewBuilder private var tinyvexDevCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Tinyvex (Dev)")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(OATheme.Colors.textSecondary)
            HStack(spacing: 16) {
                Image(systemName: tinyvex.isRunning ? "checkmark.circle.fill" : "xmark.circle.fill")
                    .font(.system(size: 20))
                    .foregroundStyle(tinyvex.isRunning ? OATheme.Colors.success : OATheme.Colors.danger)
                VStack(alignment: .leading, spacing: 6) {
                    Text(tinyvex.isRunning ? "DB Connected" : "DB Stopped")
                        .font(OAFonts.ui(.body, 15))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Text("Path: \(tinyvex.dbPath)")
                        .font(OAFonts.ui(.caption, 11)).foregroundStyle(OATheme.Colors.textSecondary).lineLimit(2)
                    HStack(spacing: 12) {
                        Text("Tables: \(tinyvex.tableCount)")
                        Text("Rows: \(tinyvex.rowCount)")
                        Text(String(format: "Size: %.2f MB", Double(tinyvex.fileSizeBytes) / (1024.0*1024.0)))
                    }
                    .font(OAFonts.ui(.caption, 11)).foregroundStyle(OATheme.Colors.textTertiary)
                }
            }
            .padding(20)
            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(OATheme.Colors.border.opacity(0.3)))
        }
    }

    @ViewBuilder private var nostrDevCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Nostr (Dev)")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(OATheme.Colors.textSecondary)
            NostrKeygenCard()
                .padding(20)
                .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(OATheme.Colors.border.opacity(0.3)))
        }
    }

    @ViewBuilder private var morningBriefingDemoCard: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Morning Briefing Demo")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(OATheme.Colors.textSecondary)

            Text("Preview the overnight orchestration summary screen")
                .font(OAFonts.ui(.body, 14))
                .foregroundStyle(OATheme.Colors.textSecondary.opacity(0.7))

            NavigationLink(destination: MorningBriefingDemoView()) {
                Text("Open Demo")
                    .font(OAFonts.ui(.body, 14))
                    .foregroundColor(.white)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                    .background(OATheme.Colors.accent)
                    .cornerRadius(8)
            }
            .buttonStyle(.plain)
        }
        .padding(20)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous)
            .fill(OATheme.Colors.border.opacity(0.3)))
    }

    #if DEBUG
    @ViewBuilder private var nostrEventFeedCard: some View {
        NostrEventFeedView(relayManager: nostrRelayManager)
    }
    #endif

    private func selectWorkingDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.title = "Select Working Directory"
        panel.message = "Choose the directory where the agent will work"
        panel.prompt = "Select"

        if let currentDir = bridge.workingDirectory {
            panel.directoryURL = currentDir
        }

        if panel.runModal() == .OK, let url = panel.url {
            bridge.setWorkingDirectory(url)
        }
    }

    private func detectAgents() {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser

        // Detect Claude Code
        let claudeDir = home.appendingPathComponent(".claude")
        claudeDetected = fm.fileExists(atPath: claudeDir.path)

        // Detect Codex
        let codexDir = home.appendingPathComponent(".codex/sessions")
        codexDetected = fm.fileExists(atPath: codexDir.path)

        // If detected, enable by default
        if claudeDetected { claudeEnabled = true }
        if codexDetected { codexEnabled = true }
    }
}

// AgentToggleRow moved to Views/Components/AgentToggleRow.swift

#Preview {
    SimplifiedMacOSView()
        .environmentObject(BridgeManager())
}

#endif
