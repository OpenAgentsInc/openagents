import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @EnvironmentObject var bridge: BridgeManager
    @Binding var isMenuPresented: Bool
    @Binding var selectedAgent: String
    var detectedAgents: [String]
    var onNavigateToSetup: () -> Void

    @State private var messageText: String = ""

    var body: some View {
        VStack(spacing: 0) {
            // Main content area
            ScrollView {
                VStack {
                    Spacer()

                    Text("New Chat")
                        .font(OAFonts.ui(.title, 24))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    Spacer()
                }
                .frame(maxWidth: .infinity, minHeight: 400)
            }

            // Composer at bottom
            Composer(
                text: $messageText,
                agentName: selectedAgent,
                onSubmit: {
                    sendMessage()
                }
            )
        }
        .background(OATheme.Colors.background)
        .navigationTitle("")
        .navigationBarBackButtonHidden(true)
        .toolbarTitleDisplayMode(.inline)
        .toolbar {
            // LEFT: Hamburger menu and agent selector (separated with spacer)
            ToolbarItem(placement: .topBarLeading) {
                HStack(spacing: 0) {
                    // Hamburger menu with glass effect
                    Button(action: { isMenuPresented.toggle() }) {
                        Image(systemName: "line.3.horizontal")
                            .foregroundStyle(OATheme.Colors.textPrimary)
                            .frame(width: 44, height: 44)
                    }

                    // Visual spacer
                    Spacer()
                        .frame(width: 16)

                    // Agent selector - plain text button
                    Button(action: {}) {
                        HStack(spacing: 4) {
                            Text(selectedAgent)
                                .font(.system(size: 16, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.Colors.textPrimary)
                            Image(systemName: "chevron.down")
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                        }
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                        .background(
                            RoundedRectangle(cornerRadius: 8, style: .continuous)
                                .fill(OATheme.Colors.border.opacity(0.3))
                        )
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        ForEach(detectedAgents, id: \.self) { agent in
                            Button(action: {
                                selectedAgent = agent
                            }) {
                                HStack {
                                    Text(agent)
                                    if selectedAgent == agent {
                                        Image(systemName: "checkmark")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        .ignoresSafeArea(.keyboard, edges: .bottom)
        .preferredColorScheme(.dark)
    }

    private func sendMessage() {
        let message = messageText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !message.isEmpty else { return }

        // TODO: Send message to selected agent
        print("[NewChat] Sending to \(selectedAgent): \(message)")

        // Clear input
        messageText = ""
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
