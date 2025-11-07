import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @Binding var isMenuPresented: Bool
    @Binding var selectedAgent: String
    var detectedAgents: [String]
    var onNavigateToSetup: () -> Void

    @State private var messageText: String = ""

    var body: some View {
        VStack(spacing: 0) {
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
            HStack(spacing: 12) {
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
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(OATheme.Colors.background)
        }
        .background(OATheme.Colors.background)
        .navigationTitle("")
        .navigationBarBackButtonHidden(true)
        .toolbarTitleDisplayMode(.inline)
        .toolbar {
            // LEFT: Hamburger menu and agent selector flush together
            ToolbarItem(placement: .topBarLeading) {
                HStack(spacing: 16) {
                    // Bare hamburger menu icon - no glass
                    Button(action: {
                        withAnimation(.easeInOut(duration: 0.3)) {
                            isMenuPresented.toggle()
                        }
                    }) {
                        Image(systemName: "line.3.horizontal")
                            .foregroundStyle(OATheme.Colors.textPrimary)
                            .font(.system(size: 18))
                    }
                    .buttonStyle(.plain)

                    // Agent selector dropdown - flush left
                    Menu {
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
                    } label: {
                        HStack(spacing: 4) {
                            Text(selectedAgent)
                                .font(OAFonts.ui(.headline, 16))
                                .foregroundStyle(OATheme.Colors.textPrimary)
                            Image(systemName: "chevron.down")
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
        }
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
