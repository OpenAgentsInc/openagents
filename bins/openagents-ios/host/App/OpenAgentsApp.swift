// Rust builds every screen; this host decodes and renders them, and supplies
// the pieces a Rust Native tree cannot: tabs, the camera, keyboards, and the
// terminal's keyboard target.
import PhotosUI
import SwiftUI

@main
struct OpenAgentsApp: App {
    @UIApplicationDelegateAdaptor(PushAppDelegate.self) private var pushDelegate
    @StateObject private var bridge = MobileBridge()
    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            HomeScreen(bridge: bridge)
                .font(.paper(.body))
                .dismissesKeyboardOnOutsideTap()
                .onChange(of: scenePhase, initial: true) { _, phase in
                    // The System theme follows the phone; a change made in
                    // Settings or Control Center shows on coming back.
                    if phase == .active { bridge.reportSystemAppearance() }
                    if phase != .inactive { bridge.lifecycle(phase == .active) }
                }
                .nativeFixture()
                .task {
                    // Does nothing unless this build is configured for push.
                    PushRegistration.shared.start(deliver: { bridge.pushToken($0) },
                                                  failed: { bridge.pushFailed($0) })
                    #if DEBUG || targetEnvironment(simulator)
                    // `--connect-link URL` opens the app as the link would,
                    // for a simulator that cannot verify openagents.com.
                    if let link = AppTabLaunch.wallet("--connect-link") { bridge.connectLink(link) }
                    // Screenshots: `--appearance light|dark|system` picks the
                    // theme (saved), and `--shell-mode code` opens Code mode.
                    if let theme = AppTabLaunch.wallet("--appearance") { bridge.chooseTheme(theme) }
                    if AppTabLaunch.wallet("--shell-mode") == "code" { bridge.shell("mode", ["code": true]) }
                    #endif
                }
                // The desktop app's QR code is a universal link,
                // https://openagents.com/connect#<code>: the system camera
                // opens the app with it, and Rust pairs as if it was scanned.
                .onOpenURL { url in bridge.connectLink(url.absoluteString) }
        }
    }
}

struct HomeScreen: View {
    @ObservedObject var bridge: MobileBridge

    var body: some View {
        AppTabs(bridge: bridge)
            // Rust resolves the theme; the scheme sets the status bar and
            // system chrome, and the palette the host's own chrome.
            .tint(bridge.colors.primary)
            .preferredColorScheme(bridge.colors.scheme)
            .environment(\.appColors, bridge.colors)
            .background(SystemAppearanceWatcher { bridge.reportSystemAppearance() })
            .fullScreenCover(isPresented: Binding(
                get: { bridge.packet?.terminal == true && !bridge.gpuTerminalVisible },
                set: { _ in })) {
                // A terminal is Coder Noir in both looks.
                TerminalScreen(bridge: bridge)
                    .preferredColorScheme(.dark)
            }
    }
}

/// One Rust Native surface on black, scrolling when it is taller than the
/// screen.
struct SurfaceView: View {
    let view: NativeView?
    let activate: (NativeView, String) -> Void
    @Environment(\.appColors) private var appColors

    var body: some View {
        ScrollView {
            if let view {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil, activate: { node in activate(view, node) })
                    .frame(maxWidth: .infinity, alignment: .topLeading)
            }
        }
        .background(appColors.background.ignoresSafeArea())
    }
}

/// Computers: a native list of your computers, then Coder's shared screens
/// for one computer, adding one, and activity. Rust builds both and checks
/// every choice.
struct ComputersTab: View {
    @ObservedObject var bridge: MobileBridge
    @Environment(\.appColors) private var appColors

    private var home: ComputersHome? { bridge.packet?.computers_home }

