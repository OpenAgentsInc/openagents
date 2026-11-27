// Firm reason: 6 (iOS root container enforcing a strict full-screen Login Gate until signed in, then presenting the full-screen Coax/Metal GPU Cloud workspace with pure-black native modal sheets).
import SwiftUI

struct RootView: View {
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @ObservedObject var wallet: MobileWalletController
    @ObservedObject var shell: MobileShellState

    var body: some View {
        Group {
            if account.isLoggedIn {
                GpuRootHostView(
                    account: account,
                    cloudChat: cloudChat,
                    wallet: wallet,
                    shell: shell
                )
                .ignoresSafeArea()
                .background(Color.black.ignoresSafeArea())
                .sheet(isPresented: $shell.isAccountPresented) {
                    AccountView(account: account, cloudChat: cloudChat)
                        .preferredColorScheme(.dark)
                }
                .sheet(isPresented: $shell.isWalletPresented) {
                    WalletView(wallet: wallet)
                        .preferredColorScheme(.dark)
                }
                .sheet(isPresented: $shell.isEnvironmentsPresented) {
                    EnvironmentsView(account: account, cloudChat: cloudChat)
                        .preferredColorScheme(.dark)
                }
                .sheet(isPresented: $shell.isUsagePresented) {
                    UsageDashboardView(account: account, cloudChat: cloudChat)
                        .preferredColorScheme(.dark)
                }
            } else {
                LoginGateView(account: account)
            }
        }
        .background(Color.black.ignoresSafeArea())
        .animation(.easeInOut(duration: 0.2), value: account.isLoggedIn)
    }
}
