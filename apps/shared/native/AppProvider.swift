// Firm reason: 6 (secure SDK input, protected files/Keychain and existing account/device transport).
// Coax admits every request, replaces the private marker, validates the reply
// and sanitizes recovery. Swift holds no provider protocol decisions.
import Foundation

@MainActor final class AppProviderNative: AppProviderService {
    private let auth: AppAuth
    private let devices: AppDeviceService
    private let http: AppHTTP
    private let store: AppIdentityPendingStore
    var input: () -> String = { "" }
    var clearInput: () -> Void = {}
    private var running = false
    private var serial: UInt64 = 0
    init(auth: AppAuth, devices: AppDeviceService, store: AppIdentityPendingStore? = nil, protocols: [AnyClass] = []) {
        self.auth = auth; self.devices = devices
        #if os(iOS)
        self.store = store ?? KeychainIdentityPending(service: "com.openagents.app.provider.pending.v1")
        #else
        self.store = store ?? AppFileIdentityPending(folder: AppFileViewStore.folder.appendingPathComponent("pending-provider-v1"))
        #endif
        http = AppHTTP(policy: .init(origin: auth.origin, admit: { method, path, bytes in ou_http(method == "POST" ? 2 : 0, path, Int32(bytes)) != 0 }), authorize: AppAccount.authorizer(auth), protocols: protocols)
    }
    func clear() { clearInput() }
    func perform(mode: Int32, host: String, metadata: Data, done: @escaping (AppCompletion.State, Int32, Data) -> Void) {
        guard !running, metadata.withUnsafeBytes({ ou_effect(mode, host, $0.bindMemory(to: UInt8.self).baseAddress, Int32(metadata.count)) }) != 0 else { done(.failed, 400, Data()); return }
        let accountTransport = mode == 1 || mode == 3
        guard !accountTransport || auth.loggedIn else { done(.failed, 401, Data()); return }
        let owner = auth.sessionOwner
        let scope = (accountTransport ? "account:" + (auth.tokens?.account ?? "") : "paired") + ":" + host
        if mode >= 2 {
            guard var kept = store.load(scope) else { done(.done, 200, Data()); return }
            defer { kept.resetBytes(in: 0..<kept.count) }
            var out = [UInt8](repeating: 0, count: 8192)
            let n = kept.withUnsafeBytes { ou_pending($0.bindMemory(to: UInt8.self).baseAddress, Int32(kept.count), &out, Int32(out.count)) }
            done(n > 0 ? .done : .failed, n > 0 ? 200 : 409, n > 0 ? Data(out.prefix(Int(n))) : Data()); return
        }
        var kept = store.load(scope) ?? Data()
        defer { kept.resetBytes(in: 0..<kept.count) }
        let mutation = metadata.withUnsafeBytes { ou_mutation($0.bindMemory(to: UInt8.self).baseAddress, Int32(metadata.count)) }
        // Reads do not consume an unresolved account change. The app recovers
        // its public intent first and chooses an explicit retry.
        guard mutation >= 0, kept.isEmpty || mutation == 1 else { done(.unknown, 409, Data()); return }
        let needsInput = kept.isEmpty && metadata.withUnsafeBytes { ou_input($0.bindMemory(to: UInt8.self).baseAddress, Int32(metadata.count)) } != 0
        var typed = needsInput ? input() : "", out = [UInt8](repeating: 0, count: 8192)
        if needsInput { clearInput() }
        defer { typed = ""; out.withUnsafeMutableBytes { $0.initializeMemory(as: UInt8.self, repeating: 0) } }
        let n = metadata.withUnsafeBytes { m in kept.withUnsafeBytes { k in ou_request(m.bindMemory(to: UInt8.self).baseAddress, Int32(metadata.count), k.bindMemory(to: UInt8.self).baseAddress, Int32(kept.count), typed, &out, Int32(out.count)) } }
        guard n > 0 else { done(.failed, 400, Data()); return }
        var request = Data(out.prefix(Int(n)))
        if mutation == 1 && kept.isEmpty {
            guard store.save(request, account: scope), store.load(scope) == request else { request.resetBytes(in: 0..<request.count); done(.failed, 507, Data()); return }
        }
        running = true
        func answer(_ state: AppCompletion.State, _ status: Int32, _ raw: Data) {
            self.running = false
            guard !accountTransport || self.auth.sessionOwner == owner else { done(.cancelled, 0, Data()); return }
            var publicBytes = [UInt8](repeating: 0, count: 65536)
            let count = metadata.withUnsafeBytes { m in raw.withUnsafeBytes { r in ou_reply(host, m.bindMemory(to: UInt8.self).baseAddress, Int32(metadata.count), r.bindMemory(to: UInt8.self).baseAddress, Int32(raw.count), &publicBytes, Int32(publicBytes.count)) } }
            if state == .done, status == 200, count > 0 {
                let publicResult = Data(publicBytes.prefix(Int(count)))
                if mutation == 1 {
                    let settled = publicResult.withUnsafeBytes { ou_settled($0.bindMemory(to: UInt8.self).baseAddress, Int32(publicResult.count)) } != 0
                    guard settled else { done(.unknown, 202, publicResult); return }
                    self.store.remove(scope)
                    guard self.store.load(scope) == nil else { done(.unknown, 507, publicResult); return }
                }
                done(.done, 200, publicResult)
            } else { done(mutation == 1 ? .unknown : state == .done ? .failed : state, status, Data()) }
        }
        if accountTransport {
            serial += 1
            http.start(op: serial, generation: 1, .init(method: "POST", path: "/api/chat/environment/accounts/v1?host=" + host, body: request, limit: 65536, timeoutMs: 25000)) { outcome in
                switch outcome {
                case .answer(let status, let raw, _): answer(.done, Int32(status), raw)
                case .failed: answer(.failed, 0, Data())
                case .unknown: answer(.unknown, 0, Data())
                case .part: break
                }
            }
        } else { devices.call(host: host, body: request) { state, raw in answer(state, 200, raw) } }
        request.resetBytes(in: 0..<request.count)
    }
}