    var body: some View {
        VStack(spacing: 0) {
            ForEach(bridge.packet?.notices ?? [], id: \.self) { notice in
                Text(notice).font(.paper(.caption)).foregroundStyle(.secondary).padding(.horizontal)
            }
            if let failure = bridge.failure {
                Text(failure).font(.paper(.caption)).padding(.horizontal)
            }
            if let home {
                ComputersList(home: home, bridge: bridge)
            } else {
                SurfaceView(view: bridge.packet?.computers) { view, node in
                    bridge.activate("computers", view: view, node: node)
                }
                if let qr = bridge.packet?.computers_qr {
                    InvitationQR(qr: qr).frame(width: 220, height: 220).padding()
                        .accessibilityLabel("Invitation QR code")
                }
            }
            if let input = bridge.packet?.computers_input {
                InputBar(input: input, busy: bridge.busy,
                         submit: { bridge.submit("computers", token: input.token, value: $0) },
                         cancel: { bridge.cancel("computers", token: input.token) })
                    .id(input.token)
            }
        }
        .background(appColors.background.ignoresSafeArea())
        .navigationTitle(home == nil ? "" : "Computers")
        .navigationBarTitleDisplayMode(.inline)
        // Past the list, back returns to it rather than to Account.
        .navigationBarBackButtonHidden(home == nil)
        .toolbar {
            if let home {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Button(home.add_other, systemImage: "plus") { bridge.computersGo("add") }
                        Button("Activity", systemImage: "list.bullet") { bridge.computersGo("activity") }
                        Button("Refresh", systemImage: "arrow.clockwise") { bridge.computersGo("refresh") }
                        if home.owner_key {
                            Button("Enter owner key", systemImage: "key") { bridge.computersGo("owner_key") }
                        }
                        if home.keep_directory {
                            Button("Keep this device's version", systemImage: "checkmark.circle") {
                                bridge.computersGo("keep_directory")
                            }
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                    }
                    .accessibilityLabel("More")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button { bridge.connectOpen() } label: { Image(systemName: "plus") }
                        .accessibilityLabel(home.connect)
                }
            } else {
                ToolbarItem(placement: .topBarLeading) {
                    Button { bridge.computersGo("home") } label: { Image(systemName: "chevron.left") }
                        .accessibilityLabel("Computers")
                }
            }
        }
        .onAppear {
            bridge.computersGo("home")
            #if targetEnvironment(simulator)
            // `--computers-script open|add|activity` opens the first computer,
            // adding a computer, or activity, so a simulator check needs no taps.
            let arguments = ProcessInfo.processInfo.arguments
            if let index = arguments.firstIndex(of: "--computers-script"), index + 1 < arguments.count {
                let step = arguments[index + 1]
                Task {
                    try? await Task.sleep(for: .seconds(1))
                    if step == "open", let first = bridge.packet?.computers_home?.rows.first {
                        bridge.openComputer(first.host)
                    } else if step == "add" || step == "activity" {
                        bridge.computersGo(step)
                    }
                }
            }
            #endif
        }
        // Host status moves on its own; poll while no value is being entered.
        .task(id: bridge.packet?.computers_input == nil) {
            guard bridge.packet?.computers_input == nil else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled && !bridge.busy { bridge.refreshComputers() }
            }
        }
    }
}

/// One row per computer: a status dot, its name, and a short status. A tap
/// opens it; its menu holds the rest.
private struct ComputersList: View {
    let home: ComputersHome
    @ObservedObject var bridge: MobileBridge
    @Environment(\.appColors) private var appColors
    @State private var confirming: (row: ComputersHome.Row, item: ComputersHome.Item)?

