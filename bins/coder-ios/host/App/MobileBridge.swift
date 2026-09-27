// A serial native boundary. Rust owns state, cache, synchronization, and effects.
import Foundation
import SwiftUI

struct MobilePacket: Decodable {
    let schema: String
    let view: NativeView?
    let public_key: String
    let status: String
    let paired: Bool
    let pairing_completed: Bool?
    let reading: Bool
    let error: String?
    let follow_target: String?
    let follow_page: String?
    // The Computers surface has its own instance and revisions.
    let computers: NativeView?
    let computers_input: ComputersInput?
    let computers_qr: ComputersQR?
    let computers_exit: Bool?
    /// Push wake status, present once push is configured or requested.
    let push: String?
}

/// An invitation QR code Rust rendered on this device: one string of `1`
/// (dark) and `0` (light) per module row.
struct ComputersQR: Decodable, Equatable {
    let size: Int
    let rows: [String]
}

/// A value Rust asks the native host to collect. Rust validates it.
struct ComputersInput: Decodable, Equatable {
    let token: String
    let purpose: String
    let label: String
    let prompt: String
    let scan: Bool
    /// Mask the field and never echo, log, or keep the value.
    let secret: Bool
    let max_bytes: Int

    private enum CodingKeys: String, CodingKey {
        case token, purpose, label, prompt, scan, secret, max_bytes
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        token = try values.decode(String.self, forKey: .token)
        purpose = try values.decode(String.self, forKey: .purpose)
        label = try values.decode(String.self, forKey: .label)
        prompt = try values.decode(String.self, forKey: .prompt)
        scan = try values.decode(Bool.self, forKey: .scan)
        secret = try values.decodeIfPresent(Bool.self, forKey: .secret) ?? false
        max_bytes = try values.decode(Int.self, forKey: .max_bytes)
    }
}

private final class RustWorker {
    private let queue = DispatchQueue(label: "com.openagents.coder.reader", qos: .userInitiated)
    private var handle: UnsafeMutableRawPointer?

    func initialize(synthetic: Bool, loopbackTest: Bool,
                    completion: @escaping (Result<MobilePacket, Error>) -> Void) {
        queue.async {
            do {
                let secret = try DeviceIdentity.loadOrCreate(synthetic: synthetic)
                let directory = try DeviceIdentity.cacheDirectory(synthetic: synthetic)
                var settings: [String: Any] = [
                    "cache_dir": directory.path,
                    "secret_hex": secret.map { String(format: "%02x", $0) }.joined(),
                    "synthetic": synthetic,
                    "loopback_test": loopbackTest,
                ]
                // Push stays off unless this build names a relay and gateway.
                // Synthetic launches leave it off, except a loopback test.
                if !synthetic || loopbackTest, let push = PushSettings.configured {
                    settings["push"] = push.rust
                }
                let configuration = try JSONSerialization.data(withJSONObject: settings)
                self.handle = configuration.withUnsafeBytes {
                    coder_mobile_create($0.bindMemory(to: UInt8.self).baseAddress, $0.count)
                }
                guard self.handle != nil else {
                    throw ReaderError.message("The reader could not open its protected local state.")
                }
                completion(.success(try self.call(["op": "snapshot"])))
            } catch { completion(.failure(error)) }
        }
    }

    func send(_ request: [String: Any], completion: @escaping (Result<MobilePacket, Error>) -> Void) {
        queue.async {
            do { completion(.success(try self.call(request))) }
            catch { completion(.failure(error)) }
        }
    }

    private func call(_ request: [String: Any]) throws -> MobilePacket {
        guard let handle else { throw ReaderError.message("The reader has not opened its local state.") }
        let input = try JSONSerialization.data(withJSONObject: request)
        guard input.count <= 131_072 else { throw ReaderError.message("The native request is too large.") }
        let result = input.withUnsafeBytes {
            coder_mobile_call(handle, $0.bindMemory(to: UInt8.self).baseAddress, $0.count)
        }
        defer { coder_mobile_buffer_free(result) }
        guard let pointer = result.data, result.len > 0, result.len <= 1_048_576 else {
            throw ReaderError.message("Rust returned an invalid or oversized view packet.")
        }
        let packet = try JSONDecoder().decode(MobilePacket.self, from: Data(bytes: pointer, count: result.len))
        guard packet.schema == "coder.mobile.v1",
              packet.view == nil || packet.view?.schema == "rust-native.view.v2",
              packet.computers == nil || packet.computers?.schema == "rust-native.view.v2" else {
            throw ReaderError.message("This app does not support the returned view version.")
        }
        return packet
    }

