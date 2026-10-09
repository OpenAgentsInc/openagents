// Forwards requests to the Rust app on one serial queue and publishes its
// packets. Rust owns every screen, grant, and connection; this bridge only
// opens URLs Rust names and collects values Rust asks for.
import SwiftUI

/// A value Rust asks the host to collect. Rust validates it.
struct ComputersInput: Decodable, Equatable {
    let token: String
    let label: String
    let prompt: String
    let scan: Bool
    /// Mask the field and never echo, log, or keep the value.
    let secret: Bool
    let max_bytes: Int

    private enum CodingKeys: String, CodingKey { case token, label, prompt, scan, secret, max_bytes }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        token = try values.decode(String.self, forKey: .token)
        label = try values.decode(String.self, forKey: .label)
        prompt = try values.decode(String.self, forKey: .prompt)
        scan = try values.decode(Bool.self, forKey: .scan)
        secret = try values.decodeIfPresent(Bool.self, forKey: .secret) ?? false
        max_bytes = try values.decode(Int.self, forKey: .max_bytes)
    }
}

/// An invitation QR code: one string of `1` (dark) and `0` per module row.
struct ComputersQR: Decodable, Equatable {
    let size: Int
    let rows: [String]
}

/// **Connect a computer** (`SCR-22`) or **Connected** (`SCR-23`). Rust
/// writes every word and decides what a code is.
struct ConnectScreen: Decodable, Equatable {
    /// `scan`, `connecting`, or `connected`.
    let stage: String
    let title: String
    let prompt: String?
    let paste: String?
    let get_app: String?
    let notice: String?
    let computer: String?
    let done: String?
    let max_bytes: Int
    /// Computers on this Wi-Fi, above the camera, while it scans.
    let nearby: Nearby?
    /// The six-digit code to compare with the computer's.
    let code: String?

    struct Nearby: Decodable, Equatable {
        struct Row: Decodable, Equatable, Hashable {
            let id: String
            let label: String
        }
        let title: String
        let computers: [Row]
        let empty: String?
    }
}

/// The native Computers list, while Coder's shared Computers screens are on
/// their list. Rust builds every row and checks every choice.
struct ComputersHome: Decodable, Equatable {
    struct Item: Decodable, Equatable, Hashable {
        let choice: String
        let label: String
        /// For a destructive choice, the question to ask before sending it.
        let confirm: String?
    }

    struct Row: Decodable, Equatable, Identifiable {
        let host: String
        let name: String
        let status: String
        /// `online`, `pending`, `offline`, or `alert`.
        let tone: String
        /// "1 background watcher · disk cleanup" for an online computer
        /// that runs any; otherwise absent.
        let watchers: String?
        /// What its background rules last did, in one line, while online.
        let background: String?
        let menu: [Item]
        var id: String { host }
    }

    let rows: [Row]
    let empty: String?
    let notice: String?
    let owner_key: Bool
    let keep_directory: Bool
    /// Opens Connect a computer, the scanner.
    let connect: String
    /// Coder's other ways to add a computer.
    let add_other: String
}

/// A paired computer Everglade's studio can act through: its host key and
/// its name on the Computers list.
struct StudioComputer: Equatable {
    let host: String
    let name: String
}

/// Account > Your keys (BYOK, #10176), as Rust shows it. Never a key: each
/// row has its last four characters and its last test.
struct ProviderKeysState: Decodable, Equatable {
    struct Input: Decodable, Equatable {
        /// Always `provider_key`: a secure, never-echoed field.
        let purpose: String
        let secret: Bool
        let label: String
        let prompt: String
        let max_bytes: Int
    }
    struct Row: Decodable, Equatable, Identifiable {
        let provider: String
        let name: String
        let last_four: String?
        let state: String?
        let line: String?
        let checking: Bool
        let page: String
        let input: Input
        var id: String { provider }
    }
    /// A key this host collected was tested: keep it or drop it.
    struct Done: Decodable, Equatable {
        let provider: String
        let kept: Bool
        let ask_mine: Bool
    }
    let rows: [Row]
    let mine: Bool
    let mine_blocked: String?
    let status: String
    let notice: String?
    let done: [Done]
}