    var body: some View {
        List {
            if let empty = home.empty {
                Section {
                    VStack(spacing: 12) {
                        Image(systemName: "desktopcomputer")
                            .font(.paper(40, weight: .light))
                            .foregroundStyle(.secondary)
                        Text(empty)
                            .font(.paper(.subheadline))
                            .foregroundStyle(.secondary)
                            .multilineTextAlignment(.center)
                        Button(home.connect) { bridge.connectOpen() }
                            .accessibilityIdentifier("computers-connect")
                            .buttonStyle(.borderedProminent)
                            .tint(appColors.primary)
                            .foregroundStyle(appColors.background)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 24)
                }
            } else {
                Section {
                    Button(home.connect, systemImage: "qrcode.viewfinder") { bridge.connectOpen() }
                        .accessibilityIdentifier("computers-connect")
                }
                Section {
                    ForEach(home.rows) { row in
                        Button { bridge.openComputer(row.host) } label: { ComputerRow(row: row) }
                            .accessibilityIdentifier("computer-\(row.name)")
                            .contextMenu {
                                ForEach(row.menu, id: \.self) { item in
                                    Button(item.label, role: item.confirm == nil ? nil : .destructive) {
                                        choose(row, item)
                                    }
                                }
                            }
                            .swipeActions {
                                if let forget = row.menu.first(where: { $0.choice == "forget" }) {
                                    Button("Forget", systemImage: "trash", role: .destructive) {
                                        choose(row, forget)
                                    }
                                }
                            }
                    }
                } footer: {
                    if let notice = home.notice { Text(notice) }
                }
            }
        }
        .listStyle(.insetGrouped)
        .scrollContentBackground(.hidden)
        .background(appColors.background.ignoresSafeArea())
        .confirmationDialog(confirming?.item.confirm ?? "", isPresented: Binding(
            get: { confirming != nil }, set: { if !$0 { confirming = nil } }),
            titleVisibility: .visible) {
            if let confirming {
                Button(confirming.item.label, role: .destructive) {
                    bridge.chooseComputer(confirming.row.host, confirming.item.choice)
                }
            }
        }
    }

    private func choose(_ row: ComputersHome.Row, _ item: ComputersHome.Item) {
        if item.confirm == nil {
            bridge.chooseComputer(row.host, item.choice)
        } else {
            confirming = (row, item)
        }
    }
}

private struct ComputerRow: View {
    let row: ComputersHome.Row
    @Environment(\.appColors) private var appColors

    private var tone: Color {
        switch row.tone {
        case "online": .green
        case "pending": .yellow
        case "alert": .red
        default: Color(white: 0.45)
        }
    }

    var body: some View {
        HStack(spacing: 12) {
            Circle().fill(tone).frame(width: 8, height: 8)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.name).foregroundStyle(appColors.primary)
                Text(row.status).font(.paper(.subheadline)).foregroundStyle(.secondary)
                if let watchers = row.watchers {
                    Text(watchers).font(.paper(.footnote)).foregroundStyle(.secondary)
                }
                if let background = row.background {
                    Text(background).font(.paper(.footnote)).foregroundStyle(.secondary)
                }
            }
            Spacer(minLength: 8)
            Image(systemName: "chevron.right")
                .font(.paper(.footnote, weight: .semibold))
                .foregroundStyle(.tertiary)
        }
        .padding(.vertical, 2)
        .contentShape(Rectangle())
    }
}

/// The Chat tab: the chat, which Rust opens on a new chat ready to type
/// (chat first, on a new install too), with previous Coder chats behind the
/// menu button, Profile in the header, and the Gym's cards under replies;
/// after **Train Coder**, the Gym intro (`SCR-02`, the end card) and the
/// Gym menu (`SCR-01`). Rust says which one shows (`gym.screen`).
struct CoderTab: View {
    @ObservedObject var bridge: MobileBridge
    /// Opens the shell's drawer (#11126).
    var openDrawer: () -> Void = {}
    /// Report a problem with the screen on view (a long press on the menu).
    var report: () -> Void = {}
    @Environment(\.appColors) private var appColors
    /// The sheet this host is showing, to tell a swipe from Rust closing it.
    @State private var shownSheet: String?
    @State private var picking = false
    @State private var picked: PhotosPickerItem?

    private var gym: GymPacket? { bridge.packet?.gymPacket }

