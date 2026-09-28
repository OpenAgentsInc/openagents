// The app's four tabs: Coder, Verse, Wallet, and Account. The tab bar shows
// icons only; each tab keeps a spoken name for VoiceOver.
import SwiftUI
import UIKit

enum AppTab: String, CaseIterable {
    case coder, verse, wallet, account

    var title: String {
        switch self {
        case .coder: "Coder"
        case .verse: "Verse"
        case .wallet: "Wallet"
        case .account: "Account"
        }
    }

    var symbol: String {
        switch self {
        case .coder: "chevron.left.forwardslash.chevron.right"
        case .verse: "globe"
        // `wallet.bifold` arrived in iOS 18.
        case .wallet: UIImage(systemName: "wallet.bifold") == nil ? "creditcard" : "wallet.bifold"
        case .account: "person.crop.circle"
        }
    }
}

/// A screen that the Account tab pushes.
enum AccountRoute: String, Hashable {
    case computers, tailnet, identity, device, changelog
}

/// Developer launch arguments that open a tab or an Account screen directly,
/// for example `--tab account --account-route tailnet`.
enum AppTabLaunch {
    static var tab: AppTab {
        #if DEBUG || targetEnvironment(simulator)
        if let value = argument("--tab"), let tab = AppTab(rawValue: value) { return tab }
        #endif
        return .coder
    }

    static var route: [AccountRoute] {
        #if DEBUG || targetEnvironment(simulator)
        if let value = argument("--account-route"), let route = AccountRoute(rawValue: value) {
            return [route]
        }
        #endif
        return []
    }

    private static func argument(_ name: String) -> String? {
        let arguments = ProcessInfo.processInfo.arguments
        guard let index = arguments.firstIndex(of: name), index + 1 < arguments.count else {
            return nil
        }
        return arguments[index + 1]
    }
}

struct AppTabs: View {
    @ObservedObject var bridge: MobileBridge
    @State private var tab = AppTabLaunch.tab

    var body: some View {
        TabView(selection: $tab) {
            CoderTab(bridge: bridge)
                .tabIcon(.coder)
            VerseTab(selected: tab == .verse)
                .tabIcon(.verse)
            ComingSoonScreen(title: "Wallet")
                .tabIcon(.wallet)
            AccountTab(bridge: bridge)
                .tabIcon(.account)
        }
    }
}

private extension View {
    func tabIcon(_ tab: AppTab) -> some View {
        tabItem {
            Image(systemName: tab.symbol)
                .accessibilityLabel(tab.title)
        }
        .tag(tab)
    }
}

/// A placeholder for a tab that is not built yet.
struct ComingSoonScreen: View {
    let title: String

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            VStack(spacing: 8) {
                Text(title).font(.largeTitle.bold()).foregroundStyle(.white)
                Text("Coming soon.").foregroundStyle(.gray)
            }
        }
    }
}

/// Settings and the screens that used to be tabs.
struct AccountTab: View {
    @ObservedObject var bridge: MobileBridge
    @State private var path = AppTabLaunch.route

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    NavigationLink("Computers", value: AccountRoute.computers)
                    NavigationLink("Tailnet", value: AccountRoute.tailnet)
                }
                Section {
                    NavigationLink("Identity keys", value: AccountRoute.identity)
                    NavigationLink("About this device", value: AccountRoute.device)
                    NavigationLink("Changelog", value: AccountRoute.changelog)
                }
                Section {
                    ExternalLink(title: "Source code", symbol: "chevron.left.forwardslash.chevron.right",
                                 url: "https://github.com/OpenAgentsInc/openagents")
                    ExternalLink(title: "Follow us on X", symbol: "at", url: "https://x.com/OpenAgentsInc")
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .background(Color.black.ignoresSafeArea())
            .navigationTitle("Account")
            .navigationDestination(for: AccountRoute.self) { route in
                destination(route)
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbarBackground(Color.black, for: .navigationBar)
            }
        }
    }

    @ViewBuilder private func destination(_ route: AccountRoute) -> some View {
        switch route {
        case .computers: ComputersTab(bridge: bridge).navigationTitle("") // The screen draws its own heading.
        case .tailnet: TailnetTab(bridge: bridge).navigationTitle("") // The screen draws its own heading.
        case .identity: IdentityKeysScreen(bridge: bridge).navigationTitle("Identity keys")
        case .device: AboutDeviceScreen(bridge: bridge).navigationTitle("About this device")
        case .changelog: ChangelogScreen(bridge: bridge).navigationTitle("Changelog")
        }
    }
}

/// A row that opens a web page in the browser.
private struct ExternalLink: View {
    let title: String
    let symbol: String
    let url: String

    var body: some View {
        if let destination = URL(string: url) {
            Link(destination: destination) {
                HStack {
                    Label(title, systemImage: symbol)
                    Spacer()
                    Image(systemName: "arrow.up.right").font(.footnote).foregroundStyle(.secondary)
                }
            }
            .foregroundStyle(.white)
        }
    }
}
