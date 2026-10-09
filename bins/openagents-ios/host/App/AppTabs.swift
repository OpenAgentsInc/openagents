// The app's four tabs: Chat, Verse, Wallet, and Account. The tab bar shows
// icons only; each tab keeps a spoken name for VoiceOver.
import SwiftUI
import UIKit

enum AppTab: String, CaseIterable {
    case coder, verse, wallet, account
    /// The account surface (#11107, #11165): sign-in, the account's chats,
    /// and Running. Opened from the drawer and Settings.
    case link

    var title: String {
        switch self {
        case .coder: "Chat"
        case .verse: "Verse"
        case .wallet: "Wallet"
        case .account: "Settings"
        case .link: "Account"
        }
    }

    var symbol: String {
        switch self {
        case .coder: "bubble.left.and.bubble.right"
        case .verse: "globe"
        // `wallet.bifold` arrived in iOS 18.
        case .wallet: UIImage(systemName: "wallet.bifold") == nil ? "creditcard" : "wallet.bifold"
        case .account: "person.crop.circle"
        case .link: "person.crop.circle.badge.checkmark"
        }
    }
}

/// A screen that the Account tab pushes.
enum AccountRoute: String, Hashable {
    case trainer, computers, tailnet, keys, identity, device, changelog, playtest, reports

    /// Shown only in a preview build (`Preview`).
    var previewOnly: Bool {
        switch self {
        case .trainer, .tailnet, .playtest, .reports: true
        default: false
        }
    }
}

/// The release gate (docs/mobile/1.0-audit.md): the Gym (its boards in the
/// Grid, Train Coder, Profile), Everglade, Trainer, Playtest, Tailnet, and
/// the display name show only when the Rust library was built with
/// `OPENAGENTS_MOBILE_PREVIEW=on`. Release and normal simulator builds hide
/// them; their code stays. The plain Grid (the Verse) shows in every build.
enum Preview {
    static let on = openagents_mobile_preview()

    static func shows(_ tab: AppTab) -> Bool { true }
    static func shows(_ route: AccountRoute) -> Bool { on || !route.previewOnly }
}

/// Developer launch arguments that open a tab or an Account screen directly,
/// for example `--tab account --account-route tailnet`.
enum AppTabLaunch {
    static var tab: AppTab {
        #if DEBUG || targetEnvironment(simulator)
        if let value = argument("--tab"), let tab = AppTab(rawValue: value), Preview.shows(tab) { return tab }
        #endif
        return .coder
    }

    static var route: [AccountRoute] {
        #if DEBUG || targetEnvironment(simulator)
        if let value = argument("--account-route"), let route = AccountRoute(rawValue: value),
           Preview.shows(route) {
            return [route]
        }
        #endif
        return []
    }

