// Firm reason: 6 (iOS SwiftUI @main application entry point for OpenAgents: 100% OpenAgents Cloud mode over the Metal GPU surface, Account/Passkey, Spark Wallet, and Voice).
import SwiftUI
import UIKit

@MainActor
final class MobileShellState: ObservableObject {
    @Published var isAccountPresented: Bool = false
    @Published var isWalletPresented: Bool = false
    @Published var isEnvironmentsPresented: Bool = false
    @Published var isUsagePresented: Bool = false
}

@main
struct OpenAgentsApp: App {
    @StateObject private var account = MobileAccountController.shared
    @StateObject private var cloudChat = MobileCloudChat.shared
    @StateObject private var wallet = MobileWalletController.shared
    @StateObject private var shell = MobileShellState()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootView(
                account: account,
                cloudChat: cloudChat,
                wallet: wallet,
                shell: shell
            )
            .preferredColorScheme(.dark)
            .tint(.white)
            .onAppear {
                cloudChat.onTurnNotificationEvent = { kind, title in
                    TurnNotifications.shared.notifyBackgroundEvent(kind: kind, title: title)
                }
                account.onAuthChanged = { loggedIn in
                    if loggedIn {
                        cloudChat.start()
                    } else {
                        shell.isAccountPresented = false
                        shell.isWalletPresented = false
                        shell.isEnvironmentsPresented = false
                        shell.isUsagePresented = false
                        cloudChat.clearOnSignOut()
                    }
                }
                #if DEBUG
                if UserDefaults.standard.bool(forKey: "OADemoPreview") {
                    account.enableDebugDemoSignedIn()
                }
                #endif
                if scenePhase != .background {
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
                if account.isLoggedIn { cloudChat.start() }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification)) { _ in
                TurnNotifications.shared.endBackgroundGracePeriod()
            }
            .onOpenURL { url in
                cloudChat.handleIncomingURL(url)
            }
            .onChange(of: scenePhase, initial: true) { _, phase in
                switch phase {
                case .active:
                    TurnNotifications.shared.endBackgroundGracePeriod()
                    if account.isLoggedIn { cloudChat.start() }
                case .background:
                    TurnNotifications.shared.beginBackgroundGracePeriod(activeTurnRunning: cloudChat.isSending)
                default:
                    break
                }
            }
        }
    }
}
