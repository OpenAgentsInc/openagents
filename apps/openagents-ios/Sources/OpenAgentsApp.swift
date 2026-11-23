// Firm reason: 6 (iOS SwiftUI @main application entry point for OpenAgents Antigravity-over-Tailscale mobile client).
import SwiftUI

@main
struct OpenAgentsApp: App {
    @StateObject private var store = AppStore()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView(store: store)
                .preferredColorScheme(.dark)
                .tint(.white)
                .onChange(of: scenePhase, initial: true) { _, phase in
                    switch phase {
                    case .active:
                        store.onForeground()
                    case .background:
                        store.onBackground()
                    default:
                        break
                    }
                }
        }
    }
}