    var body: some View {
        Group {
            switch gym?.screen {
            case "menu":
                // The Gym's screens are dark only for now (#11028).
                if let menu = gym?.menu {
                    GymMenuView(menu: menu) { bridge.gym($0) }.environment(\.colorScheme, .dark)
                }
            case "first_run":
                if let first = gym?.first_run {
                    GymFirstRunView(first: first) { bridge.gym($0) }.environment(\.colorScheme, .dark)
                }
            default:
                if let view = bridge.packet?.coder {
                    NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                                   followChanged: nil,
                                   surface: { resource, label in
                                       if resource == "home-cards" {
                                           return AnyView(HomeCardsSurface(cards: bridge.packet?.shell?.cards ?? []) {
                                               bridge.shell("try_card", ["id": $0])
                                           })
                                       }
                                       if resource.hasPrefix("image:") {
                                           return AnyView(ChatImageSurface(resource: resource, label: label,
                                                                           bridge: bridge))
                                       }
                                       if resource.hasPrefix("link:") {
                                           return AnyView(LinkCardSurface(resource: resource, label: label,
                                                                          card: bridge.packet?.links?[resource],
                                                                          bridge: bridge)
                                               .environment(\.appColors, appColors))
                                       }
                                       return AnyView(GymCardSurface(resource: resource, bridge: bridge))
                                   },
                                   submit: { token, text in bridge.submit("coder", token: token, value: text) },
                                   activate: { node in bridge.activate("coder", view: view, node: node) },
                                   activateCurrent: { node in
                                       bridge.activate("coder", view: bridge.packet?.coder ?? view, node: node)
                                   })
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        // Under the shell the composer floats over the
                        // conversation (#11126).
                        .environment(\.nativeFloatingComposer,
                                     bridge.packet?.shell != nil ? appColors.background : nil)
                } else {
                    Color.clear
                }
            }
        }
        .toolbar(gym?.screen == "first_run" ? .hidden : .visible, for: .tabBar)
        // The shell's top bar over the chat; every chat lists its own.
        .safeAreaInset(edge: .top, spacing: 0) {
            if (gym?.screen ?? "chat") == "chat", let shell = bridge.packet?.shell, shell.screen != "list" {
                ShellTopBar(state: shell, bridge: bridge, openDrawer: openDrawer, report: report)
            }
        }
        // **Attach image**: the system photo picker; Rust decodes the photo.
        // Mounted only while the chat takes images; the phone is text only
        // since #10093 (`coder_tab::ATTACHMENTS_ENABLED`).
        .photosPicker(isPresented: Binding(get: { picking && bridge.attachmentsEnabled },
                                           set: { picking = $0 }),
                      selection: $picked, matching: .images)
        .onChange(of: bridge.imagePickRequested) { _, _ in picking = bridge.attachmentsEnabled }
        .onChange(of: picked) { _, item in
            guard let item else { return }
            picked = nil
            Task {
                guard let data = try? await item.loadTransferable(type: Data.self),
                      let photo = ChatImageSurface.encoded(data) else { return }
                bridge.attachImage(name: photo.name, data: photo.data)
            }
        }
        .sheet(item: Binding(get: { gym?.sheet }, set: { _ in }), onDismiss: {
            // A swipe closed it while Rust still shows it: tell Rust.
            if let sheet = gym?.sheet, sheet.id == shownSheet {
                bridge.gym(sheet.close?.id ?? (sheet.kind == "stop" ? "sheet.keep" : sheet.primary?.id ?? "sheet.close"))
            }
            shownSheet = nil
        }) { sheet in
            GymSheetView(sheet: sheet) { bridge.gym($0) }
                .presentationDetents(sheet.kind == "publish" || sheet.kind == "stop" ? [.fraction(0.72), .large] : [.large])
                .onAppear { shownSheet = sheet.id }
        }
        .sheet(item: Binding(get: { bridge.gymShare.map { GymShareText(text: $0) } },
                             set: { if $0 == nil { bridge.gymShare = nil } })) { share in
            GymShareSheet(text: share.text)
        }
        .background(appColors.background.ignoresSafeArea())
        .task { await CoderLaunchTaps.run(bridge) }
        // Rust says when a transcript page, a streamed reply, or a task's
        // status arrives, and asks every second while the open chat runs
        // (MobileBridge.watchChanges). This slow timer is only a fallback.
        .onAppear { bridge.coderShown(true) }
        .onDisappear { bridge.coderShown(false) }
        .task {
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(3))
                if !Task.isCancelled && !bridge.busy { bridge.refreshComputers() }
            }
        }
    }
}

/// Text to share, as a sheet's item.
struct GymShareText: Identifiable {
    let text: String
    var id: String { text }
}

/// The system share sheet.
struct GymShareSheet: UIViewControllerRepresentable {
    let text: String

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: [text], applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}

