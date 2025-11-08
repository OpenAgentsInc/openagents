import SwiftUI
import OpenAgentsCore
import OSLog

#if os(iOS)
import UIKit

struct NewChatView: View {
    @EnvironmentObject var bridge: BridgeManager
    @Binding var isMenuPresented: Bool
    @Binding var selectedAgent: String
    var detectedAgents: [String]
    var onNavigateToSetup: () -> Void

    @State private var messageText: String = ""
    @State private var composerFocused: Bool = false
    @State private var showAgentPicker = false
    @State private var showMoreMenu = false
    @StateObject private var timelineVM = ACPTimelineViewModel()

    var body: some View {
        ZStack(alignment: .bottom) {
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

                    // Agent selector - tap to show picker
                    Button(action: {
                        showAgentPicker = true
                    }) {
                        HStack(spacing: 4) {
                            Text(selectedAgent)
                                .font(OAFonts.ui(.headline, 16))
                                .foregroundStyle(.white)
                                .fixedSize()
                            Image(systemName: "chevron.down")
                                .font(.system(size: 12))
                                .foregroundStyle(.white.opacity(0.6))
                        }
                        .fixedSize()
                    }
                    .buttonStyle(.plain)

                    Spacer()

                    // New chat icon
                    Button(action: {
                        // Start a new session and wipe local timeline/cache
                        let mode: ACPSessionModeId? = {
                            let lower = selectedAgent.lowercased()
                            if lower.contains("codex") { return .codex }
                            if lower.contains("claude") { return .claude_code }
                            return nil
                        }()
                        bridge.startNewSession(desiredMode: mode)
                        composerFocused = true
                    }) {
                        Image(systemName: "square.and.pencil")
                            .foregroundStyle(.white)
                            .font(.system(size: 18))
                    }
                    .buttonStyle(.plain)

                    // More options menu (ellipsis)
                    Button(action: {
                        showMoreMenu = true
                    }) {
                        Image(systemName: "ellipsis")
                            .foregroundStyle(.white)
                            .font(.system(size: 18))
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(.black)
            }

            // Connection info at top
            VStack(alignment: .leading, spacing: 8) {
                // Bridge status
                HStack(spacing: 8) {
                    Circle()
                        .fill(bridgeStatusColor)
                        .frame(width: 8, height: 8)
                    Text(bridgeStatusText)
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }

                // Session indicator
                HStack(spacing: 8) {
                    Image(systemName: "number")
                        .font(.system(size: 10))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                    if let sid = bridge.currentSessionId?.value, !sid.isEmpty {
                        Text("Session: \(sid.prefix(8))…")
                            .font(OAFonts.ui(.caption, 12))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Button(action: { UIPasteboard.general.string = sid }) {
                            Image(systemName: "doc.on.doc")
                                .font(.system(size: 11))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel("Copy session ID")
                    } else {
                        Text("Session: No session")
                            .font(OAFonts.ui(.caption, 12))
                            .foregroundStyle(OATheme.Colors.textTertiary)
                    }
                }

                // Working directory
                if let workingDir = bridge.workingDirectory {
                    HStack(spacing: 8) {
                        Image(systemName: "folder")
                            .font(.system(size: 10))
                            .foregroundStyle(OATheme.Colors.textTertiary)
                        Text(URL(fileURLWithPath: workingDir).lastPathComponent)
                            .font(OAFonts.ui(.caption, 12))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                }

                // Enabled agents
                HStack(spacing: 8) {
                    Image(systemName: "terminal")
                        .font(.system(size: 10))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                    Text(detectedAgents.joined(separator: ", "))
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(OATheme.Colors.border.opacity(0.2))

            // ACP timeline area - tap to dismiss keyboard anywhere above the composer
            ACPTimelineView(items: timelineVM.items)
                .contentShape(Rectangle())
                .onTapGesture { dismissKeyboard() }
        }
        // Also capture taps anywhere in the background to dismiss the keyboard
        .contentShape(Rectangle())
        .simultaneousGesture(TapGesture().onEnded { dismissKeyboard() })
        // Bottom composer overlay pinned to the view's bottom
        HStack(alignment: .center, spacing: 12) {
            Composer(
                text: $messageText,
                isFocused: $composerFocused,
                agentName: selectedAgent,
                onSubmit: { sendMessage() }
            )
            .frame(maxWidth: .infinity, minHeight: 40, maxHeight: 40)
            .layoutPriority(1)

                Button(action: { sendMessage() }) {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 32))
                        .foregroundStyle(
                            messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                ? OATheme.Colors.textTertiary
                                : OATheme.Colors.accent
                        )
                }
                .disabled(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(OATheme.Colors.background)
        }
        .background(OATheme.Colors.background)
        .navigationBarHidden(true)
        .preferredColorScheme(.dark)
        .onAppear { timelineVM.attach(bridge: bridge) }
        .confirmationDialog("Select Agent", isPresented: $showAgentPicker, titleVisibility: .visible) {
            ForEach(detectedAgents, id: \.self) { agent in
                Button(agent) {
                    // Determine new mode from selection
                    let lower = agent.lowercased()
                    let newMode: ACPSessionModeId? = lower.contains("codex") ? .codex : (lower.contains("claude") ? .claude_code : nil)

                    // Update local UI selection
                    selectedAgent = agent

                    // If switching provider while in an active chat that already has messages,
                    // start a brand new chat bound to the newly selected provider.
                    if bridge.currentSessionId != nil, !timelineVM.items.isEmpty {
                        bridge.startNewSession(desiredMode: newMode)
                        messageText = ""
                        composerFocused = true
                    } else if let mode = newMode, bridge.currentSessionId != nil, timelineVM.items.isEmpty {
                        // Reuse empty chat: set mode immediately so the first send uses the chosen provider
                        bridge.setSessionMode(mode)
                    }
                }
            }
        }
        .confirmationDialog("Options", isPresented: $showMoreMenu, titleVisibility: .hidden) {
            Button("Delete", role: .destructive) {
                OpenAgentsLog.ui.debug("NewChat Delete tapped")
            }
            Button("Share") {
                OpenAgentsLog.ui.debug("NewChat Share tapped")
            }
            Button("Cancel", role: .cancel) {}
        }
    }

    private func sendMessage() {
        let message = messageText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else { return }

        // Send via bridge (creates a session on first send, then reuses it)
        let mode: ACPSessionModeId? = {
            let lower = selectedAgent.lowercased()
            if lower.contains("codex") { return .codex }
            if lower.contains("claude") { return .claude_code }
            return nil
        }()
        bridge.sendPrompt(text: message, mode: mode)

        // Clear input
        messageText = ""
    }

    private func dismissKeyboard() {
        composerFocused = false
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }

    private var bridgeStatusText: String {
        switch bridge.status {
        case .connected(let host, let port):
            return "Connected to \(host):\(port)"
        case .connecting(let host, let port):
            return "Connecting to \(host):\(port)"
        case .handshaking(let host, let port):
            return "Handshaking with \(host):\(port)"
        case .discovering:
            return "Discovering desktop..."
        case .advertising(let port):
            return "Advertising on :\(port)"
        case .idle:
            return "Idle"
        case .error(let msg):
            return "Error: \(msg)"
        }
    }

    private var bridgeStatusColor: Color {
        switch bridge.status {
        case .connected:
            return OATheme.Colors.success
        case .connecting, .handshaking, .discovering, .advertising:
            return OATheme.Colors.textSecondary
        case .idle:
            return OATheme.Colors.textTertiary
        case .error:
            return OATheme.Colors.danger
        }
    }
}

#Preview {
    NavigationStack {
        NewChatView(
            isMenuPresented: .constant(false),
            selectedAgent: .constant("Codex"),
            detectedAgents: ["Codex", "Claude Code"],
            onNavigateToSetup: {}
        )
        .environmentObject(BridgeManager())
    }
}

#endif
