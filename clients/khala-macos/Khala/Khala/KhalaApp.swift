import SwiftUI

@main
struct KhalaApp: App {
    @StateObject private var store = ConversationStore()
    @StateObject private var pylonSupervisor = PylonSupervisor()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView(store: store, pylonSupervisor: pylonSupervisor)
                .frame(minWidth: 1060, minHeight: 720)
                .task { await pylonSupervisor.start() }
                .onChange(of: scenePhase) { _, phase in
                    if phase == .background { pylonSupervisor.stop() }
                }
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
