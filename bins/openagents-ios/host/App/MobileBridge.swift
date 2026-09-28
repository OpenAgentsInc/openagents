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
        let menu: [Item]
        var id: String { host }
    }

    let rows: [Row]
    let empty: String?
    let notice: String?
    let owner_key: Bool
    let keep_directory: Bool
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
    /// The open Coder chat changes on its own; ask for a packet sooner.
    let coder_live: Bool?
    let chats: NativeView?
    let chats_input: ComputersInput?
    let chats_loading: Bool
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
    /// How bitcoin amounts show and are typed, app-wide (BIP 177 or BTC).
    let amounts: AmountsState?
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
/// whether a screenshot may be offered and holds the session log.
struct ReportDraft: Decodable {
    let schema: String
    let tab: String
    let route: String
    let screenshot_allowed: Bool
    let triage_ready: Bool
    let task: String?
    let session_on: Bool
    let session_lines: [String]
    let session_digest: String
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
    let session: Bool
    /// The public, content-free record of the report is on the relay.
    let published: Bool?
}

/// The playtest session's state.
struct PlaytestSessionRow: Decodable {
    let on: Bool
    let started_at: UInt64?
    let events: Int
    let lines: [String]
}

/// My reports and the session, with this request's result.
struct ReportsPacket: Decodable {
    let schema: String
    let triage_ready: Bool
    let session: PlaytestSessionRow
    let reports: [ReportRow]
    let sent: ReportRow?
    let error: String?
    let fallback: String
}

struct TerminalPacket: Decodable {
    let schema: String
    let open: Bool
    let revision: UInt64
    let view: NativeView?
    let paste: Bool
}

@MainActor
final class MobileBridge: ObservableObject {
    @Published private(set) var packet: AppPacket?
    @Published private(set) var terminalView: NativeView?
    @Published private(set) var failure: String?
    @Published private(set) var pending = 0
    private let queue = DispatchQueue(label: "com.openagents.app.rust")
    private nonisolated(unsafe) let handle: UnsafeMutableRawPointer?
    private var terminalRevision: UInt64 = 0
    private var terminalPolling = false

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
            ]
            // Simulator screenshots: an offline wallet with no money.
            if AppTabLaunch.wallet("--wallet-fixture") != nil { options["wallet_fixture"] = true }
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
    }

    deinit {
        let handle = handle
        queue.async { openagents_mobile_destroy(handle) }
    }

    func lifecycle(_ active: Bool) { send(["op": "lifecycle", "active": active]) }
    func refreshComputers() { send(["op": "computers_refresh"]) }
    func refreshTailnet() { send(["op": "tailnet_refresh"]) }
    func snapshot() { send(["op": "snapshot"]) }

    func activate(_ surface: String, view: NativeView, node: String) {
        send(["op": "\(surface)_activate", "instance": view.instance, "revision": view.revision, "node": node])
    }

    func refreshChats() { send(["op": "chats_refresh"]) }

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

    /// Answer or close an input request on the Computers or Chats surface.
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

    /// My reports; reports that wait or failed are sent again.
    func reports(received: @escaping (ReportsPacket) -> Void) {
        let hex = (try? DeviceKey.loadOrCreateVerse())?.map { String(format: "%02x", $0) }.joined() ?? ""
        call(["op": "reports", "world_secret_hex": hex]) { data in
            guard let packet = try? JSONDecoder().decode(ReportsPacket.self, from: data),
                  packet.schema == "openagents.reports.v1" else { return }
            received(packet)
        }
    }

    /// Turn Playtest session on (a new log) or off, or delete its log.
    func playtestSession(on: Bool?, tab: String = "account", route: String = "playtest",
                         received: @escaping (ReportsPacket) -> Void) {
        let request: [String: Any] = on.map { ["op": "playtest_session", "on": $0, "tab": tab, "route": route] }
            ?? ["op": "playtest_clear"]
        call(request) { data in
            guard let packet = try? JSONDecoder().decode(ReportsPacket.self, from: data),
                  packet.schema == "openagents.reports.v1" else { return }
            received(packet)
        }
    }

    /// Where the tester is, for the session log; Rust records it only
    /// while Playtest session is on.
    func playtestScreen(tab: String, route: String) {
        send(["op": "playtest_screen", "tab": tab, "route": route])
    }

    /// A terminal request: a resize, typed text, a key, or a paste.
    func terminal(_ request: [String: Any]) {
        call(request) { data in
            guard let packet = try? JSONDecoder().decode(TerminalPacket.self, from: data) else { return }
            self.receiveTerminal(packet)
        }
    }

    /// Poll the open terminal; skipped while a poll is in flight.
    func pollTerminal() {
        guard !terminalPolling else { return }
        terminalPolling = true
        call(["op": "terminal_poll", "known": terminalRevision]) { data in
            self.terminalPolling = false
            guard let packet = try? JSONDecoder().decode(TerminalPacket.self, from: data) else { return }
            self.receiveTerminal(packet)
        }
    }

    private func receiveTerminal(_ packet: TerminalPacket) {
        guard packet.open else {
            terminalView = nil
            terminalRevision = 0
            return
        }
        if let view = packet.view, packet.revision > terminalRevision {
            terminalRevision = packet.revision
            terminalView = view
        }
        if packet.paste {
            terminal(["op": "terminal_paste", "text": UIPasteboard.general.string ?? ""])
        }
    }

    private func send(_ request: [String: Any]) {
        call(request) { data in
            guard let packet = try? JSONDecoder().decode(AppPacket.self, from: data),
                  packet.schema == "openagents.mobile.v1" else {
                self.failure = "OpenAgents returned an unreadable screen."
                return
            }
            self.packet = packet
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

    private func call(_ request: [String: Any], received: @escaping (Data) -> Void) {
        guard let handle, let body = try? JSONSerialization.data(withJSONObject: request) else { return }
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