    /// `--drawer`: open on the drawer, for screenshots.
    static var drawer: Bool {
        #if DEBUG || targetEnvironment(simulator)
        return ProcessInfo.processInfo.arguments.contains("--drawer")
        #else
        return false
        #endif
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
    /// The drawer (#11126) is open over the place on view.
    @State private var drawer = AppTabLaunch.drawer
    /// Settings opens on its first screen when this changes.
    @State private var settingsHome = 0
    /// The account surface's screen (`account`, `chats`, `chat`, or
    /// `running`) and the chat it opens.
    @State private var linkScreen = "account"
    @State private var linkChat: String?
    @ObservedObject private var notifier = LinkNotifier.shared
    /// The Grid was opened once: it stays mounted (paused) behind the chat
    /// so going back is instant.
    @State private var verseMounted = false
    /// The shell's switch is on Verse.
    private var inVerse: Bool { tab == .coder && bridge.packet?.shell?.place == "verse" }
    @Environment(\.appColors) private var appColors

    var body: some View {
        GeometryReader { proxy in
            let width = min(proxy.size.width * 0.82, 360)
            ZStack(alignment: .leading) {
                if drawer {
                    ShellDrawer(state: bridge.packet?.shell, bridge: bridge, go: go, close: closeDrawer)
                        .frame(width: width)
                        .transition(.move(edge: .leading).combined(with: .opacity))
                }
                current
                    // Closed, the clip reaches past the safe areas, where
                    // the Verse's world draws.
                    .clipShape(RoundedRectangle(cornerRadius: drawer ? 32 : 0, style: .continuous)
                        .inset(by: drawer ? 0 : -200))
                    .overlay {
                        if drawer {
                            RoundedRectangle(cornerRadius: 32, style: .continuous)
                                .fill(Color.black.opacity(appColors.scheme == .dark ? 0.35 : 0.12))
                                .ignoresSafeArea()
                                .onTapGesture(perform: closeDrawer)
                                .accessibilityLabel("Close menu")
                                .accessibilityAddTraits(.isButton)
                                .accessibilityIdentifier("shell-close")
                        }
                    }
                    .offset(x: drawer ? width : 0)
            }
            .animation(.snappy(duration: 0.3), value: drawer)
            .background(appColors.background.ignoresSafeArea())
        }
        .onChange(of: drawer, initial: true) { _, open in
            bridge.shell("drawer", ["open": open])
            bridge.link("drawer", ["open": open])
            if open {
                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
            }
        }
        .sheet(item: $reporter.session) { session in
            ReportSheet(session: session, bridge: bridge)
        }
        // Give feedback from a transcript's selection menu (#10127).
        .sheet(item: $reporter.feedback) { request in
            FeedbackSheet(request: request, tab: place.tabName, route: place.routeName, bridge: bridge)
        }
        .onChange(of: tab) { _, tab in
            place.tab = tab
            bridge.playtestScreen(tab: place.tabName, route: place.routeName)
        }
        // Coder asked to connect a computer: Account > Computers.
        .onChange(of: bridge.computersRequested) { _, _ in tab = .account }
        // A tap on a notice from Coder: Running.
        .onChange(of: notifier.openRunning) { _, _ in go("running") }
        // An offer under a chat reply opened another screen.
        .onChange(of: bridge.screenRequest) { _, request in
            switch request.screen {
            case "wallet": tab = .wallet
            case "keys": tab = .account
            case "playtest" where Preview.on: tab = .account
            case "report": reporter.start(bridge: bridge, place: place)
            // Train Coder from the Verse or Account: the Chat tab, on the
            // Gym intro.
            case "chat": tab = .coder
            // See the board: the Verse tab, walked into the Gym before its
            // EVALS board.
            case "verse_gym" where Preview.on:
                VerseWorldView.pendingGoEvals = true
                bridge.shell("switch", ["verse": true])
                tab = .coder
            case "verse":
                bridge.shell("switch", ["verse": true])
                tab = .coder
            default: break
            }
        }
        .onChange(of: inVerse, initial: true) { _, verse in
            guard verse else { return }
            verseMounted = true
            // The chat's keyboard goes away with the chat.
            UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        }
        .onAppear {
            // `--tab verse` opens on the Grid.
            if AppTabLaunch.tab == .verse {
                tab = .coder
                bridge.shell("switch", ["verse": true])
            }
            NativeRowView.giveFeedback = { text, row in
                reporter.feedback = FeedbackRequest(text: text, row: row)
            }
            #if targetEnvironment(simulator)
            // `--connect` opens Connect a computer, the scanner.
            if ProcessInfo.processInfo.arguments.contains("--connect") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { bridge.connectOpen() }
            }
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
        // A computer's ask for the wallet shows over any tab until the
        // owner approves or denies it; Rust closes it.
        .sheet(item: Binding(get: { bridge.packet?.spend?.sheet == nil ? bridge.packet?.wallet_link?.sheet : nil },
                             set: { _ in })) { sheet in
            WalletLinkSheet(sheet: sheet, busy: bridge.packet?.wallet_link?.busy ?? false, bridge: bridge)
        }
        .alert(bridge.packet?.wallet_link?.notice ?? "",
               isPresented: Binding(get: { bridge.packet?.wallet_link?.notice != nil },
                                    set: { shown in if !shown { bridge.wallet("wallet_link_dismiss") } })) {
            Button("OK") { bridge.wallet("wallet_link_dismiss") }
        }
        // Connect a computer, from a chat's offer or Account > Computers.
        // Done or Close returns to the screen it was opened from.
        .fullScreenCover(isPresented: Binding(get: { bridge.packet?.connect != nil },
                                              set: { shown in if !shown { bridge.connectClose() } })) {
            if let screen = bridge.packet?.connect {
                ConnectView(screen: screen, bridge: bridge)
            }
        }
    }
}

extension AppTabs {
    /// The place on view, each with the menu button that opens the drawer.
    @ViewBuilder var current: some View {
        switch tab {
        case .coder, .verse:
            ZStack {
                // Out of the Verse's way, keyboard and all; Rust keeps the
                // chat's state.
                if !inVerse {
                    CoderTab(bridge: bridge, openDrawer: openDrawer,
                             report: { reporter.start(bridge: bridge, place: place) })
                }
                // The Verse: the Grid world under the shell's top bar, its
                // switch back to Coder and the menu. The world is dark only
                // for now (#11028); it pauses while the chat shows.
                if verseMounted {
                    VerseTab(app: bridge, selected: inVerse, studioComputer: bridge.studioComputer,
                             connectStudio: bridge.studioConnect) { bridge.gymTrain() }
                        .ignoresSafeArea(.keyboard)
                        .environment(\.colorScheme, .dark)
                        .overlay(alignment: .top) {
                            ShellTopBar(state: bridge.packet?.shell, bridge: bridge, openDrawer: openDrawer,
                                        report: { reporter.start(bridge: bridge, place: place) })
                                .environment(\.colorScheme, .dark)
                                .environment(\.appColors, AppColors.dark)
                        }
                        .opacity(inVerse ? 1 : 0)
                        .allowsHitTesting(inVerse)
                        .accessibilityHidden(!inVerse)
                }
            }
        case .wallet:
            WalletTab(bridge: bridge)
                .environment(\.colorScheme, .dark)
                .safeAreaInset(edge: .top, spacing: 0) {
                    HStack { menuButton; Spacer() }
                        .padding(.horizontal, 16).padding(.top, 4)
                        .background(Color.black.ignoresSafeArea())
                        .environment(\.colorScheme, .dark)
                }
        case .account:
            AccountTab(bridge: bridge, home: settingsHome, menu: { AnyView(menuButton) },
                       openLink: { go("account") })
        case .link:
            LinkScreen(bridge: bridge, screen: linkScreen, chat: linkChat, menu: { AnyView(menuButton) })
        }
    }