/// Simulator checks: `--coder-tap KEY[,KEY...]` taps Coder nodes in order
/// once the surface shows them, waiting up to 30 seconds for each. A key
/// ending in `*` taps the first node whose key starts with the rest, such as
/// `task-*` for the first chat in the menu. A step `send:TEXT` sends TEXT
/// from the screen's composer; it is the last step, and TEXT is the rest of
/// the list, commas and all (#10118). `sleep:N` waits N seconds, and `try:KEY`
/// taps KEY only if the screen shows it now (a tap the screen may have
/// replaced before it arrived, tried again), and `attach:PATH` hands the
/// image file at PATH to the draft as the photo picker would (dropped while
/// the chat is text only, #10093). Then
/// `--coder-send TEXT` sends TEXT from the screen's composer.
enum CoderLaunchTaps {
    @MainActor static func run(_ bridge: MobileBridge) async {
        #if DEBUG || targetEnvironment(simulator)
        let arguments = ProcessInfo.processInfo.arguments
        if let index = arguments.firstIndex(of: "--coder-tap"), index + 1 < arguments.count {
            var steps: [String] = []
            for piece in arguments[index + 1].split(separator: ",", omittingEmptySubsequences: false).map(String.init) {
                if let last = steps.last, last.hasPrefix("send:") {
                    steps[steps.count - 1] = last + "," + piece
                } else {
                    steps.append(piece)
                }
            }
            for key in steps where !key.isEmpty {
                if key.hasPrefix("sleep:") {
                    try? await Task.sleep(for: .seconds(Double(key.dropFirst(6)) ?? 1))
                    continue
                }
                if key.hasPrefix("attach:") {
                    // A photo from the Mac's disk, as the picker would send it.
                    let path = String(key.dropFirst(7))
                    if let data = FileManager.default.contents(atPath: path),
                       let photo = ChatImageSurface.encoded(data) {
                        bridge.attachImage(name: photo.name, data: photo.data)
                    }
                    continue
                }
                if key.hasPrefix("try:") {
                    if let view = bridge.packet?.coder, let node = find(String(key.dropFirst(4)), in: view.root) {
                        bridge.activate("coder", view: view, node: node)
                    }
                    continue
                }
                for _ in 0..<60 {
                    try? await Task.sleep(for: .milliseconds(500))
                    if key.hasPrefix("send:") {
                        guard let view = bridge.packet?.coder, let token = composer(in: view.root) else { continue }
                        bridge.submit("coder", token: token, value: String(key.dropFirst(5)))
                        break
                    }
                    guard let view = bridge.packet?.coder, let node = find(key, in: view.root) else { continue }
                    bridge.activate("coder", view: view, node: node)
                    break
                }
            }
        }
        if let index = arguments.firstIndex(of: "--coder-send"), index + 1 < arguments.count {
            for _ in 0..<20 {
                try? await Task.sleep(for: .milliseconds(500))
                guard let view = bridge.packet?.coder, let token = composer(in: view.root) else { continue }
                bridge.submit("coder", token: token, value: arguments[index + 1])
                break
            }
        }
        // `--gym-script "tap:menu.chat|send:Which tool should I try?|tap:*.start|sleep:3"`:
        // each step waits up to a minute for its button or composer. A tap
        // names a Gym button's ID or a chat node's key; `*` ends a prefix.
        guard let index = arguments.firstIndex(of: "--gym-script"), index + 1 < arguments.count else { return }
        for step in arguments[index + 1].split(separator: "|").map(String.init) {
            let (verb, value) = step.split(separator: ":", maxSplits: 1).map(String.init)
                .reduce(into: ("", "")) { pair, part in if pair.0.isEmpty { pair.0 = part } else { pair.1 = part } }
            if verb == "sleep" {
                try? await Task.sleep(for: .seconds(Double(value) ?? 1))
                continue
            }
            if verb == "hide" {
                // Put the keyboard away, as a tap outside the field does.
                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
                continue
            }
            for _ in 0..<120 {
                try? await Task.sleep(for: .milliseconds(500))
                if verb == "send" {
                    guard !bridge.busy, bridge.packet?.gymPacket?.screen == "chat" || bridge.packet?.gymPacket == nil,
                          let view = bridge.packet?.coder, let token = composer(in: view.root) else { continue }
                    bridge.submit("coder", token: token, value: value)
                    break
                }
                if let id = gymButton(value, in: bridge.packet?.gymPacket) {
                    bridge.gym(id)
                    break
                }
                if bridge.packet?.gymPacket?.screen == "chat", let view = bridge.packet?.coder,
                   let node = find(value, in: view.root) {
                    bridge.activate("coder", view: view, node: node)
                    break
                }
            }
        }
        #endif
    }

