import SwiftUI

#if os(macOS)
import AppKit

struct SimplifiedMacOSView: View {
    @EnvironmentObject var bridge: BridgeManager
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
            OATheme.Colors.background.ignoresSafeArea()

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
                        Label("Bridge Status", systemImage: "network")
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
                }
                .frame(maxWidth: 500)

                Divider()
                    .background(OATheme.Colors.border)
                    .frame(maxWidth: 500)

                // Working Directory Section
                VStack(alignment: .leading, spacing: 12) {
                    HStack {
                        Label("Working Directory", systemImage: "folder")
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
                        Label("Configure Coding Agents", systemImage: "terminal")
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

                Spacer()
            }
            .padding(40)
        }
        .frame(minWidth: 500)
        .sheet(isPresented: $showInstructions) {
            BridgeSetupInstructionsSheet()
                .environmentObject(bridge)
        }
        .preferredColorScheme(.dark)
    }

    private var bridgeStatusText: String {
        switch bridge.status {
        case .idle: return "Idle"
        case .advertising: return "Ready for Connections"
        case .discovering: return "Discovering..."
        case .connecting(let h, let p): return "Connecting to \(h):\(p)"
        case .handshaking(let h, let p): return "Handshaking with \(h):\(p)"
        case .connected(let h, let p): return "Connected to \(h):\(p)"
        case .error: return "Error"
        }
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

// MARK: - Agent Toggle Row Component

struct AgentToggleRow: View {
    let name: String
    let detected: Bool
    @Binding var enabled: Bool
    var showStatus: Bool = true

    private var isInteractive: Bool {
        detected || name == "OpenAgents Coder"
    }

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                if showStatus {
                    Text(detected ? "Detected" : "Not found")
                        .font(OAFonts.ui(.caption, 11))
                        .foregroundStyle(detected ? OATheme.Colors.success : OATheme.Colors.textTertiary)
                }
            }

            Spacer()

            Toggle("", isOn: $enabled)
                .labelsHidden()
                .disabled(!isInteractive)
                .tint(OATheme.Colors.success)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(enabled ? OATheme.Colors.border.opacity(0.5) : OATheme.Colors.card)
        )
        .contentShape(Rectangle())
        .onTapGesture {
            if isInteractive {
                enabled.toggle()
            }
        }
        .opacity(isInteractive ? 1.0 : 0.5)
    }
}

#Preview {
    SimplifiedMacOSView()
        .environmentObject(BridgeManager())
}

#endif
