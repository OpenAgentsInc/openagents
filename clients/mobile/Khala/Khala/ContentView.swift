import SwiftUI

/// Compatibility entry point. The app now boots through `KhalaApp` -> `RootView`
/// (the ChatGPT-style shell). `ContentView` remains as a thin wrapper so any
/// preview/tooling that referenced it keeps working; it builds its own
/// in-memory store so it can render standalone.
struct ContentView: View {
    @StateObject private var store = ConversationStore(inMemory: true)

    var body: some View {
        RootView(store: store)
    }
}

#Preview {
    ContentView().preferredColorScheme(.dark)
}
