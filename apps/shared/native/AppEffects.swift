// Firm reason: 6 (the native side of a compiled Coax app's effect outbox: it performs platform effects).
// The effect bridge for a compiled Coax app (GitHub #13645). The app's Coax decides; it writes
// typed requests to its outbox, each with an operation ID and its owner's generation. After every
// dispatch into Coax the host calls `pump`: the bridge takes the requests out, performs each with
// a native service and gives the completion back as a later dispatch.
//
// The rules it keeps:
//   - a completion is never delivered inside a dispatch. Every one, even of a service that
//     answers at once, arrives in a later turn of the main queue, in the order it was produced;
//   - Coax is entered by one caller at a time, on the main thread;
//   - every request completes exactly once, except a cancel, which is an instruction and has no
//     completion of its own. A cancelled request completes as cancelled, at once, and its real
//     answer is dropped. A completion carries the generation its request had, so Coax
//     can drop one that outlived its owner;
//   - a request the host does not know completes as unsupported. It is never dropped silently;
//   - no token, key or pending handle is in any request or completion.
// The host adapts the app's own records to `AppRequest` and `AppCompletion` in one place
// (apps/openagents-ios/AppBridge.swift), so this file names no Coax procedure.
//   apps/shared/native/app-services-check.sh
import Foundation

/// What the app asks the host to do.
enum AppCall {
    case http(AppHTTP.Request)
    /// End one earlier request, or every request of a generation. Each completes as cancelled.
    case cancel(op: UInt64)
    case cancelGeneration(UInt64)
    case authState
    case authEmailStart(String)
    case authEmailVerify(String)
    case authEmailCancel
    case authLogout
    case authRefresh
    /// Complete after this many milliseconds.
    case wake(milliseconds: Int)
    /// Random 128-bit identities and the wall clock.
    case identities
    /// Write text to the clipboard. The app asks only on an explicit action.
    case clipboard(String)
    /// Open a link outside the app. The app asks only on an explicit action.
    case link(String)
    /// The computers this phone is paired with, with no account (AppDeviceService).
    case deviceList
    /// Pair with the computer in an openagents://pair? link.
    case devicePair(String)
    /// One environment request to a paired computer (its key, 64 hex).
    case deviceCall(host: String, body: Data)
    case identity(action: String, expected: String)
    case unsupported
}
/// The phone's own computers (AppDeviceLink on iOS). The device key and the grants stay in it.
@MainActor protocol AppDeviceService: AnyObject {
    /// One computer a line: its key, TAB, its name.
    func list() -> String
    func pair(_ link: String, done: @escaping (_ paired: Bool, _ words: String) -> Void)
    func call(host: String, body: Data, done: @escaping (AppCompletion.State, Data) -> Void)
}
struct AppRequest { var op: UInt64; var generation: UInt64; var call: AppCall }

struct AppCompletion: Equatable {
    /// done: the effect happened and `body` is its answer. failed: it did not happen.
    /// unknown: it may have happened; Coax must not repeat it by itself. cancelled: the host
    /// stopped it when asked. unsupported: this host has no such service. part: more follows.
    enum State: Int32 { case done = 1, failed = 2, unknown = 3, cancelled = 4, unsupported = 5, part = 6 }
    var op: UInt64
    var generation: UInt64
    var state: State
    /// The HTTP status, or a reason (AppHTTP.Reason) when the state is failed or unknown.
    var status: Int32 = 0
    var body = Data()
    var contentType = ""
}

/// The app's outbox and its completion entry, as the host exposes them.
@MainActor protocol AppOutbox: AnyObject {
    /// Takes every request written since the last call.
    func drain() -> [AppRequest]
    /// Dispatches one completion into the app.
    func complete(_ completion: AppCompletion)
}
/// The effects that need the platform's UI framework.
@MainActor protocol AppPlatform: AnyObject {
    func copy(_ text: String) -> Bool
    func open(_ link: String, done: @escaping (Bool) -> Void)
}
/// Closed custody effects. Only public metadata or a static refusal returns to the app core.
@MainActor protocol AppIdentityService: AnyObject {
    func perform(action: String, expected: String, done: @escaping (AppCompletion.State, Int32, Data) -> Void)
    func clear()
}

@MainActor final class AppEffects {
    private weak var outbox: AppOutbox?
    private let http: AppHTTP
    private let account: AppAccount
    private let wake: AppWake
    private weak var platform: AppPlatform?
    private weak var devices: AppDeviceService?
    var identity: AppIdentityService?
    private var inside = false
    private var late: [AppCompletion] = []
    /// Requests with a completion still owed, and the generation each had.
    private var owed: [UInt64: UInt64] = [:]
    /// Called after a completion was dispatched: the screen may have changed.
    var changed: (() -> Void)?
    /// Completions delivered, for the checks.
    private(set) var delivered = 0

    init(outbox: AppOutbox, http: AppHTTP, account: AppAccount, wake: AppWake, platform: AppPlatform?, devices: AppDeviceService? = nil) {
        self.outbox = outbox; self.http = http; self.account = account; self.wake = wake; self.platform = platform; self.devices = devices
    }

    /// Call after every dispatch into the app. Does nothing when called from inside one.
    func pump() {
        guard !inside, let outbox else { return }
        inside = true
        let requests = outbox.drain()
        inside = false
        for r in requests { perform(r) }
    }

