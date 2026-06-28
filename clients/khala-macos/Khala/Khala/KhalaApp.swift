import SwiftUI

@main
struct KhalaApp: App {
    @StateObject private var store = ConversationStore()

    var body: some Scene {
        WindowGroup {
            RootView(store: store)
                .frame(minWidth: 1060, minHeight: 720)
        }
        .windowStyle(.titleBar)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Chat") { store.createConversation() }
                    .keyboardShortcut("n")
            }
        }
    }
}