struct AppPacket: Decodable {
    let schema: String
    let device: String
    let device_npub: String
    let computers: NativeView?
    let computers_home: ComputersHome?
    let computers_input: ComputersInput?
    let computers_qr: ComputersQR?
    let coder: NativeView?
    /// The phone's shell (#11126): the Chat / Code switch, the feature
    /// cards, and the drawer's rows while it is open.
    let shell: ShellState?
    /// The chat's link cards by surface resource (#11126).
    let links: [String: LinkCard]?
    /// The open Coder chat changes on its own; ask for a packet sooner.
    let coder_live: Bool?
    /// A basic Coder reply is streaming; ask for packets every few hundred
    /// milliseconds.
    let chat_streaming: Bool?
    /// Show another tab's screen once: `computers` is Account > Computers.
    let coder_go: String?
    /// The chat takes images. Off since #10093: the phone is text only, so
    /// the photo picker never mounts and nothing is sent to attach.
    let attachments: Bool?
    /// Connect a computer, while it shows.
    let connect: ConnectScreen?
    let tailnet: NativeView?
    let tailnet_loading: Bool
    let open_url: String?
    let terminal: Bool
    let notices: [String]
    let wallet: WalletState?
    let wallet_loading: Bool?
    /// A bitcoin purchase page to open once.
    let wallet_open_url: String?
    /// Agents' payment requests (`spend::View`).
    let spend: SpendState?
    /// A computer's ask for the wallet (`wallet_link::View`).
    let wallet_link: WalletLinkState?
    /// How bitcoin amounts show and are typed, app-wide (BIP 177 or BTC).
    let amounts: AmountsState?
    /// Push wake status, present once push is configured or requested.
    let push: String?
    /// The Gym in chat: which of the Chat tab's screens shows, the cards
    /// the chat's surfaces name, the open sheet, and a share sheet to open.
    let gym: GymSlot?
    /// Account > Your keys (BYOK).
    let provider_keys: ProviderKeysState?
    /// The theme: the choice, the resolved scheme, and the chrome palette.
    let appearance: AppearanceState?
    /// The account surface: sign-in, the account's chats, and Running
    /// (#11107, #11165).
    let link: LinkPacket?

    /// The Gym's part, when it decodes.
    var gymPacket: GymPacket? { gym?.value }
}

/// Rust's direct reply with the recovery words or a checked restore. It is
/// never part of the app packet, and this host keeps none of it.
struct WalletSecretPacket: Decodable {
    let schema: String
    let words: [String]?
    let entropy_hex: String?
    let error: String?
}

/// Rust's direct reply with the wallet's exit backup as a file. It is never
/// part of the app packet.
struct WalletFilePacket: Decodable {
    let schema: String
    let file_name: String?
    let text: String?
    let error: String?
}

struct WalletExportError: Error { let message: String }

/// One TestFlight build, what it brought, and where to test it.
struct Release: Decodable, Hashable {
    struct Item: Decodable, Hashable {
        let title: String
        let detail: String
    }
    let version: String
    let build: String
    let title: String
    let what_to_test: String
    let items: [Item]
}

/// This device's identity keys and the changelog. `nsec` is present only in
/// the answer to an explicit reveal.
struct AccountPacket: Decodable {
    let schema: String
    let npub: String
    let public_hex: String
    let origin: String
    let changelog: [Release]
    let nsec: String?
    /// The name over the player's head in the Verse; absent until set.
    let display_name: String?
}

/// One counted award on the trainer card.
struct TrainerAward: Decodable, Hashable {
    let title: String
    let quest: String
    let season: String
    let rule: String
    let role: String
    let xp: UInt64
    let award: String
    let link: String
}

/// The signed trainer card as a JSON file and its public link.
struct CardExport: Decodable {
    let schema: String
    let file_name: String?
    let json: String?
    let link: String?
    let preview: Bool
    let error: String?
}

/// A key the trainer profile lists: `linked` once it signed a link back.
struct LinkedKey: Decodable, Hashable {
    let npub: String
    let public_hex: String
    let status: String
}

/// The Account tab's trainer card for the Verse world key: Rust derives the
/// level from signed NIP-XP awards; this only shows it.
struct TrainerPacket: Decodable {
    let schema: String
    let npub: String
    let public_hex: String
    let tag: String
    let state: String
    let relay: String
    let referee_npub: String
    let curve: String
    let xp: UInt64
    let level: UInt32
    let next_level_at: UInt64
    let to_next: UInt64
    let titles: [String]
    let awards: [TrainerAward]
    let open_quests: Int
    let note: String
    let profile: String
    let profile_status: String
    let profile_error: String?
    let linked_keys: [LinkedKey]
    let linked_to: String?
    let card_status: String
    let playtest: PlaytestXP
    let nsec: String?
}

/// The Account playtest card: playtest XP from the separate playtest
/// referee, never summed into the trainer level.
struct PlaytestXP: Decodable {
    let state: String
    let referee_npub: String?
    let xp: UInt64
    let titles: [String]
    let accepted_reports: Int
    let fixes_verified: Int
    let sessions: Int
    let diaries: Int
    let awards: [TrainerAward]
    let note: String
}

/// A report kind the form offers, with Rust's words for it.
struct ReportKind: Decodable, Hashable {
    let value: String
    let label: String
    let hint: String
}

/// The Report a problem form for the screen the tester is on. Rust decides
/// whether a screenshot may be offered and holds the playtest log.
struct ReportDraft: Decodable {
    let schema: String
    let tab: String
    let route: String
    let screenshot_allowed: Bool
    let triage_ready: Bool
    let task: String?
    let logging: Bool
    let log_lines: [String]
    let log_digest: String
    /// The open chat with OpenAgents, one line per message, as Share this
    /// chat would send it; empty when none is open.
    let chat_lines: [String]?
    let chat_digest: String?
    let kinds: [ReportKind]
    let privacy: String
    let fallback: String
}

/// One report in My reports.
struct ReportRow: Decodable, Hashable {
    let id: String
    let code: String?
    let kind: String
    let kind_label: String
    let at: UInt64
    let build: String
    let place: String
    let summary: String
    let status: String
    let status_label: String
    let error: String?
    let screenshot: Bool
    let log: Bool
    /// The public, content-free record of the report is on the relay.
    let published: Bool?
}