    deinit {
        // The application retains this worker for its entire scene lifetime;
        // queued operations retain it until their synchronous Rust call ends.
        if let handle { coder_mobile_destroy(handle) }
    }
}

@MainActor
final class MobileBridge: ObservableObject {
    @Published private(set) var packet: MobilePacket?
    @Published private(set) var busy = true
    @Published private(set) var nativeError: String?
    /// Why this device couldn't obtain a push token, if it couldn't.
    @Published private(set) var pushFailure: String?
    private let worker = RustWorker()
    private var foreground = false
    private var pendingForeground: Bool?
    private var pendingLifecycle: Bool?
    private var pendingFollow: (enabled: Bool, page: String?)?
    private var pendingPushToken: String?

    /// `loopbackTest` is for test launches only: the Computers surface then
    /// admits a `ws://` loopback relay and a host on this machine.
    init(synthetic: Bool, loopbackTest: Bool = false) {
        worker.initialize(synthetic: synthetic, loopbackTest: loopbackTest) { result in
            Task { @MainActor in self.receive(result) }
        }
    }

    func activate(view: NativeView, node: String) {
        request(["op": "activate", "instance": view.instance,
                 "revision": view.revision, "node": node])
    }

    func connect(_ code: String, completed: @escaping (Bool) -> Void = { _ in }) {
        request(["op": "connect", "code": code]) { result in
            if case let .success(packet) = result { completed(packet.pairing_completed == true) }
            else { completed(false) }
        }
    }

    func disconnect() { request(["op": "disconnect"]) }

    func activateComputers(view: NativeView, node: String) {
        request(["op": "computers_activate", "instance": view.instance,
                 "revision": view.revision, "node": node])
    }

    func submitComputers(token: String, value: String) {
        request(["op": "computers_input", "token": token, "value": value])
    }

    func cancelComputers(token: String) {
        request(["op": "computers_cancel", "token": token])
    }

    func refreshComputers() { request(["op": "computers_refresh"]) }

    /// Push wake status for the device details: Rust's, or the native failure.
    var pushStatus: String? { pushFailure ?? packet?.push }

    func pushFailed(_ reason: String) { pushFailure = reason }

    /// The platform issued a push token, as lowercase hex. Rust registers it.
    func pushToken(_ token: String) {
        pushFailure = nil
        if busy { pendingPushToken = token; return }
        request(["op": "push_token", "token": token])
    }

    /// Poll the Computers surface while it is visible; skipped while busy.
    func pollComputers() {
        guard !busy else { return }
        refreshComputers()
    }

    /// The scene became active or moved to the background. Rust passes it to
    /// every host supervisor.
    func setLifecycle(_ active: Bool) {
        if busy { pendingLifecycle = active; return }
        request(["op": "lifecycle", "active": active])
    }

    func setFollowing(_ enabled: Bool, page: String?) {
        if busy { pendingFollow = (enabled, page); return }
        var command: [String: Any] = ["op": "follow", "enabled": enabled]
        if let page { command["page"] = page }
        request(command)
    }

    func refresh(force: Bool = false) {
        guard foreground, !busy else { return }
        request(["op": force ? "refresh_now" : "refresh"])
    }

    func setForeground(_ active: Bool) {
        foreground = active
        if busy { pendingForeground = active; return }
        request(["op": "foreground", "active": active])
    }

    private func request(_ request: [String: Any], completed: ((Result<MobilePacket, Error>) -> Void)? = nil) {
        guard !busy else {
            nativeError = "The reader is updating. Try this action again when the refresh finishes."
            completed?(.failure(ReaderError.message(nativeError!)))
            return
        }
        busy = true
        nativeError = nil
        worker.send(request) { result in
            Task { @MainActor in self.receive(result); completed?(result) }
        }
    }

    private func receive(_ result: Result<MobilePacket, Error>) {
        busy = false
        switch result {
        case let .success(packet): self.packet = packet
        case let .failure(error): nativeError = error.localizedDescription
        }
        if let active = pendingForeground {
            pendingForeground = nil
            setForeground(active)
        } else if let active = pendingLifecycle {
            pendingLifecycle = nil
            setLifecycle(active)
        } else if let follow = pendingFollow {
            pendingFollow = nil
            setFollowing(follow.enabled, page: follow.page)
        } else if let token = pendingPushToken {
            pendingPushToken = nil
            pushToken(token)
        }
    }
}
