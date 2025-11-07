import SwiftUI

#if os(iOS)

struct NewChatView: View {
    @EnvironmentObject var bridge: BridgeManager
    @State private var isMenuPresented = false

    var body: some View {
        NavigationStack {
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
            .sheet(isPresented: $isMenuPresented) {
                NavigationMenuSheet(isPresented: $isMenuPresented)
            }
        }
        .preferredColorScheme(.dark)
    }
}

#Preview {
    NewChatView()
        .environmentObject(BridgeManager())
}

#endif
