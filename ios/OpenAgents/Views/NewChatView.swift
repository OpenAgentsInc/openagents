import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @EnvironmentObject var bridge: BridgeManager
    @Binding var isMenuPresented: Bool
    @Binding var selectedAgent: String
    var detectedAgents: [String]
    var onNavigateToSetup: () -> Void

    var body: some View {
        VStack {
            Spacer()

            Text("New Chat")
                .font(OAFonts.ui(.title, 24))
                .foregroundStyle(OATheme.Colors.textSecondary)

            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(OATheme.Colors.background)
        .navigationTitle("")
        .navigationBarBackButtonHidden(true)
        .toolbarTitleDisplayMode(.inline)
        .toolbar {
            // LEFT: Hamburger menu
            ToolbarItem(placement: .topBarLeading) {
                Button(action: { isMenuPresented.toggle() }) {
                    Image(systemName: "line.3.horizontal")
                        .foregroundStyle(OATheme.Colors.textPrimary)
                }
            }

            // CENTER-LEFT: Agent selector (standalone, no glass effect)
            ToolbarItem(placement: .principal) {
                HStack(spacing: 0) {
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
                        .padding(.horizontal, 12)
                        .padding(.vertical, 6)
                    }
                    .buttonStyle(.plain)

                    Spacer()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .preferredColorScheme(.dark)
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
