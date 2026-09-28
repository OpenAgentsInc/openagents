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

struct AppPacket: Decodable {
    let schema: String
    let device: String
    let device_npub: String
    let computers: NativeView?
    let computers_input: ComputersInput?
    let computers_qr: ComputersQR?
    let coder: NativeView?
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
}

/// One released version and what it brought.
struct Release: Decodable, Hashable {
    struct Item: Decodable, Hashable {
        let title: String
        let detail: String
    }
    let version: String
    let title: String
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
            let configuration = try JSONSerialization.data(withJSONObject: [
                "state_dir": try DeviceKey.stateDirectory().path,
                "secret_hex": secret.map { String(format: "%02x", $0) }.joined(),
            ])
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

    /// Hand Rust the wallet key from Keychain. Rust ignores a repeat.
    func openWallet() {
        guard let entropy = try? DeviceKey.loadOrCreateWallet() else { return }
        send(["op": "wallet_open", "entropy_hex": entropy.map { String(format: "%02x", $0) }.joined()])
    }
    func refreshWallet() { send(["op": "wallet_refresh"]) }

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
