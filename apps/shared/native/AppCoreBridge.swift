// Firm reason: 6 (the native side of the shared app core's host ABI: it adapts typed requests to the services).
// The bridge between the shared app core (apps/shared/app/core/host.coax, GitHub #13645) and the
// native services. The core decides and keeps its outbox; this file only moves requests and
// results between the core's C entry points and AppEffects:
//   - each queued request is read, taken (or refused when this host cannot read it) and handed
//     to AppEffects; each cancellation the core asks for stops that operation, which still
//     completes, as cancelled;
//   - each completion goes back through `done` with the fields of the core's service table;
//   - the core's next wake is a timer here; when it fires the core hears the clock;
//   - lifecycle facts go in as they happen.
// Every number of the core's ABI (kinds, fields, codes) is in `AppCoreKind`, `AppCoreField` and
// `AppCoreCode`; outcomes are AppCompletion.State's raw values (FXO_). So a renumbering costs one edit. No token, key or email attempt crosses.
//   apps/shared/native/app-services-check.sh
import Foundation

/// The core's host entry points (host.coax app_host_*). A compiled app implements it over the
/// generated C functions; the check implements it with a stand-in core.
@MainActor protocol AppCoreABI: AnyObject {
    func next() -> Int64
    /// The queued request's fields in `Field` order, nil when op is not queued.
    func request(_ op: Int64) -> [Int64]?
    func path(_ op: Int64) -> Data?
    func text(_ op: Int64) -> Data?
    func body(_ op: Int64) -> Data?
    func taken(_ op: Int64, accepted: Bool)
    func cancelNext() -> Int64
    func done(op: Int64, generation: Int64, outcome: Int32, status: Int32, flag: Bool, text: Data, more: Data, value: Int64)
    /// The next monotonic millisecond the core wants the clock at, or -1.
    func wake() -> Int64
    func clock()
    func lifecycle(_ code: Int32)
}

/// effects.coax FX_ kinds.
enum AppCoreKind: Int64 { case http = 1, authState = 2, authEmail = 3, authVerify = 4, authLogout = 5, ids = 6, clipboard = 7, link = 8, authCancel = 9, deviceList = 10, devicePair = 11, deviceCall = 12, identity = 13, provider = 14 }
/// host.coax APP_HOST_ fields, in order.
enum AppCoreField: Int { case kind = 0, generation, method, limit, timeout, owner; static let count = 6 }
/// url-policy.coax UP_ methods and dispatch.coax LIFE_ codes.
enum AppCoreCode { static let get: Int64 = 1, post: Int64 = 2; static let foreground: Int32 = 1, background: Int32 = 2 }

@MainActor final class AppCoreBridge: AppOutbox {
    private let core: AppCoreABI
    /// The kind of every request in flight, to shape its completion.
    private var kinds: [UInt64: AppCoreKind] = [:]
    private var timer: DispatchSourceTimer?
    private var armed: Int64 = -1
    /// The monotonic clock in ms; the check gives its own.
    var now: () -> Int64 = { AppWake.steadyMs() }
    /// Called after the core heard something: draw what it marked.
    var changed: (() -> Void)?
    weak var effects: AppEffects?

    init(core: AppCoreABI) { self.core = core }

    func drain() -> [AppRequest] {
        var out: [AppRequest] = []
        while true {
            let op = core.next()
            guard op > 0 else { break }
            guard let call = read(op), let fields = core.request(op) else { core.taken(op, accepted: false); continue }
            let generation = UInt64(max(0, fields[AppCoreField.generation.rawValue]))
            kinds[UInt64(op)] = AppCoreKind(rawValue: fields[AppCoreField.kind.rawValue])
            core.taken(op, accepted: true)
            out.append(AppRequest(op: UInt64(op), generation: generation, call: call))
        }
        while true {
            let op = core.cancelNext()
            guard op > 0 else { break }
            out.append(AppRequest(op: UInt64(op), generation: 0, call: .cancel(op: UInt64(op))))
        }
        return out
    }