    /// The first Gym button on screen whose ID is `key`, or starts with it
    /// when it ends in `*`.
    private static func gymButton(_ key: String, in gym: GymPacket?) -> String? {
        guard let gym else { return nil }
        var ids: [String] = []
        if let sheet = gym.sheet {
            ids += [sheet.primary?.id, sheet.close?.id].compactMap { $0 } + sheet.secondary.map(\.id)
        } else {
            switch gym.screen {
            case "menu": ids += [gym.menu.primary.id] + gym.menu.chips.map(\.id) + gym.menu.rows.map(\.button.id)
            case "first_run":
                if let first = gym.first_run { ids += [first.primary.id] + [first.secondary?.id].compactMap { $0 } }
            default:
                for card in gym.cards.values.sorted(by: { $0.id < $1.id }) {
                    ids += [card.primary?.id].compactMap { $0 } + card.secondary.map(\.id) + card.chips.map(\.id)
                }
            }
        }
        let prefix = key.hasSuffix("*") ? String(key.dropLast()) : nil
        return ids.first { id in
            if let prefix {
                return prefix.hasPrefix("*") ? id.hasSuffix(String(prefix.dropFirst())) : id.hasPrefix(prefix)
            }
            if key.hasPrefix("*") { return id.hasSuffix(String(key.dropFirst())) }
            return id == key
        }
    }

    private static func composer(in node: NativeNode) -> String? {
        switch node.element {
        case let .composer(props): return props.token
        case let .stack(_, nodes), let .list(_, nodes), let .transcript(_, nodes, _, _):
            for child in nodes { if let token = composer(in: child) { return token } }
            return nil
        default: return nil
        }
    }

    private static func find(_ key: String, in node: NativeNode) -> String? {
        let matches = key.hasSuffix("*") ? node.key.hasPrefix(String(key.dropLast())) : node.key == key
        if matches { return node.key }
        let children: [NativeNode]
        switch node.element {
        case let .stack(_, nodes), let .list(_, nodes), let .transcript(_, nodes, _, _): children = nodes
        default: children = []
        }
        for child in children { if let found = find(key, in: child) { return found } }
        return nil
    }
}

struct TailnetTab: View {
    @ObservedObject var bridge: MobileBridge
    @Environment(\.appColors) private var appColors

    var body: some View {
        ZStack(alignment: .topTrailing) {
            appColors.background.ignoresSafeArea()
            // The device list is its own scrolling list; a surrounding
            // scroll view would collapse it.
            if let view = bridge.packet?.tailnet {
                NativeRenderer(node: view.root, revision: view.revision, followTarget: nil,
                               followChanged: nil,
                               activate: { node in bridge.activate("tailnet", view: view, node: node) })
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            }
            if bridge.packet?.tailnet_loading == true {
                ProgressView().padding().accessibilityLabel("Checking your tailnet")
            }
        }
        // A background read finishes on its own; poll until it does.
        .task(id: bridge.packet?.tailnet_loading == true) {
            guard bridge.packet?.tailnet_loading == true else { return }
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                if !Task.isCancelled { bridge.snapshot() }
            }
        }
    }
}

