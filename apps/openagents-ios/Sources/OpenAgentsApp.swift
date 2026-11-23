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
                .onAppear {
                    store.onTurnNotificationEvent = { kind, title in
                        TurnNotifications.shared.notifyBackgroundEvent(kind: kind, title: title)
                    }
                }
                .onOpenURL { url in
                    store.handleIncomingURL(url)
                }
                .onChange(of: scenePhase, initial: true) { _, phase in
                    switch phase {
                    case .active:
                        TurnNotifications.shared.endBackgroundGracePeriod()
                        store.onForeground()
                    case .background:
                        let active = store.activeSession?.busy == true || store.isAwaitingTurnStart
                        TurnNotifications.shared.beginBackgroundGracePeriod(activeTurnRunning: active)
                        store.onBackground()
                    default:
                        break
                    }
                }
        }
    }
}
