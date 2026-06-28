import SwiftUI

@main
struct KhalaApp: App {
    @StateObject private var store = ConversationStore()
    @State private var isShowingSettings = false

    var body: some Scene {
        WindowGroup {
            RootView(store: store, isShowingSettings: $isShowingSettings)
                .frame(minWidth: 1060, minHeight: 720)
        }
        .defaultSize(width: 1280, height: 820)
        .windowResizability(.contentMinSize)
        .windowStyle(.titleBar)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New Chat") { store.createConversation() }
                    .keyboardShortcut("n")
            }
            CommandGroup(replacing: .appSettings) {
                Button("Settings...") { isShowingSettings = true }
                    .keyboardShortcut(",", modifiers: [.command])
            }
        }
    }
}
