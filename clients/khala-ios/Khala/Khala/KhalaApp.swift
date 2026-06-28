import SwiftUI

/// Khala — a ChatGPT-style mobile client for the Khala API with a retained
/// push-to-talk voice visualization.
///
/// The app shell (`RootView`) hosts the main chat `NavigationStack` inside a
/// left slide-over drawer. Conversation history persists locally via SwiftData
/// (`ConversationStore`). See
/// `docs/mobile/2026-06-26-khala-chatgpt-style-app-spec.md`.
@main
struct KhalaApp: App {
    var body: some Scene {
        WindowGroup {
            if ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil {
                Text("Khala Tests")
            } else {
                KhalaRootScene()
            }
        }
    }
}

private struct KhalaRootScene: View {
    @StateObject private var store = ConversationStore()

    var body: some View {
        RootView(store: store)
            .preferredColorScheme(.dark)
    }
}
