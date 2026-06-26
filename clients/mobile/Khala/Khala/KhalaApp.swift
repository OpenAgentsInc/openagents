import SwiftUI

/// Khala — a minimal push-to-talk voice client for the Khala API.
///
/// One screen: hold the button, speak, and Khala answers. See
/// `docs/mobile/2026-06-26-khala-voice-app-spec.md` for the full spec.
@main
struct KhalaApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
                .preferredColorScheme(.dark)
        }
    }
}
