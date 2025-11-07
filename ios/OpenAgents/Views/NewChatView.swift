import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @EnvironmentObject var bridge: BridgeManager
    @Binding var isMenuPresented: Bool
    @Binding var selectedAgent: String
    var detectedAgents: [String]
    var onNavigateToSetup: () -> Void

    @State private var messageText: String = ""
    @State private var showAgentPicker = false
    @State private var showMoreMenu = false

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
                        // TODO: Start new chat
                        print("[NewChat] New chat button tapped")
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

            // Main content area
            VStack {
                Spacer()

                Text("New Chat")
                    .font(OAFonts.ui(.title, 24))
                    .foregroundStyle(OATheme.Colors.textSecondary)

                Spacer()
            }
            .frame(maxWidth: .infinity)

            // Composer at bottom
            HStack(alignment: .bottom, spacing: 12) {
                Composer(
                    text: $messageText,
                    agentName: selectedAgent,
                    onSubmit: {
                        sendMessage()
                    }
                )

                Button(action: {
                    sendMessage()
                }) {
                    Image(systemName: "arrow.up.circle.fill")
                        .font(.system(size: 32))
                        .foregroundStyle(
                            messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                ? OATheme.Colors.textTertiary
                                : OATheme.Colors.accent
                        )
                }
                .disabled(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .padding(.bottom, 6)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(OATheme.Colors.background)
        }
        .background(OATheme.Colors.background)
        .navigationBarHidden(true)
        .preferredColorScheme(.dark)
        .confirmationDialog("Select Agent", isPresented: $showAgentPicker, titleVisibility: .visible) {
            ForEach(detectedAgents, id: \.self) { agent in
                Button(agent) {
                    selectedAgent = agent
                }
            }
        }
        .confirmationDialog("Options", isPresented: $showMoreMenu, titleVisibility: .hidden) {
            Button("Delete", role: .destructive) {
                print("[NewChat] Delete tapped")
            }
            Button("Share") {
                print("[NewChat] Share tapped")
            }
            Button("Cancel", role: .cancel) {}
        }
    }

    private func sendMessage() {
        let message = messageText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else { return }

        // TODO: Send message to selected agent
        print("[NewChat] Sending to \(selectedAgent): \(message)")

        // Clear input
        messageText = ""
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