/// Playtest logging's state: on unless this build turned it off.
struct PlaytestLogRow: Decodable {
    let on: Bool
    /// The line the Playtest screen shows about it.
    let note: String
    let started_at: UInt64?
    let events: Int
    let lines: [String]
}

/// My reports and the playtest log, with this request's result.
struct ReportsPacket: Decodable {
    let schema: String
    let triage_ready: Bool
    let log: PlaytestLogRow
    let reports: [ReportRow]
    let sent: ReportRow?
    let error: String?
    let fallback: String
    /// What Give feedback's dialog says once it filed: Sent, or saved.
    let feedback: String?
}

struct TerminalPacket: Decodable {
    let schema: String
    let open: Bool
    let revision: UInt64
    let view: NativeView?
    let paste: Bool
}

/// A request from the Coder tab to show another screen.
struct ScreenRequest: Equatable {
    let screen: String
    let serial: Int
}

@MainActor
final class MobileBridge: ObservableObject {
    @Published private(set) var packet: AppPacket?
    @Published private(set) var terminalView: NativeView?
    @Published private(set) var hudFeedRevision: UInt64 = 0
    @Published var gpuTerminalVisible = false
    private(set) var hudFeed: [String: Any] = [:]
    private var hudBytes: [String: Data] = [:]

