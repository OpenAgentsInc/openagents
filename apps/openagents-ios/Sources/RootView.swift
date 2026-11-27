// Firm reason: 6 (iOS root container hosting the full-screen Coax/Metal GPU surface with pure-black native modal sheets for Account, Spark Wallet, Computers, Workspace, Subagent, Agent Hub, Usage, and Side Question).
import SwiftUI

struct RootView: View {
    @ObservedObject var store: AppStore
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @ObservedObject var wallet: MobileWalletController
    @ObservedObject var shell: MobileShellState

    var body: some View {
        GpuRootHostView(
            store: store,
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
        .sheet(isPresented: $store.isComputersPresented) {
            ComputersView(store: store)
                .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $store.isWorkspacePresented) {
            WorkspaceView(store: store)
                .preferredColorScheme(.dark)
        }
        .sheet(item: $store.activeSubagentItem) { sub in
            SubagentSheetView(store: store, subagent: sub)
                .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $store.isHubPresented) {
            AntigravityHubView(store: store)
                .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $store.isSideQuestionPresented) {
            SideQuestionSheetView(store: store)
                .preferredColorScheme(.dark)
        }
        .sheet(isPresented: $store.isUsagePresented) {
            UsageDashboardView(store: store)
                .preferredColorScheme(.dark)
        }
    }
}
