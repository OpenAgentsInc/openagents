// A serial native boundary. Rust owns state, cache, synchronization, and effects.
import Foundation
import SwiftUI

struct MobilePacket: Decodable {
    let schema: String
    let view: NativeView?
    let public_key: String
    let status: String
    let error: String?
    let follow_target: String?
    let follow_page: String?
}

private final class RustWorker {
    private let queue = DispatchQueue(label: "com.openagents.coder.reader", qos: .userInitiated)
    private var handle: UnsafeMutableRawPointer?

    func initialize(synthetic: Bool, completion: @escaping (Result<MobilePacket, Error>) -> Void) {
        queue.async {
            do {
                let secret = try DeviceIdentity.loadOrCreate(synthetic: synthetic)
                let directory = try DeviceIdentity.cacheDirectory(synthetic: synthetic)
                let configuration = try JSONSerialization.data(withJSONObject: [
                    "cache_dir": directory.path,
                    "secret_hex": secret.map { String(format: "%02x", $0) }.joined(),
                    "synthetic": synthetic,
                ])
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
              packet.view == nil || packet.view?.schema == "rust-native.view.v2" else {
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
    private let worker = RustWorker()
    private var foreground = false
    private var pendingForeground: Bool?
    private var pendingFollow: (enabled: Bool, page: String?)?

    init(synthetic: Bool) {
        worker.initialize(synthetic: synthetic) { result in
            Task { @MainActor in self.receive(result) }
        }
    }

    func activate(view: NativeView, node: String) {
        request(["op": "activate", "instance": view.instance,
                 "revision": view.revision, "node": node])
    }

    func connect(_ code: String) {
        request(["op": "connect", "code": code])
    }

    func disconnect() { request(["op": "disconnect"]) }

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

    private func request(_ request: [String: Any]) {
        guard !busy else {
            nativeError = "The reader is updating. Try this action again when the refresh finishes."
            return
        }
        busy = true
        nativeError = nil
        worker.send(request) { result in Task { @MainActor in self.receive(result) } }
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
        } else if let follow = pendingFollow {
            pendingFollow = nil
            setFollowing(follow.enabled, page: follow.page)
        }
    }
}
