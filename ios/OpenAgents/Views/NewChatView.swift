import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @Binding var isMenuPresented: Bool
    @Binding var selectedAgent: String
    var detectedAgents: [String]
    var onNavigateToSetup: () -> Void

    @State private var messageText: String = ""

    var body: some View {
        VStack {
            Spacer()
            Text("New Chat")
            Spacer()

            Composer(
                text: $messageText,
                agentName: selectedAgent,
                onSubmit: { messageText = "" }
            )
        }
        .background(.black)
        .navigationTitle("")
        .navigationBarBackButtonHidden(true)
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button(action: { isMenuPresented.toggle() }) {
                    Image(systemName: "line.3.horizontal")
                }
            }

            ToolbarItem(placement: .principal) {
                Text(selectedAgent)
            }
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
