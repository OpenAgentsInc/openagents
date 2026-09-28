// Forwards requests to the Rust app on one serial queue and publishes its
// view. Rust owns the tailnet state; this bridge only opens URLs it names.
import SwiftUI

private struct Reply: Decodable {
    let view: NativeView?
    let open_url: String?
    let wait_for_sign_in: Bool
}

@MainActor
final class MobileBridge: ObservableObject {
    @Published private(set) var view: NativeView?
    @Published private(set) var busy = false
    private let queue = DispatchQueue(label: "com.openagents.app.rust")
    private nonisolated(unsafe) let handle: UnsafeMutableRawPointer?
    private var pending = 0

    init() {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let directory = support.appendingPathComponent("Tailscale", isDirectory: true)
        let configuration = (try? JSONSerialization.data(withJSONObject: ["state_dir": directory.path])) ?? Data()
        handle = configuration.withUnsafeBytes { bytes in
            openagents_mobile_create(bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
        }
        send(["kind": "show"])
    }

    deinit {
        let handle = handle
        queue.async { openagents_mobile_destroy(handle) }
    }

    func refresh() { send(["kind": "refresh"]) }

    func activate(_ node: String) {
        guard let view else { return }
        send(["kind": "activate",
              "activation": ["instance": view.instance, "revision": view.revision, "node": node]])
    }

    private func send(_ request: [String: Any]) {
        guard let handle, let body = try? JSONSerialization.data(withJSONObject: request) else { return }
        pending += 1
        busy = true
        queue.async { [weak self] in
            let reply = body.withUnsafeBytes { bytes -> Reply? in
                let buffer = openagents_mobile_call(handle, bytes.bindMemory(to: UInt8.self).baseAddress, bytes.count)
                defer { openagents_mobile_buffer_free(buffer) }
                guard let data = buffer.data, buffer.len > 0 else { return nil }
                return try? JSONDecoder().decode(Reply.self, from: Data(bytes: data, count: buffer.len))
            }
            DispatchQueue.main.async { self?.receive(reply) }
        }
    }

    private func receive(_ reply: Reply?) {
        pending -= 1
        busy = pending > 0
        guard let reply else { return }
        if let view = reply.view { self.view = view }
        if let link = reply.open_url, let url = URL(string: link), url.scheme == "https" {
            UIApplication.shared.open(url)
        }
        if reply.wait_for_sign_in { send(["kind": "wait_for_sign_in"]) }
    }
}