/// The keyboard or camera for the current Computers input request. Rust
/// validates every value; nothing here is kept.
struct InputBar: View {
    let input: ComputersInput
    @Environment(\.appColors) private var appColors
    let busy: Bool
    let submit: (String) -> Void
    let cancel: () -> Void
    @State private var value = ""
    @State private var scanning = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(input.label).font(.paper(.headline))
            Text(input.prompt).font(.paper(.caption))
            if let error { Text(error).font(.paper(.caption)) }
            if scanning {
                InlineQRScanner { send($0) }
                Button("Type instead") { scanning = false }
            } else if input.secret {
                SecretInputField(label: input.label, value: $value) { if !busy { send(value) } }
            } else {
                TextField(input.label, text: $value, axis: .vertical)
                    .lineLimit(1...6)
                    .autocorrectionDisabled().textInputAutocapitalization(.never)
                    .accessibilityIdentifier("computers-input")
            }
            HStack {
                if !scanning {
                    Button("Submit") { send(value) }
                        .disabled(busy || value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    if input.scan { Button("Scan QR code", systemImage: "qrcode.viewfinder") { scanning = true } }
                }
                Spacer()
                Button("Cancel") { value = ""; cancel() }
            }
        }
        .padding(12)
        .background(appColors.raised, in: RoundedRectangle(cornerRadius: 16))
        .padding(8)
        .onAppear { scanning = input.scan }
    }

    private func send(_ text: String) {
        guard text.utf8.count <= input.max_bytes else {
            error = "That's too long. Copy it again."
            return
        }
        error = nil
        value = ""
        scanning = false
        submit(text)
    }
}

/// Draws the modules Rust rendered; the invitation never leaves the device.
struct InvitationQR: View {
    let qr: ComputersQR

    var body: some View {
        Canvas { context, size in
            let cell = min(size.width, size.height) / CGFloat(max(qr.size, 1))
            context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
            for (y, row) in qr.rows.enumerated() {
                for (x, module) in row.enumerated() where module == "1" {
                    context.fill(Path(CGRect(x: CGFloat(x) * cell, y: CGFloat(y) * cell,
                                             width: cell, height: cell)), with: .color(.black))
                }
            }
        }
    }
}

/// An attached image's card: the `image:` surface Rust names, drawn from
/// the bytes Rust decoded, with its alternative text as the spoken label.
struct ChatImageSurface: View {
    let resource: String
    let label: String
    @ObservedObject var bridge: MobileBridge
    @State private var image: UIImage?

    var body: some View {
        Group {
            if let image {
                Image(uiImage: image).resizable().scaledToFill()
            } else {
                Color(uiColor: NativeChatPalette.raised)
            }
        }
        .frame(width: 96, height: 72)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous)
            .strokeBorder(Color(uiColor: NativeChatPalette.border), lineWidth: 0.5))
        .accessibilityElement()
        .accessibilityLabel(label)
        .accessibilityAddTraits(.isImage)
        .accessibilityIdentifier(resource)
        .task(id: resource) { bridge.image(resource) { image = $0 } }
    }

    /// A picked photo as the PNG or JPEG Rust accepts: kept as is when it
    /// already is one and fits, else re-encoded as a JPEG at most 4096
    /// pixels a side.
    static func encoded(_ data: Data) -> (name: String, data: Data)? {
        let png: [UInt8] = [0x89, 0x50, 0x4E, 0x47]
        let jpeg: [UInt8] = [0xFF, 0xD8, 0xFF]
        let head = [UInt8](data.prefix(4))
        guard let image = UIImage(data: data) else { return nil }
        let side = max(image.size.width * image.scale, image.size.height * image.scale)
        if side <= 4096, data.count <= 8 * 1024 * 1024 {
            if head.starts(with: png) { return ("Photo.png", data) }
            if head.starts(with: jpeg) { return ("Photo.jpg", data) }
        }
        let scale = min(1, 4096 / max(side, 1))
        let size = CGSize(width: floor(image.size.width * image.scale * scale),
                          height: floor(image.size.height * image.scale * scale))
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let resized = UIGraphicsImageRenderer(size: size, format: format).image { _ in
            image.draw(in: CGRect(origin: .zero, size: size))
        }
        for quality in [0.85, 0.7, 0.5] {
            if let encoded = resized.jpegData(compressionQuality: quality), encoded.count <= 8 * 1024 * 1024 {
                return ("Photo.jpg", encoded)
            }
        }
        return nil
    }
}
