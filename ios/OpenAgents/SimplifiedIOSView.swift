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
        BridgeStatusText.text(for: bridge.status, platform: .ios)
    }
}

// MARK: - Agent Info Row Component

// AgentInfoRow moved to Views/Components/AgentInfoRow.swift

// MARK: - Drawer Menu View

// DrawerMenuView moved to Views/Components/DrawerMenuView.swift

#Preview {
    SimplifiedNavigationView()
        .environmentObject(BridgeManager())
}

#endif
