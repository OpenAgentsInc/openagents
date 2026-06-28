import SwiftUI

@main
struct KhalaApp: App {
    @StateObject private var store = ConversationStore()

    var body: some Scene {
        WindowGroup {
            DesktopRootView(store: store)
                .preferredColorScheme(.dark)
                .frame(minWidth: 980, minHeight: 680)
        }
        .windowStyle(.titleBar)
        .windowToolbarStyle(.unified)

        Settings {
            SettingsView()
                .preferredColorScheme(.dark)
        }
    }
}
