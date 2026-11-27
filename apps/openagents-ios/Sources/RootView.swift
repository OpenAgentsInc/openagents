// Firm reason: 6 (iOS root container hosting the full-screen Coax/Metal GPU surface with pure-black native modal sheets for Account, Spark Wallet, Environments & Projects, and Usage).
import SwiftUI

struct RootView: View {
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @ObservedObject var wallet: MobileWalletController
    @ObservedObject var shell: MobileShellState

    var body: some View {
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
    }
}
