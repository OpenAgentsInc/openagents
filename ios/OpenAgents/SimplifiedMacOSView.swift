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

    var body: some View {
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

                // Bridge Status Section
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
                #endif

                Spacer()
            }
            .padding(40)
        }
        .frame(minWidth: 650)
        .sheet(isPresented: $showInstructions) {
            BridgeSetupInstructionsSheet()
                .environmentObject(bridge)
        }
        .preferredColorScheme(.dark)
    }

    private var bridgeStatusText: String {
        BridgeStatusText.text(for: bridge.status, platform: .macos)
    }

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