    /// Ends everything in flight with no completion; nothing completes afterwards (the app is closing or was reset).
    func retire() { epoch += 1; http.cancel(generation: 0); wake.cancel(generation: 0); owed = [:]; late = [] }
    private var epoch = 0

    private func perform(_ r: AppRequest) {
        let op = r.op, generation = r.generation
        func done(_ body: [String: Any] = [:]) { finish(op, generation, .done, 200, (try? JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])) ?? Data("{}".utf8)) }
        func answered(_ a: AppAccount.Answer) {
            if case .said(let words) = a { finish(op, generation, .failed, 0, (try? JSONSerialization.data(withJSONObject: ["error": words])) ?? Data()) } else { done(["loggedIn": account.loggedIn]) }
        }
        if case .cancel = r.call {} else if case .cancelGeneration = r.call {} else { owed[op] = generation }
        switch r.call {
        case .http(let request):
            http.start(op: op, generation: generation, request) { [weak self] outcome in
                guard let self, self.owed[op] == generation else { return }
                switch outcome {
                case .answer(let status, let body, let type): self.finish(op, generation, .done, Int32(status), body, type)
                case .part(let data): self.post(AppCompletion(op: op, generation: generation, state: .part, status: 200, body: data))
                case .failed(let why): self.finish(op, generation, .failed, why.rawValue, Data())
                case .unknown(let why): self.finish(op, generation, .unknown, why.rawValue, Data())
                }
            }
        case .cancel(let target): stop(target)
        case .cancelGeneration(let target): for (each, g) in owed where g == target { stop(each) }
        case .wake(let ms): wake.after(op: op, generation: generation, milliseconds: ms) { [weak self] in self?.finish(op, generation, .done, 0, Data("{}".utf8)) }
        case .authState: done(["loggedIn": account.loggedIn])
        case .authEmailStart(let email): Task { answered(await account.emailStart(email)) }
        case .authEmailVerify(let code): Task { answered(await account.emailVerify(code)) }
        case .authEmailCancel: account.emailCancel(); done()
        case .authLogout: Task { await account.logout(); done(["loggedIn": false]) }
        case .authRefresh: Task { done(["loggedIn": await account.refresh()]) }
        case .identities:
            guard let random = AppWake.randomHex(32) else { finish(op, generation, .failed, 0, Data()); return }
            done(["command_id": String(random.prefix(32)), "session_id": String(random.suffix(32)), "now_ms": AppWake.wallMs()])
        case .clipboard(let text):
            if platform?.copy(text) == true { done() } else { finish(op, generation, .failed, 0, Data()) }
        case .link(let link):
            guard let platform else { finish(op, generation, .failed, 0, Data()); return }
            platform.open(link) { [weak self] opened in self?.finish(op, generation, opened ? .done : .failed, 0, Data("{}".utf8)) }
        case .deviceList:
            guard let devices else { finish(op, generation, .unsupported, 0, Data()); return }
            finish(op, generation, .done, 200, Data(devices.list().utf8), "text/plain")
        case .devicePair(let link):
            guard let devices else { finish(op, generation, .unsupported, 0, Data()); return }
            devices.pair(link) { [weak self] ok, words in
                self?.finish(op, generation, ok ? .done : .failed, 200, (try? JSONSerialization.data(withJSONObject: ok ? ["name": words] : ["error": words])) ?? Data())
            }
        case .deviceCall(let host, let body):
            guard let devices else { finish(op, generation, .unsupported, 0, Data()); return }
            devices.call(host: host, body: body) { [weak self] state, view in self?.finish(op, generation, state, 200, view) }
        case .identity(let action, let expected):
            guard let identity else { finish(op, generation, .unsupported, 0, Data()); return }
            identity.perform(action: action, expected: expected) { [weak self] state, status, publicBody in
                self?.finish(op, generation, state, status, publicBody)
            }
        case .unsupported: finish(op, generation, .unsupported, 0, Data())
        }
    }

    /// Stops a request and completes it as cancelled. Its own answer, if one comes, is dropped.
    private func stop(_ op: UInt64) {
        guard let generation = owed[op] else { return }
        http.cancel(op: op); wake.cancel(op: op)
        finish(op, generation, .cancelled, 0, Data())
    }

    /// The one final completion of a request: dropped when it already had one (it was cancelled).
    private func finish(_ op: UInt64, _ generation: UInt64, _ state: AppCompletion.State, _ status: Int32, _ body: Data, _ type: String = "application/json") {
        guard owed[op] == generation else { return }
        owed[op] = nil
        post(AppCompletion(op: op, generation: generation, state: state, status: status, body: body, contentType: type))
    }

    /// Queues a completion for a later turn of the main queue.
    private func post(_ c: AppCompletion) {
        let at = epoch
        DispatchQueue.main.async { [weak self] in MainActor.assumeIsolated { if let self, self.epoch == at { self.deliver(c) } } }
    }
    private func deliver(_ c: AppCompletion) {
        guard let outbox else { return }
        if inside { late.append(c); return }
        inside = true
        outbox.complete(c); delivered += 1
        inside = false
        pump()
        changed?()
        if !late.isEmpty { let next = late.removeFirst(); post(next) }
    }
}
