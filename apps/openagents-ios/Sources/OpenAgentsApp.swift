// Firm reason: 6 (iOS SwiftUI @main application entry point for OpenAgents: Metal GPU surface, Account/Passkey, Cloud Chat sync, Spark Wallet, and Tailscale).
import SwiftUI
import UIKit

@MainActor
final class MobileShellState: ObservableObject {
    @Published var isAccountPresented: Bool = false
    @Published var isWalletPresented: Bool = false
    @Published var preferCloudMode: Bool = false
}

@main
struct OpenAgentsApp: App {
    @StateObject private var store = AppStore()
    @StateObject private var account = MobileAccountController.shared
    @StateObject private var cloudChat = MobileCloudChat.shared
    @StateObject private var wallet = MobileWalletController.shared
    @StateObject private var shell = MobileShellState()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView(
                store: store,
                account: account,
                cloudChat: cloudChat,
                wallet: wallet,
                shell: shell
            )
            .preferredColorScheme(.dark)
            .tint(.white)
            .onAppear {
                store.onTurnNotificationEvent = { kind, title in
                    TurnNotifications.shared.notifyBackgroundEvent(kind: kind, title: title)
                }
                account.onAuthChanged = { loggedIn in
                    if loggedIn {
                        cloudChat.start()
                    } else {
                        cloudChat.stop()
                        shell.preferCloudMode = false
                    }
                }
                if scenePhase != .background {
                    store.onForeground()
                    Task {
                        await account.refresh()
                        if account.isLoggedIn {
                            cloudChat.start()
                        }
                    }
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.willEnterForegroundNotification)) { _ in
                TurnNotifications.shared.endBackgroundGracePeriod()
                store.onForeground()
                if account.isLoggedIn { cloudChat.start() }
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
                    if account.isLoggedIn { cloudChat.start() }
                case .background:
                    let active = store.activeSession?.busy == true || store.isAwaitingTurnStart || cloudChat.isSending
                    TurnNotifications.shared.beginBackgroundGracePeriod(activeTurnRunning: active)
                    store.onBackground()
                default:
                    break
                }
            }
        }
    }
}
