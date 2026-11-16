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
//   - a cancelled request never completes, and a completion carries the generation its request
//     had, so Coax can drop one that outlived its owner;
//   - a request the host does not know completes as failed. It is never dropped silently;
//   - no token, key or pending handle is in any request or completion.
// The host adapts the app's own records to `AppRequest` and `AppCompletion` in one place
// (apps/openagents-ios/AppBridge.swift), so this file names no Coax procedure.
//   apps/shared/native/app-services-check.sh
import Foundation

/// What the app asks the host to do.
enum AppCall {
    case http(AppHTTP.Request)
    /// End one earlier request, or every request of a generation. Neither completes.
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
    case unsupported
}
struct AppRequest { var op: UInt64; var generation: UInt64; var call: AppCall }

struct AppCompletion: Equatable {
    /// done: the effect happened and `body` is its answer. failed: it did not happen.
    /// unknown: it may have happened; Coax must not repeat it by itself. part: more follows.
    enum State: Int32 { case done = 1, failed = 2, unknown = 3, part = 4 }
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

@MainActor final class AppEffects {
    private weak var outbox: AppOutbox?
    private let http: AppHTTP
    private let account: AppAccount
    private let wake: AppWake
    private weak var platform: AppPlatform?
    private var inside = false
    private var late: [AppCompletion] = []
    /// Requests with a completion still owed (not HTTP's or the wakes', which have their own).
    private var owed: [UInt64: UInt64] = [:]
    /// Called after a completion was dispatched: the screen may have changed.
    var changed: (() -> Void)?
    /// Completions delivered, for the checks.
    private(set) var delivered = 0

    init(outbox: AppOutbox, http: AppHTTP, account: AppAccount, wake: AppWake, platform: AppPlatform?) {
        self.outbox = outbox; self.http = http; self.account = account; self.wake = wake; self.platform = platform
    }

    /// Call after every dispatch into the app. Does nothing when called from inside one.
    func pump() {
        guard !inside, let outbox else { return }
        inside = true
        let requests = outbox.drain()
        inside = false
        for r in requests { perform(r) }
    }

    /// Ends everything in flight; nothing completes afterwards (the app is closing or was reset).
    func retire() { epoch += 1; http.cancel(generation: 0); wake.cancel(generation: 0); owed = [:]; late = [] }
    private var epoch = 0

    private func perform(_ r: AppRequest) {
        let op = r.op, generation = r.generation
        func done(_ body: [String: Any] = [:]) { finish(op, generation, .done, 0, (try? JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])) ?? Data("{}".utf8)) }
        func answered(_ a: AppAccount.Answer) {
            if case .said(let words) = a { finish(op, generation, .failed, 0, (try? JSONSerialization.data(withJSONObject: ["error": words])) ?? Data()) } else { done(["loggedIn": account.loggedIn]) }
        }
        switch r.call {
        case .http(let request):
            http.start(op: op, generation: generation, request) { [weak self] outcome in
                guard let self else { return }
                switch outcome {
                case .answer(let status, let body, let type): self.post(AppCompletion(op: op, generation: generation, state: .done, status: Int32(status), body: body, contentType: type))
                case .part(let data): self.post(AppCompletion(op: op, generation: generation, state: .part, status: 200, body: data))
                case .failed(let why): self.post(AppCompletion(op: op, generation: generation, state: .failed, status: why.rawValue))
                case .unknown(let why): self.post(AppCompletion(op: op, generation: generation, state: .unknown, status: why.rawValue))
                }
            }
        case .cancel(let target):
            http.cancel(op: target); wake.cancel(op: target); owed[target] = nil
        case .cancelGeneration(let target):
            http.cancel(generation: target); wake.cancel(generation: target)
            owed = owed.filter { $0.value != target }
        case .wake(let ms):
            wake.after(op: op, generation: generation, milliseconds: ms) { [weak self] in self?.post(AppCompletion(op: op, generation: generation, state: .done)) }
        case .authState: owed[op] = generation; done(["loggedIn": account.loggedIn])
        case .authEmailStart(let email): owed[op] = generation; Task { answered(await account.emailStart(email)) }
        case .authEmailVerify(let code): owed[op] = generation; Task { answered(await account.emailVerify(code)) }
        case .authEmailCancel: owed[op] = generation; account.emailCancel(); done()
        case .authLogout: owed[op] = generation; Task { await account.logout(); done(["loggedIn": false]) }
        case .authRefresh: owed[op] = generation; Task { done(["loggedIn": await account.refresh()]) }
        case .identities:
            owed[op] = generation
            guard let random = AppWake.randomHex(32) else { finish(op, generation, .failed, 0, Data()); return }
            done(["command_id": String(random.prefix(32)), "session_id": String(random.suffix(32)), "now_ms": AppWake.wallMs()])
        case .clipboard(let text):
            owed[op] = generation
            if platform?.copy(text) == true { done() } else { finish(op, generation, .failed, 0, Data()) }
        case .link(let link):
            owed[op] = generation
            guard let platform else { finish(op, generation, .failed, 0, Data()); return }
            platform.open(link) { [weak self] opened in self?.finish(op, generation, opened ? .done : .failed, 0, Data("{}".utf8)) }
        case .unsupported:
            owed[op] = generation; finish(op, generation, .failed, 0, Data())
        }
    }

    /// A completion of a request this file tracks: dropped when the request was cancelled.
    private func finish(_ op: UInt64, _ generation: UInt64, _ state: AppCompletion.State, _ status: Int32, _ body: Data) {
        guard owed[op] == generation else { return }
        owed[op] = nil
        post(AppCompletion(op: op, generation: generation, state: state, status: status, body: body, contentType: "application/json"))
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