    private func updateHud(_ name: String, value: Any?) {
        guard let value, !(value is NSNull), JSONSerialization.isValidJSONObject(value),
              let bytes = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]) else {
            if hudFeed.removeValue(forKey: name) != nil { hudBytes.removeValue(forKey: name); hudFeedRevision &+= 1 }
            return
        }
        if hudBytes[name] != bytes { hudBytes[name] = bytes; hudFeed[name] = value; hudFeedRevision &+= 1 }
    }

    func showNativeComputers() { computersRequested += 1 }

    func activateHud(_ command: [String: Any]) {
        guard let surface = command["surface"] as? String, ["computers", "terminal"].contains(surface),
              let instance = command["instance"] as? String, let revision = command["revision"] as? NSNumber,
              let node = command["node"] as? String else { return }
        send(["op": "\(surface)_activate", "instance": instance, "revision": revision, "node": node])
    }
    @Published private(set) var failure: String?
    @Published private(set) var pending = 0
    /// Counts the Coder tab's requests to open Account > Computers.
    @Published private(set) var computersRequested = 0
    /// The Coder tab's last request to open another screen (`wallet`,
    /// `keys`, `playtest`, or `report`), numbered so a repeat still shows.
    @Published private(set) var screenRequest = ScreenRequest(screen: "", serial: 0)
    /// Counts the Chat tab's requests for the photo picker (`pick_image`),
    /// only while the packet says the chat takes images.
    @Published private(set) var imagePickRequested = 0
    /// The chat takes images (`attachments` in the packet; off since
    /// #10093, when the phone became text only).
    var attachmentsEnabled: Bool { packet?.attachments == true }
    /// Decoded images for the chat's `image:` surfaces, by resource.
    private var images: [String: UIImage] = [:]
    private let queue = DispatchQueue(label: "com.openagents.app.rust")
    private nonisolated(unsafe) let handle: UnsafeMutableRawPointer?
    private var terminalRevision: UInt64 = 0
    private var terminalPolling = false
    /// A packet asked for after Rust said it changed is on its way, and
    /// whether Rust changed again since it was asked for.
    private var changeInFlight = false
    private var changedAgain = false

    var busy: Bool { pending > 0 }

    init() {
        do {
            let secret = try DeviceKey.loadOrCreate()
            var options: [String: Any] = [
                "state_dir": try DeviceKey.stateDirectory().path,
                "secret_hex": secret.map { String(format: "%02x", $0) }.joined(),
                // This host draws the Computers list and its navigation.
                "native_computers": true,
                // Its transcript layout reads chat rows from Rust.
                "pulled_transcripts": true,
                // It draws the shell: the top bar, the drawer, the cards.
                "shell": true,
                // The chat router's context names the build.
                "app_build": "\(ReportDevice.version) (\(ReportDevice.build))",
            ]
            // The iroh key beside the device key: connecting a computer
            // dials it over iroh. Without it, pairing uses the relay only.
            if let iroh = try? DeviceKey.loadOrCreateIroh() {
                options["iroh_secret_hex"] = iroh.map { String(format: "%02x", $0) }.joined()
            }
            // Push stays off unless this build names a relay and a gateway.
            if let push = PushSettings.configured { options["push"] = push.rust }
            // Simulator screenshots: an offline wallet with no money.
            if AppTabLaunch.wallet("--wallet-fixture") != nil { options["wallet_fixture"] = true }
            // Simulator screenshots: an offline chat worker that sends the
            // chat router's offers and follow-ups (`--chat-fixture 1`).
            if AppTabLaunch.wallet("--chat-fixture") != nil { options["chat_fixture"] = true }
            // `--chat-script "Who are you?|!wrong"` plays those steps in a new chat.
            if let script = AppTabLaunch.wallet("--chat-script") {
                options["chat_script"] = script.split(separator: "|").map(String.init)
            }
            // `--gym-first-run choose|end_card|chat|done` starts the first
            // run at that step.
            if let step = AppTabLaunch.wallet("--gym-first-run") { options["gym_first_run"] = step }
            // `--gym-fixture 1`: the chat router's recorded Gym cards and a
            // recorded test run, offline, for screenshots (debug builds only).
            if AppTabLaunch.wallet("--gym-fixture") != nil { options["gym_fixture"] = true }
            // `--account-origin https://staging.openagents.com`: sign in to
            // another site (debug builds only; release is openagents.com).
            if let origin = AppTabLaunch.wallet("--account-origin") { options["account_origin"] = origin }
            let configuration = try JSONSerialization.data(withJSONObject: options)
            handle = configuration.withUnsafeBytes { bytes in
                openagents_mobile_create(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
            }
            if handle == nil { failure = "OpenAgents could not start." }
        } catch {
            handle = nil
            failure = error.localizedDescription
        }
        send(["op": "snapshot"])
        gymWorld()
        providerKeysLoad()
        linkHello()
        LinkNotifier.shared.install(self)
        if handle != nil { watchChanges() }
    }

    /// Hand Rust the trainer's Verse world key: it names the trainer on the
    /// menu, reads their XP, and signs their test requests. Rust keeps it in
    /// memory only.
    func gymWorld() {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        send(["op": "gym_world", "world_secret_hex": secret.map { String(format: "%02x", $0) }.joined()])
    }

    /// Keys the person entered that wait for their test, by provider. Held
    /// in memory only until Rust says to keep or drop them.
    private var pendingKeys: [String: String] = [:]
    /// Ask "Use your keys for everything?" after a key was added.
    @Published var askMine = false
    /// A key could not be saved in Keychain.
    @Published var providerKeyError: String?

    /// Hand Rust the person's own provider keys from Keychain and the saved
    /// switch (BYOK). Rust keeps them in memory only.
    func providerKeysLoad() {
        send(["op": "provider_keys", "keys": ProviderKeyStore.all(), "mine": ProviderKeyStore.mine]) {
            self.providerKeysLoaded = true
        }
    }
    /// Rust holds the saved keys: from here on its switch is the one saved.
    private var providerKeysLoaded = false

    /// A key from the secure field: Rust tests it, and it goes to Keychain
    /// only once the provider accepts it.
    func providerKeyAdd(_ provider: String, key: String) {
        let key = key.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !key.isEmpty else { return }
        pendingKeys[provider] = key
        send(["op": "provider_key_add", "provider": provider, "key": key])
    }

    func providerKeyTest(_ provider: String) { send(["op": "provider_key_test", "provider": provider]) }

    func providerKeyRemove(_ provider: String) {
        ProviderKeyStore.delete(provider)
        send(["op": "provider_key_remove", "provider": provider])
    }

    func providerKeysMine(_ on: Bool) { send(["op": "provider_keys_mine", "on": on]) }

    /// Keep or drop the keys whose tests finished, and save the switch.
    private func settleProviderKeys(_ state: ProviderKeysState?) {
        guard let state else { return }
        for done in state.done {
            guard let key = pendingKeys.removeValue(forKey: done.provider) else { continue }
            if done.kept {
                let saved = ProviderKeyStore.save(done.provider, key: key)
                providerKeyError = saved ? nil : "Could not save the key in Keychain. It works until OpenAgents restarts."
                if done.ask_mine { askMine = true }
            }
        }
        if providerKeysLoaded, ProviderKeyStore.mine != state.mine { ProviderKeyStore.mine = state.mine }
    }

    /// A tap on a Gym button, by the ID Rust gave it.
    func gym(_ id: String) { send(["op": "gym", "id": id]) }

    /// Train Coder, from the Verse's Gym board or Account: Rust opts into
    /// the Gym and asks for the Chat tab (`coder_go` is `chat`).
    func gymTrain() { send(["op": "gym_train"]) }

    /// Profile, from Account: Rust shows the Profile sheet on the Chat tab
    /// (`coder_go` is `chat`).
    func profile() { send(["op": "profile"]) }

    /// A shell action (`coder_tab::ShellAction`): the switch, the drawer,
    /// a recent chat, See all, or a card's Try it.
    /// An action on the account surface (`account_link::Action`).
    func link(_ action: String, _ fields: [String: Any] = [:], done: (() -> Void)? = nil) {
        var link = fields
        link["action"] = action
        send(["op": "link", "link": link], done: done)
    }

    func shell(_ action: String, _ fields: [String: Any] = [:]) {
        var shell = fields
        shell["action"] = action
        send(["op": "shell", "shell": shell])
    }

    /// Text for the system share sheet, which Rust asked to open.
    @Published var gymShare: String?

    /// Rust says when its packet changes (a transcript page, a streamed
    /// reply, a computer's task summary): a thread of its own waits on it
    /// and asks for the packet at once, instead of on a timer.
    private func watchChanges() {
        let thread = Thread { [weak self] in
            var seen: UInt64 = 0
            while true {
                let now = openagents_mobile_wait(seen, 30_000)
                guard self != nil else { return }
                if now == seen { continue }
                seen = now
                DispatchQueue.main.async { self?.rustChanged() }
            }
        }
        thread.name = "com.openagents.app.changes"
        thread.qualityOfService = .userInitiated
        thread.start()
    }

    /// Ask for the changed packet, one at a time: a change while one is on
    /// its way asks again once it arrives, so the last change always shows.
    private func rustChanged() {
        if changeInFlight { changedAgain = true; return }
        changeInFlight = true
        changedAgain = false
        send(["op": "changed"]) { [weak self] in
            guard let self else { return }
            self.changeInFlight = false
            if self.changedAgain { self.rustChanged() }
        }
    }

    /// The Coder tab shows or hides: while it shows a live chat, Rust asks
    /// for a packet every second.
    func coderShown(_ shown: Bool) { openagents_mobile_coder_shown(shown) }

    deinit {
        let handle = handle
        queue.async { openagents_mobile_destroy(handle) }
    }

    func lifecycle(_ active: Bool) { send(["op": "lifecycle", "active": active]) }

    /// The theme's chrome colors and scheme, from Rust; the dark look
    /// until the first packet.
    var colors: AppColors { packet?.appearance.map(AppColors.init) ?? .dark }

    /// The appearance last reported, so an unchanged one is not sent again.
    private var reportedDark: Bool?

    /// The phone's own appearance, which the System theme follows: at
    /// launch, on coming to the front, and when it changes.
    func reportSystemAppearance() {
        let dark = SystemAppearance.isDark
        guard dark != reportedDark else { return }
        reportedDark = dark
        send(["op": "system_appearance", "dark": dark])
    }

    /// Account > Appearance: `system`, `light`, or `dark`. Rust saves it.
    func chooseTheme(_ id: String) {
        reportSystemAppearance()
        send(["op": "theme", "theme": id])
    }
    func refreshComputers() { send(["op": "computers_refresh"]) }
    func refreshTailnet() { send(["op": "tailnet_refresh"]) }
    func snapshot() { send(["op": "snapshot"]) }

    func activate(_ surface: String, view: NativeView, node: String) {
        send(["op": "\(surface)_activate", "instance": view.instance, "revision": view.revision, "node": node])
    }

    /// The paired computer Everglade's studio acts through: the first
    /// computer the Computers list shows online, with its name.
    var studioComputer: StudioComputer? {
        packet?.computers_home?.rows.first { $0.tone == "online" }
            .map { StudioComputer(host: $0.host, name: $0.name) }
    }

    /// Connects the Verse world's Everglade studio to the paired computer
    /// `host` (`openagents_verse_studio_connect`). The call needs the app
    /// handle with no other call running and the Verse handle on the main
    /// thread, so it holds this bridge's queue and runs on the main thread.
    /// `verse` answers the world's live handle then, or nil once it is gone.
    /// `done` gets Rust's JSON reply on the main thread, or nil.
    func studioConnect(host: String, verse: @escaping () -> UnsafeMutableRawPointer?,
                       done: @escaping (Data?) -> Void) {
        let key = Data(host.utf8)
        guard let handle, !key.isEmpty, key.count <= 256 else { return done(nil) }
        queue.async {
            let data: Data? = DispatchQueue.main.sync {
                MainActor.assumeIsolated {
                    guard let world = verse() else { return nil }
                    return key.withUnsafeBytes { bytes -> Data? in
                        let buffer = openagents_verse_studio_connect(
                            world, handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
                        defer { openagents_mobile_buffer_free(buffer) }
                        guard let pointer = buffer.data, buffer.len > 0 else { return nil }
                        return Data(bytes: pointer, count: buffer.len)
                    }
                }
            }
            DispatchQueue.main.async { done(data) }
        }
    }

    /// Hand Rust the Spark wallet's seed from Keychain. Rust ignores a
    /// repeat. The Mutinynet test wallet's key, which this replaced, is
    /// deleted.
    func openWallet() {
        DeviceKey.deleteMutinynetWallet()
        guard let entropy = try? DeviceKey.loadOrCreateSpark() else { return }
        send(["op": "wallet_open", "entropy_hex": entropy.map { String(format: "%02x", $0) }.joined()])
    }
    func refreshWallet() { send(["op": "wallet_refresh"]) }
    /// An agent payment request's answer: `spend_approve` or `spend_deny`
    /// with `request`, `spend_block` or `spend_allow` with `host`, or
    /// `spend_dismiss`. Rust checks every field.
    /// Why this device couldn't obtain a push token, if it couldn't.
    @Published private(set) var pushFailure: String?
    /// Push wake status: the native failure, or Rust's.
    var pushStatus: String? { pushFailure ?? packet?.push }
    func pushFailed(_ reason: String) { pushFailure = reason }
    /// The platform issued a push token, as lowercase hex. Rust registers it.
    func pushToken(_ token: String) {
        pushFailure = nil
        send(["op": "push_token", "token": token])
    }

    func spend(_ op: String, _ fields: [String: Any] = [:]) {
        var request = fields
        request["op"] = op
        send(request)
    }
    /// A Wallet request whose fields Rust checks.
    func wallet(_ op: String, _ fields: [String: Any] = [:]) {
        var request = fields
        request["op"] = op
        send(request)
    }

    /// The recovery words, for a sheet the person asked to see after a
    /// warning. Nothing here keeps or logs them.
    func walletWords(received: @escaping ([String]) -> Void) {
        call(["op": "wallet_words"]) { data in
            guard let packet = try? JSONDecoder().decode(WalletSecretPacket.self, from: data),
                  packet.schema == "openagents.wallet-secret.v1", let words = packet.words else { return }
            received(words)
        }
    }

    /// The saved unilateral-exit backup, for a Files export the person
    /// asked for: its file name and text, or why there is none.
    func walletExitExport(received: @escaping (Result<(String, String), WalletExportError>) -> Void) {
        call(["op": "wallet_exit_export"]) { data in
            guard let packet = try? JSONDecoder().decode(WalletFilePacket.self, from: data),
                  packet.schema == "openagents.wallet-file.v1" else {
                received(.failure(WalletExportError(message: "The backup could not be read.")))
                return
            }
            if let name = packet.file_name, let text = packet.text {
                received(.success((name, text)))
            } else {
                received(.failure(WalletExportError(message: packet.error ?? "The backup could not be read.")))
            }
        }
    }

    /// Restore from recovery words: Rust checks them, this saves the seed in
    /// Keychain, and Rust replaces the running wallet. `done` gets the
    /// reason on failure.
    func restoreWallet(words: String, done: @escaping (String?) -> Void) {
        call(["op": "wallet_restore_check", "words": words]) { data in
            guard let packet = try? JSONDecoder().decode(WalletSecretPacket.self, from: data),
                  packet.schema == "openagents.wallet-secret.v1" else {
                done("The words could not be checked.")
                return
            }
            guard let hex = packet.entropy_hex, let entropy = Data(hexString: hex) else {
                done(packet.error ?? "The words could not be checked.")
                return
            }
            do {
                try DeviceKey.replaceSpark(entropy)
            } catch {
                done(error.localizedDescription)
                return
            }
            self.send(["op": "wallet_open", "entropy_hex": hex, "replace": true])
            done(nil)
        }
    }

    /// Open a computer from the native Computers list.
    func openComputer(_ host: String) { send(["op": "computers_open", "host": host]) }
    /// A choice from a Computers row's menu, already confirmed when it asks.
    func chooseComputer(_ host: String, _ choice: String) {
        send(["op": "computers_choose", "host": host, "choice": choice])
    }
    /// `home`, `add`, `activity`, `owner_key`, `keep_directory`, or `refresh`.
    func computersGo(_ destination: String) { send(["op": "computers_go", "to": destination]) }
    /// Connect a computer: open the scanner, hand Rust a code, or close.
    func connectOpen() { send(["op": "connect_open"]) }
    func connectCode(_ value: String) { send(["op": "connect_code", "value": value]) }
    /// The app was opened with a connect link (the desktop app's QR code,
    /// read by the system camera): show Connect a computer and pair with it.
    func connectLink(_ value: String) { send(["op": "connect_link", "value": value]) }
    func connectClose() { send(["op": "connect_close"]) }
    func connectNearby(_ id: String) { send(["op": "connect_nearby", "id": id]) }

    /// Answer or close an input request, or send from a composer.
    func submit(_ surface: String, token: String, value: String) {
        send(["op": "\(surface)_input", "token": token, "value": value])
    }
    func cancel(_ surface: String, token: String) { send(["op": "\(surface)_cancel", "token": token]) }

    /// This device's keys and the changelog. With `reveal`, the answer also
    /// carries the nsec: ask for it only after the person chose to see it,
    /// and keep it no longer than it shows. Nothing here logs it.
    func account(reveal: Bool = false, received: @escaping (AccountPacket) -> Void) {
        call(["op": "account", "reveal": reveal]) { data in
            guard let packet = try? JSONDecoder().decode(AccountPacket.self, from: data),
                  packet.schema == "openagents.account.v1" else { return }
            received(packet)
        }
    }

    /// Sets the display name shown over the player's head in the Verse; an
    /// empty name clears it. The answer is the account packet with the name
    /// as the app cleaned it, which is also kept for the Verse tab.
    func setDisplayName(_ name: String, received: @escaping (AccountPacket) -> Void) {
        call(["op": "set_display_name", "name": name]) { data in
            guard let packet = try? JSONDecoder().decode(AccountPacket.self, from: data),
                  packet.schema == "openagents.account.v1" else { return }
            UserDefaults.standard.set(packet.display_name, forKey: VerseWorld.displayNameKey)
            received(packet)
        }
    }

    /// The trainer card for the Verse world key, the key over this player's
    /// head in the Grid. With `reveal`, the answer also carries that key's
    /// nsec: ask for it only after the person chose to see it. Nothing here
    /// logs it.
    func trainer(reveal: Bool = false, received: @escaping (TrainerPacket) -> Void) {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        let hex = secret.map { String(format: "%02x", $0) }.joined()
        call(["op": "trainer", "world_secret_hex": hex, "reveal": reveal,
              "preview": AppTabLaunch.xpPreview]) { data in
            guard let packet = try? JSONDecoder().decode(TrainerPacket.self, from: data),
                  packet.schema == "openagents.trainer.v1" else { return }
            received(packet)
        }
    }

    /// Publish the trainer profile after the person confirms: `shown`
    /// puts their level over their head in other players' Grids.
    func trainerProfile(shown: Bool, received: @escaping (TrainerPacket) -> Void) {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        let hex = secret.map { String(format: "%02x", $0) }.joined()
        call(["op": "trainer_profile", "world_secret_hex": hex, "shown": shown]) { data in
            guard let packet = try? JSONDecoder().decode(TrainerPacket.self, from: data),
                  packet.schema == "openagents.trainer.v1" else { return }
            received(packet)
        }
    }

    /// Sign the trainer card and publish it at its address, after the
    /// person confirms.
    func trainerExport(received: @escaping (CardExport) -> Void) {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        let hex = secret.map { String(format: "%02x", $0) }.joined()
        call(["op": "trainer_export", "world_secret_hex": hex]) { data in
            guard let reply = try? JSONDecoder().decode(CardExport.self, from: data),
                  reply.schema == "openagents.trainer-card.v1" else { return }
            received(reply)
        }
    }

    /// Add a key to, or remove one from, the trainer profile, and publish it.
    func trainerLink(add: String? = nil, remove: String? = nil,
                     received: @escaping (TrainerPacket) -> Void) {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        let hex = secret.map { String(format: "%02x", $0) }.joined()
        var request: [String: Any] = ["op": "trainer_link", "world_secret_hex": hex]
        if let add { request["add"] = add }
        if let remove { request["remove"] = remove }
        call(request) { data in
            guard let packet = try? JSONDecoder().decode(TrainerPacket.self, from: data),
                  packet.schema == "openagents.trainer.v1" else { return }
            received(packet)
        }
    }

    /// The Report a problem form for `tab` and `route`.
    func reportDraft(tab: String, route: String, received: @escaping (ReportDraft) -> Void) {
        call(["op": "report_draft", "tab": tab, "route": route]) { data in
            guard let packet = try? JSONDecoder().decode(ReportDraft.self, from: data),
                  packet.schema == "openagents.report-draft.v1" else { return }
            received(packet)
        }
    }

    /// File a report, signed by the Verse world key. Rust checks every
    /// field, refuses a screenshot from the Wallet or a key screen, and
    /// seals it to the triage key.
    func sendReport(_ form: [String: Any], received: @escaping (ReportsPacket) -> Void) {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        let hex = secret.map { String(format: "%02x", $0) }.joined()
        call(["op": "report_send", "world_secret_hex": hex, "form": form]) { data in
            guard let packet = try? JSONDecoder().decode(ReportsPacket.self, from: data),
                  packet.schema == "openagents.reports.v1" else { return }
            received(packet)
        }
    }

    /// Give feedback on selected text (#10127), signed by the Verse world
    /// key. Rust adds where the text came from and seals it to the triage
    /// key as a report.
    func sendFeedback(_ form: [String: Any], received: @escaping (ReportsPacket) -> Void) {
        guard let secret = try? DeviceKey.loadOrCreateVerse() else { return }
        let hex = secret.map { String(format: "%02x", $0) }.joined()
        call(["op": "feedback_send", "world_secret_hex": hex, "form": form]) { data in
            guard let packet = try? JSONDecoder().decode(ReportsPacket.self, from: data),
                  packet.schema == "openagents.reports.v1" else { return }
            received(packet)
        }
    }

    /// My reports; reports that wait or failed are sent again.
    func reports(received: @escaping (ReportsPacket) -> Void) {
        let hex = (try? DeviceKey.loadOrCreateVerse())?.map { String(format: "%02x", $0) }.joined() ?? ""
        call(["op": "reports", "world_secret_hex": hex]) { data in
            guard let packet = try? JSONDecoder().decode(ReportsPacket.self, from: data),
                  packet.schema == "openagents.reports.v1" else { return }
            received(packet)
        }
    }

    /// Delete the playtest log; logging goes on recording.
    func playtestClear(received: @escaping (ReportsPacket) -> Void) {
        call(["op": "playtest_clear"]) { data in
            guard let packet = try? JSONDecoder().decode(ReportsPacket.self, from: data),
                  packet.schema == "openagents.reports.v1" else { return }
            received(packet)
        }
    }

    /// Where the tester is, for the playtest log; Rust records it unless
    /// this build turned playtest logging off.
    func playtestScreen(tab: String, route: String) {
        send(["op": "playtest_screen", "tab": tab, "route": route])
    }

    /// A terminal request: a resize, typed text, a key, or a paste.
    func terminal(_ request: [String: Any]) {
        call(request) { data in
            guard let packet = try? JSONDecoder().decode(TerminalPacket.self, from: data) else { return }
            self.receiveTerminal(packet, raw: (try? JSONSerialization.jsonObject(with: data)) as? [String: Any])
        }
    }

    /// Poll the open terminal; skipped while a poll is in flight.
    func pollTerminal() {
        guard !terminalPolling else { return }
        terminalPolling = true
        call(["op": "terminal_poll", "known": terminalRevision]) { data in
            self.terminalPolling = false
            guard let packet = try? JSONDecoder().decode(TerminalPacket.self, from: data) else { return }
            self.receiveTerminal(packet, raw: (try? JSONSerialization.jsonObject(with: data)) as? [String: Any])
        }
    }

    private func receiveTerminal(_ packet: TerminalPacket, raw: [String: Any]?) {
        guard packet.open else {
            terminalView = nil
            updateHud("terminal", value: nil)
            terminalRevision = 0
            return
        }
        if let view = packet.view, packet.revision > terminalRevision {
            terminalRevision = packet.revision
            terminalView = view
            updateHud("terminal", value: raw?["view"])
        }
        if packet.paste {
            terminal(["op": "terminal_paste", "text": UIPasteboard.general.string ?? ""])
        }
    }

    private func send(_ request: [String: Any], done: (() -> Void)? = nil) {
        call(request, finished: done) { data in
            guard let packet = try? JSONDecoder().decode(AppPacket.self, from: data),
                  packet.schema == "openagents.mobile.v1" else {
                self.failure = "OpenAgents returned an unreadable screen."
                return
            }
            self.updateHud("computers", value: ((try? JSONSerialization.jsonObject(with: data)) as? [String: Any])?["computers"])
            self.packet = packet
            self.settleProviderKeys(packet.provider_keys)
            self.settleLink(packet.link)
            if let share = packet.gymPacket?.share { self.gymShare = share }
            switch packet.coder_go {
            case "computers": self.computersRequested += 1
            case "pick_image" where packet.attachments == true: self.imagePickRequested += 1
            case let screen? where ["wallet", "keys", "playtest", "report", "verse_gym", "chat"].contains(screen):
                self.screenRequest = ScreenRequest(screen: screen, serial: self.screenRequest.serial + 1)
            default: break
            }
            if !packet.terminal { self.terminalView = nil; self.terminalRevision = 0 }
            if let link = packet.open_url, let url = URL(string: link), url.scheme == "https" {
                UIApplication.shared.open(url)
                self.send(["op": "tailnet_wait_for_sign_in"])
            }
            if let link = packet.wallet_open_url, let url = URL(string: link), url.scheme == "https" {
                UIApplication.shared.open(url)
            }
        }
    }

    /// Attach a photo's encoded bytes to the open chat's draft. Rust decodes
    /// and bounds them; the packet that answers shows the image's card.
    /// While the chat is text only (#10093) the image is dropped here.
    func attachImage(name: String, data: Data) {
        guard let handle, attachmentsEnabled else { return }
        pending += 1
        queue.async {
            let name = Array(name.utf8)
            let reply = data.withUnsafeBytes { bytes -> Data? in
                name.withUnsafeBufferPointer { name in
                    let buffer = openagents_mobile_attach_image(
                        handle, name.baseAddress, name.count,
                        bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
                    defer { openagents_mobile_buffer_free(buffer) }
                    guard let pointer = buffer.data, buffer.len > 0 else { return nil }
                    return Data(bytes: pointer, count: buffer.len)
                }
            }
            DispatchQueue.main.async {
                self.pending -= 1
                if let reply, let packet = try? JSONDecoder().decode(AppPacket.self, from: reply),
                   packet.schema == "openagents.mobile.v1" {
                    self.updateHud("computers", value: ((try? JSONSerialization.jsonObject(with: reply)) as? [String: Any])?["computers"])
                    self.packet = packet
                    self.settleProviderKeys(packet.provider_keys)
                }
            }
        }
    }

    /// The image an `image:` surface in the chat shows: the bytes Rust
    /// decoded and bounded, decoded again here for display and cached.
    func image(_ resource: String, received: @escaping (UIImage?) -> Void) {
        if let image = images[resource] { return received(image) }
        guard let handle else { return received(nil) }
        queue.async {
            let key = Array(resource.utf8)
            let data = key.withUnsafeBufferPointer { key -> Data? in
                let buffer = openagents_mobile_image(handle, key.baseAddress, key.count)
                defer { openagents_mobile_buffer_free(buffer) }
                guard let pointer = buffer.data, buffer.len > 0 else { return nil }
                return Data(bytes: pointer, count: buffer.len)
            }
            let image = data.flatMap { UIImage(data: $0)?.preparingThumbnail(of: CGSize(width: 480, height: 480)) }
            DispatchQueue.main.async {
                if let image {
                    if self.images.count > 16 { self.images.removeAll() }
                    self.images[resource] = image
                }
                received(image)
            }
        }
    }

    private func call(_ request: [String: Any], finished: (() -> Void)? = nil,
                      received: @escaping (Data) -> Void) {
        guard let handle, let body = try? JSONSerialization.data(withJSONObject: request) else {
            finished?()
            return
        }
        pending += 1
        queue.async {
            let data = body.withUnsafeBytes { bytes -> Data? in
                let buffer = openagents_mobile_call(handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
                defer { openagents_mobile_buffer_free(buffer) }
                guard let pointer = buffer.data, buffer.len > 0 else { return nil }
                return Data(bytes: pointer, count: buffer.len)
            }
            DispatchQueue.main.async {
                self.pending -= 1
                if let data { received(data) }
                finished?()
            }
        }
    }
}

extension Data {
    /// Bytes from lowercase or uppercase hex; nil for anything else.
    init?(hexString: String) {
        guard hexString.count.isMultiple(of: 2) else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(hexString.count / 2)
        var index = hexString.startIndex
        while index < hexString.endIndex {
            let next = hexString.index(index, offsetBy: 2)
            guard let byte = UInt8(hexString[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self.init(bytes)
    }
}
