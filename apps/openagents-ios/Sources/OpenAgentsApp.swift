// Firm reason: 6 (iOS SwiftUI @main application entry point for OpenAgents Antigravity-over-Tailscale mobile client).
import SwiftUI
import UIKit

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
                    if scenePhase != .background {
                        store.onForeground()
                    }
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                    TurnNotifications.shared.endBackgroundGracePeriod()
                    store.onForeground()
                }
                .onReceive(NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)) { _ in
                    TurnNotifications.shared.endBackgroundGracePeriod()
                    store.onForeground()
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
