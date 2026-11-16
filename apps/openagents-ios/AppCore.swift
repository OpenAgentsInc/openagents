// Firm reason: 6 (the app's native services wired to the compiled shared core; every decision is Coax).
// The shared app core in the iOS app (apps/shared/app/README.md, GitHub #13645). `AppCoreC` is the
// core's host ABI over core-host.h; `AppServices` builds the native services, bridges them to the
// core (AppCoreBridge, AppEffects) and launches it, rejoining the conversation kept when it last
// changed. HTTP admits only what the core's installed policy admits (up_http). Tokens and the
// email attempt stay in AppAuth and AppAccount.
import UIKit

@MainActor final class AppCoreC: AppCoreABI {
    let raw: UnsafeMutableRawPointer
    /// Runs after every call into the core: the page's login flow follows the core (chat_step).
    var after: (() -> Void)?
    init?() { guard let raw = oa_core_open() else { return nil }; self.raw = raw }
    deinit { oa_core_close(raw) }
    private var mono: Int64 { AppWake.steadyMs() }
    private var wall: Int64 { AppWake.wallMs() }
    private func bytes(_ read: (UnsafeMutablePointer<UInt8>, Int64) -> Int64) -> Data? {
        var out = [UInt8](repeating: 0, count: 1 << 18)
        let n = read(&out, Int64(out.count))
        return n < 0 ? nil : Data(out[0..<Int(n)])
    }
    func next() -> Int64 { oa_core_next(raw) }
    func request(_ op: Int64) -> [Int64]? {
        var f = [Int64](repeating: 0, count: AppCoreField.count)
        return oa_core_request(raw, op, &f) != 0 ? f : nil
    }
    func path(_ op: Int64) -> Data? { bytes { oa_core_path(raw, op, $0, $1) } }
    func text(_ op: Int64) -> Data? { bytes { oa_core_text(raw, op, $0, $1) } }
    func body(_ op: Int64) -> Data? { bytes { oa_core_body(raw, op, $0, $1) } }
    func taken(_ op: Int64, accepted: Bool) { _ = oa_core_taken(raw, mono, wall, op, accepted ? 1 : 0); after?() }
    func cancelNext() -> Int64 { oa_core_cancel_next(raw) }
    func done(op: Int64, generation: Int64, outcome: Int32, status: Int32, flag: Bool, text: Data, more: Data, value: Int64) {
        let t = [UInt8](text), m = [UInt8](more)
        _ = oa_core_done(raw, mono, wall, op, generation, Int64(outcome), Int64(status), flag ? 1 : 0, t, Int64(t.count), m, Int64(m.count), value); after?()
    }
    func wake() -> Int64 { oa_core_wake(raw) }
    func clock() { _ = oa_core_clock(raw, mono, wall); after?() }
    func lifecycle(_ code: Int32) { _ = oa_core_lifecycle(raw, mono, wall, Int64(code)); after?() }
    func launch(rejoin: String) { _ = oa_core_launch(raw, mono, wall, rejoin); after?() }
    /// A person's action the platform owns (the editor's text, a hardware Return).
    func action(_ code: Int64, flag: Bool = false, text: String = "", more: String = "") { _ = oa_core_action(raw, mono, wall, code, flag ? 1 : 0, text, more); after?() }
    func dirty(_ mask: Int64) -> Int64 { oa_core_dirty_take(raw, mask) }
    /// The conversation to rejoin at the next launch (app_host_keep): an ID, at most 193 bytes.
    func keep() -> String {
        var out = [UInt8](repeating: 0, count: 256)
        let n = oa_core_keep(raw, &out, Int64(out.count))
        return n > 0 ? String(decoding: out[0..<Int(n)], as: UTF8.self) : ""
    }
}

