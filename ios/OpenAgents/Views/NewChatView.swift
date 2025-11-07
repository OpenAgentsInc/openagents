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
            VStack {
                Spacer()
                Text("New Chat")
                    .font(.system(size: 24, weight: .regular))
                    .foregroundStyle(.secondary)
                Spacer()
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
        .background(.black)
        .navigationTitle("")
        .navigationBarBackButtonHidden(true)
        .toolbarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button(action: { isMenuPresented.toggle() }) {
                    Image(systemName: "line.3.horizontal")
                }
            }

            ToolbarItem(placement: .principal) {
                Text(selectedAgent)
                    .font(.system(size: 16, weight: .semibold))
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