    /// The request as a service call, nil when this host cannot read it (it is refused).
    private func read(_ op: Int64) -> AppCall? {
        guard let f = core.request(op), f.count >= AppCoreField.count else { return nil }
        let words = { (d: Data?) in d.flatMap { String(data: $0, encoding: .utf8) } }
        guard let kind = AppCoreKind(rawValue: f[AppCoreField.kind.rawValue]) else { return .unsupported }
        switch kind {
        case .http:
            let method = f[AppCoreField.method.rawValue] == AppCoreCode.get ? "GET" : f[AppCoreField.method.rawValue] == AppCoreCode.post ? "POST" : ""
            guard !method.isEmpty, let path = words(core.path(op)), let body = core.body(op) else { return nil }
            return .http(.init(method: method, path: path, body: body, limit: Int(f[AppCoreField.limit.rawValue]), timeoutMs: Int(f[AppCoreField.timeout.rawValue])))
        case .authState: return .authState
        case .authEmail: return words(core.text(op)).map { .authEmailStart($0) }
        case .authVerify: return words(core.text(op)).map { .authEmailVerify($0) }
        case .authCancel: return .authEmailCancel
        case .authLogout: return .authLogout
        case .ids: return .identities
        case .clipboard: return words(core.body(op)).map { .clipboard($0) }
        case .link: return words(core.body(op)).map { .link($0) }
        case .deviceList: return .deviceList
        case .devicePair: return words(core.body(op)).map { .devicePair($0) }
        case .deviceCall:
            guard let host = words(core.text(op)), let body = core.body(op) else { return nil }
            return .deviceCall(host: host, body: body)
        case .provider:
            guard let host = words(core.text(op)), let metadata = core.body(op) else { return nil }
            return .provider(mode: Int32(f[AppCoreField.method.rawValue]), host: host, metadata: metadata)
        case .identity:
            guard let action = words(core.text(op)), let expected = words(core.body(op)) else { return nil }
            return .identity(action: action, expected: expected)
        }
    }

    func complete(_ c: AppCompletion) {
        if c.state == .part { return }   // the core asks for no stream
        let kind = kinds.removeValue(forKey: c.op)
        let json = (try? JSONSerialization.jsonObject(with: c.body)) as? [String: Any] ?? [:]
        var status: Int32 = 0, flag = false, text = Data(), more = Data(), value: Int64 = 0
        switch kind {
        case .http?, .identity?, .provider?: status = c.status; text = c.body
        case .authState?, .authLogout?: flag = json["loggedIn"] as? Bool ?? false
        case .authEmail?, .authVerify?:
            flag = c.state == .done
            if let words = json["error"] as? String { text = Data(words.utf8) }
        case .deviceList?, .deviceCall?: text = c.body
        case .devicePair?:
            flag = c.state == .done
            text = Data(((json["name"] as? String) ?? (json["error"] as? String) ?? "").utf8)
        case .ids?:
            text = Data((json["command_id"] as? String ?? "").utf8); more = Data((json["session_id"] as? String ?? "").utf8)
            value = (json["now_ms"] as? NSNumber)?.int64Value ?? 0
        default: if c.state == .failed || c.state == .unknown { status = c.status }
        }
        core.done(op: Int64(c.op), generation: Int64(c.generation), outcome: c.state.rawValue, status: status, flag: flag, text: text, more: more, value: value)
        heard()
    }

    /// The core heard input or a fact: perform what it asked and wake it when it wants.
    func heard() {
        effects?.pump()
        arm()
        changed?()
    }
    /// The page dispatched into the core itself: perform what it asked, with no change call.
    func pump() { effects?.pump(); arm() }

    /// The app came to the front or left it.
    func lifecycle(foreground: Bool) {
        core.lifecycle(foreground ? AppCoreCode.foreground : AppCoreCode.background)
        heard()
    }

    /// One timer for the core's next wake.
    private func arm() {
        let at = core.wake()
        guard at != armed else { return }
        armed = at; timer?.cancel(); timer = nil
        guard at >= 0 else { return }
        let t = DispatchSource.makeTimerSource(queue: .main)
        t.schedule(deadline: .now() + .milliseconds(Int(max(0, min(at - now(), 86_400_000)))), leeway: .milliseconds(5))
        t.setEventHandler { [weak self] in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.timer?.cancel(); self.timer = nil; self.armed = -1
                self.core.clock(); self.heard()
            }
        }
        timer = t; t.resume()
    }

    /// Stops the timer (the app is closing).
    func retire() { timer?.cancel(); timer = nil; armed = -1 }
}