@MainActor final class AppServices {
    let core: AppCoreC
    let bridge: AppCoreBridge
    private let effects: AppEffects
    private let device = AppDevice()
    private let account: AppAccount
    let identity: AppIdentityNative
    /// The conversation to rejoin, kept across launches (an ID, never a token).
    private static let rejoinKey = "openagents.rejoin"

    init?(auth: AppAuth = .shared) {
        guard let core = AppCoreC() else { return nil }
        self.core = core
        account = AppAccount(auth: auth)
        identity = AppIdentityNative(auth: auth)
        let http = AppHTTP(policy: .init(origin: auth.origin, admit: { method, path, bytes in
            oa_policy_http(method == "GET" ? AppCoreCode.get : method == "POST" ? AppCoreCode.post : 0, path, Int64(bytes)) != 0
        }), authorize: AppAccount.authorizer(auth))
        bridge = AppCoreBridge(core: core)
        effects = AppEffects(outbox: bridge, http: http, account: account, wake: AppWake(), platform: device, devices: AppDeviceLink.shared)
        effects.identity = identity
        bridge.effects = effects
        device.fact = { [weak self] fact in
            guard let self else { return }
            switch fact {
            case .foreground: self.bridge.lifecycle(foreground: true)
            case .background: self.keepRejoin(); self.bridge.lifecycle(foreground: false)
            default: break
            }
        }
    }

    /// Launches the core, rejoining the conversation kept at the last background. Attach the page first.
    func launch() {
        let rejoin = UserDefaults.standard.string(forKey: AppServices.rejoinKey) ?? ""
        kept = rejoin
        core.launch(rejoin: rejoin)
        bridge.heard()
        // a pairing link the system opened (now, or before the core was up): dispatch.coax ACT_DEVICE_PAIR
        AppDeviceLink.shared.onLink = { [weak self] link in self?.core.action(AppServices.actDevicePair, text: link); self?.bridge.heard() }
    }
    private static let actDevicePair: Int64 = 19
    /// The page went into the core (a tap): perform what it asked, without waking the page again.
    func pump() { bridge.pump(); keepRejoin() }

    /// The conversation to rejoin, written whenever it changes: iOS ends an app swiped away in the
    /// switcher, or one that crashed, with no background notice, so the last background is too late.
    private var kept: String?
    private func keepRejoin() {
        guard kept != nil else { return }   // not launched yet: the core has not read the kept one
        let now = core.keep()
        if now == kept { return }
        kept = now
        if now.isEmpty { UserDefaults.standard.removeObject(forKey: AppServices.rejoinKey) } else { UserDefaults.standard.set(now, forKey: AppServices.rejoinKey) }
    }

    /// The login card's typed email, kept between launches (no secret: app preferences).
    private static let draftKey = "openagents.login-draft"
    static var keptDraft: String { UserDefaults.standard.string(forKey: draftKey) ?? "" }
    static func keepDraft(_ bytes: String) {
        if bytes.isEmpty { UserDefaults.standard.removeObject(forKey: draftKey) } else { UserDefaults.standard.set(bytes, forKey: draftKey) }
    }
    /// The chips' choice per draft (es_save: a computer's key and a repository's name per line), kept
    /// between launches next to the drafts (no secret: app preferences).
    private static let environmentKey = "openagents.environment"
    static var keptEnvironment: String { UserDefaults.standard.string(forKey: environmentKey) ?? "" }
    static func keepEnvironment(_ text: String) {
        if text.isEmpty { UserDefaults.standard.removeObject(forKey: environmentKey) } else { UserDefaults.standard.set(text, forKey: environmentKey) }
    }
}

/// A pairing link the system opened (openagents://pair?..., printed by `openagents-environment pair`):
/// the device service hands it to the core.
extension OpenAgentsApp {
    @objc func application(_ app: UIApplication, open url: URL, options: [UIApplication.OpenURLOptionsKey: Any]) -> Bool {
        guard url.scheme == "openagents", url.host == "pair" else { return false }
        AppDeviceLink.shared.opened(url.absoluteString)
        return true
    }
}
