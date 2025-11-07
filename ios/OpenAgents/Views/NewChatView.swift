import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @EnvironmentObject var bridge: BridgeManager
    @Binding var isMenuPresented: Bool
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
            .toolbarTitleDisplayMode(.inline)
            .toolbar {
                ChatHeaderToolbar(
                    title: "New Chat",
                    onToggleMenu: { isMenuPresented.toggle() },
                    onNewChat: { /* no-op for now */ }
                )
            }
        .preferredColorScheme(.dark)
    }
}

#Preview {
    NavigationStack {
        NewChatView(
            isMenuPresented: .constant(false),
            onNavigateToSetup: {}
        )
        .environmentObject(BridgeManager())
    }
}

#endif