    private var menuButton: some View {
        ShellMenuButton(open: openDrawer, report: { reporter.start(bridge: bridge, place: place) })
    }

    private func openDrawer() { drawer = true }
    private func closeDrawer() { drawer = false }

    /// Open a place from the drawer. Any place but the Verse leaves it.
    func go(_ id: String) {
        if id != "verse", inVerse { bridge.shell("switch", ["verse": false]) }
        switch id {
        case "code":
            bridge.shell("switch", ["verse": false])
            tab = .coder
        case "computers":
            tab = .account
            bridge.showNativeComputers()
        case "wallet": tab = .wallet
        case "verse":
            bridge.shell("switch", ["verse": true])
            tab = .coder
        case "settings":
            settingsHome += 1
            tab = .account
        case "account", "running", "account_chats":
            linkScreen = id == "account_chats" ? "chats" : id
            linkChat = nil
            tab = .link
        case let place where place.hasPrefix("link_chat:"):
            linkScreen = "chat"
            linkChat = String(place.dropFirst("link_chat:".count))
            tab = .link
        default: tab = .coder
        }
        drawer = false
    }
}

/// Settings and the screens that used to be tabs.
struct AccountTab: View {
    @ObservedObject var bridge: MobileBridge
    /// Back to the first screen when it changes (the drawer's Settings).
    var home = 0
    /// The menu button that opens the drawer, on the first screen.
    var menu: (() -> AnyView)?
    /// Open the account surface (Log in, or the account).
    var openLink: () -> Void = {}
    @Environment(\.appColors) private var appColors
    @State private var path = AppTabLaunch.route
    @EnvironmentObject private var place: PlaytestPlace
    @EnvironmentObject private var reporter: ReportCoordinator

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    LinkSettingsRow(bridge: bridge, open: openLink)
                }
                // Trainer, Train Coder, Profile, and Playtest are preview
                // features (`Preview`).
                if Preview.on {
                    Section {
                        NavigationLink(value: AccountRoute.trainer) {
                            Label("Trainer", systemImage: "star.circle")
                        }
                        .accessibilityIdentifier("account-trainer")
                        // Train Coder opened a test of the Gym's sample plugins,
                        // which are no longer shown; it comes back when the Gym
                        // has a real plugin to test.
                        // Button {
                        //     bridge.gymTrain()
                        // } label: {
                        //     Label("Train Coder", systemImage: "dumbbell")
                        // }
                        // .foregroundStyle(appColors.primary)
                        // .accessibilityIdentifier("account-train")
                        // Profile: Rust shows it as a sheet on the Chat tab.
                        Button {
                            bridge.profile()
                        } label: {
                            Label("Profile", systemImage: "person.circle")
                        }
                        .foregroundStyle(appColors.primary)
                        .accessibilityIdentifier("account-profile")
                    }
                }
                Section {
                    if Preview.on {
                        NavigationLink(value: AccountRoute.playtest) {
                            Label("Playtest", systemImage: "gamecontroller")
                        }
                        .accessibilityIdentifier("account-playtest")
                    }
                    Button {
                        reporter.start(bridge: bridge, place: place)
                    } label: {
                        Label("Report a problem", systemImage: "exclamationmark.bubble")
                    }
                    .foregroundStyle(appColors.primary)
                    .accessibilityIdentifier("account-report")
                }
                Section {
                    NavigationLink("Computers", value: AccountRoute.computers)
                    if Preview.on {
                        NavigationLink("Tailnet", value: AccountRoute.tailnet)
                    }
                }
                Section {
                    // System follows the phone; Rust saves the choice.
                    Picker("Appearance", selection: Binding(
                        get: { bridge.packet?.appearance?.choice ?? "dark" },
                        set: { bridge.chooseTheme($0) })) {
                        ForEach(bridge.packet?.appearance?.choices ?? []) { Text($0.label).tag($0.id) }
                    }
                    .accessibilityIdentifier("account-appearance")
                }
                Section {
                    NavigationLink("Your keys", value: AccountRoute.keys)
                        .accessibilityIdentifier("account-your-keys")
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
            .background(appColors.background.ignoresSafeArea())
            .navigationTitle("Settings")
            .toolbar {
                if let menu {
                    ToolbarItem(placement: .topBarLeading) { menu() }
                }
            }
            .navigationDestination(for: AccountRoute.self) { route in
                destination(route)
                    .navigationBarTitleDisplayMode(.inline)
                    .toolbarBackground(appColors.background, for: .navigationBar)
            }
        }
        .onChange(of: bridge.computersRequested) { _, _ in path = [.computers] }
        .onChange(of: home) { _, _ in path = [] }
        .onChange(of: bridge.screenRequest) { _, request in
            switch request.screen {
            case "keys": path = [.identity]
            case "playtest" where Preview.on: path = [.playtest]
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
        if !Preview.shows(route) {
            EmptyView()
        } else {
            routeScreen(route)
        }
    }

    @ViewBuilder private func routeScreen(_ route: AccountRoute) -> some View {
        switch route {
        case .trainer: TrainerScreen(bridge: bridge).navigationTitle("Trainer")
        case .computers: ComputersTab(bridge: bridge) // It sets its own title.
        case .tailnet: TailnetTab(bridge: bridge).navigationTitle("") // The screen draws its own heading.
        case .keys: YourKeysScreen(bridge: bridge).navigationTitle("Your keys")
        case .identity: IdentityKeysScreen(bridge: bridge).navigationTitle("Identity keys")
        case .device: AboutDeviceScreen(bridge: bridge).navigationTitle("About this device")
        case .changelog: ChangelogScreen(bridge: bridge).navigationTitle("Changelog")
        // Playtest and My reports are dark only for now (#11028).
        case .playtest: PlaytestScreen(bridge: bridge).navigationTitle("Playtest").environment(\.colorScheme, .dark)
        case .reports: MyReportsScreen(bridge: bridge).navigationTitle("My reports").environment(\.colorScheme, .dark)
        }
    }
}

/// A row that opens a web page in the browser.
private struct ExternalLink: View {
    @Environment(\.appColors) private var appColors
    let title: String
    let symbol: String
    let url: String

    var body: some View {
        if let destination = URL(string: url) {
            Link(destination: destination) {
                HStack {
                    Label(title, systemImage: symbol)
                    Spacer()
                    Image(systemName: "arrow.up.right").font(.paper(.footnote)).foregroundStyle(.secondary)
                }
            }
            .foregroundStyle(appColors.primary)
        }
    }
}
