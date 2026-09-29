// The app's four tabs: Chat, Verse, Wallet, and Account. The tab bar shows
// icons only; each tab keeps a spoken name for VoiceOver.
import SwiftUI
import UIKit

enum AppTab: String, CaseIterable {
    case coder, verse, wallet, account

    var title: String {
        switch self {
        case .coder: "Chat"
        case .verse: "Verse"
        case .wallet: "Wallet"
        case .account: "Account"
        }
    }

    var symbol: String {
        switch self {
        case .coder: "bubble.left.and.bubble.right"
        case .verse: "globe"
        // `wallet.bifold` arrived in iOS 18.
        case .wallet: UIImage(systemName: "wallet.bifold") == nil ? "creditcard" : "wallet.bifold"
        case .account: "person.crop.circle"
        }
    }
}

/// A screen that the Account tab pushes.
enum AccountRoute: String, Hashable {
    case trainer, computers, tailnet, identity, device, changelog, playtest, reports
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

    /// Wallet screenshots: `--wallet-section send`, `--wallet-method spark`,
    /// `--wallet-send TEXT` (reviewed once the wallet runs), and
    /// `--wallet-invoice AMOUNT` (made once the wallet runs), `--amount-format btc`
    /// (the amount format, saved), and `--wallet-info 1`
    /// (the trust note).
    static func wallet(_ name: String) -> String? {
        #if DEBUG || targetEnvironment(simulator)
        return argument(name)
        #else
        return nil
        #endif
    }

    /// `--xp-preview`: levels in the Grid and on the trainer card come from
    /// the labeled tutorial fixture, offline, instead of the relay.
    static var xpPreview: Bool {
        #if DEBUG || targetEnvironment(simulator)
        return ProcessInfo.processInfo.arguments.contains("--xp-preview")
        #else
        return false
        #endif
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
    @StateObject private var place = PlaytestPlace()
    @StateObject private var reporter = ReportCoordinator()

    var body: some View {
        TabView(selection: $tab) {
            CoderTab(bridge: bridge)
                .tabIcon(.coder)
            VerseTab(selected: tab == .verse)
                .tabIcon(.verse)
            WalletTab(bridge: bridge)
                .tabIcon(.wallet)
            AccountTab(bridge: bridge)
                .tabIcon(.account)
        }
        // A long press on the tab bar reports the screen on view.
        .background(TabBarLongPress { reporter.start(bridge: bridge, place: place) })
        .sheet(item: $reporter.session) { session in
            ReportSheet(session: session, bridge: bridge)
        }
        .onChange(of: tab) { _, tab in
            place.tab = tab
            bridge.playtestScreen(tab: place.tabName, route: place.routeName)
        }
        // Coder asked to connect a computer: Account > Computers.
        .onChange(of: bridge.computersRequested) { _, _ in tab = .account }
        // An offer under a chat reply opened another screen.
        .onChange(of: bridge.screenRequest) { _, request in
            switch request.screen {
            case "wallet": tab = .wallet
            case "keys", "playtest": tab = .account
            case "report": reporter.start(bridge: bridge, place: place)
            // See the board: the Verse tab, walked into the Gym before its
            // EVALS board.
            case "verse_gym":
                VerseWorldView.pendingGoEvals = true
                tab = .verse
            default: break
            }
        }
        .onAppear {
            #if targetEnvironment(simulator)
            // `--report` opens Report a problem for the first screen.
            if ProcessInfo.processInfo.arguments.contains("--report") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                    reporter.start(bridge: bridge, place: place)
                }
            }
            #endif
        }
        .environmentObject(place)
        .environmentObject(reporter)
        // An agent's payment request shows over any tab until the owner
        // approves or denies it; Rust closes it.
        .sheet(item: Binding(get: { bridge.packet?.spend?.sheet }, set: { _ in })) { sheet in
            SpendApprovalSheet(sheet: sheet, waiting: bridge.packet?.spend?.waiting ?? 0,
                               busy: bridge.packet?.spend?.busy ?? false, bridge: bridge)
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
    @EnvironmentObject private var place: PlaytestPlace
    @EnvironmentObject private var reporter: ReportCoordinator

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    NavigationLink(value: AccountRoute.trainer) {
                        Label("Trainer", systemImage: "star.circle")
                    }
                    .accessibilityIdentifier("account-trainer")
                }
                Section {
                    NavigationLink(value: AccountRoute.playtest) {
                        Label("Playtest", systemImage: "gamecontroller")
                    }
                    .accessibilityIdentifier("account-playtest")
                    Button {
                        reporter.start(bridge: bridge, place: place)
                    } label: {
                        Label("Report a problem", systemImage: "exclamationmark.bubble")
                    }
                    .foregroundStyle(.white)
                    .accessibilityIdentifier("account-report")
                }
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
        .onChange(of: bridge.computersRequested) { _, _ in path = [.computers] }
        .onChange(of: bridge.screenRequest) { _, request in
            switch request.screen {
            case "keys": path = [.identity]
            case "playtest": path = [.playtest]
            default: break
            }
        }
        .onChange(of: path, initial: true) { _, path in
            place.accountRoute = path.last
            if place.tab == .account {
                bridge.playtestScreen(tab: place.tabName, route: place.routeName)
            }
        }
    }

    @ViewBuilder private func destination(_ route: AccountRoute) -> some View {
        switch route {
        case .trainer: TrainerScreen(bridge: bridge).navigationTitle("Trainer")
        case .computers: ComputersTab(bridge: bridge) // It sets its own title.
        case .tailnet: TailnetTab(bridge: bridge).navigationTitle("") // The screen draws its own heading.
        case .identity: IdentityKeysScreen(bridge: bridge).navigationTitle("Identity keys")
        case .device: AboutDeviceScreen(bridge: bridge).navigationTitle("About this device")
        case .changelog: ChangelogScreen(bridge: bridge).navigationTitle("Changelog")
        case .playtest: PlaytestScreen(bridge: bridge).navigationTitle("Playtest")
        case .reports: MyReportsScreen(bridge: bridge).navigationTitle("My reports")
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
