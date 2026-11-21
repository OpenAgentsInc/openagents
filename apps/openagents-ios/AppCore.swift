// Firm reason: 6 (the app's native services wired to the compiled shared core; every decision is Coax).
// The shared app core in the iOS app (apps/shared/app/README.md, GitHub #13645). `AppCoreC` is the
// core's host ABI over core-host.h; `AppServices` builds the native services, bridges them to the
// core (AppCoreBridge, AppEffects) and launches it, rejoining the conversation kept when it last
// changed. HTTP admits only what the core's installed policy admits (up_http). Tokens and the
// email attempt stay in AppAuth and AppAccount.
import UIKit

@MainActor final class AppServices {
    let core: AppCoreC
    let bridge: AppCoreBridge
    private let runtime: AppRuntime
    private let device = AppDevice()
    var presentation: (() -> UIViewController?)? {get {device.presentation} set {device.presentation=newValue}}
    var provider:AppProviderNative {runtime.provider}
    let identity: AppIdentityNative
    /// The conversation to rejoin, kept across launches (an ID, never a token).
    private static let rejoinKey = "openagents.rejoin"

    init?(auth: AppAuth = .shared) {
        let identity = AppIdentityNative(auth: auth)
        identity.captured = { UIScreen.main.isCaptured }
        guard let runtime = AppRuntime(auth: auth, platform: device, devices: AppDeviceLink.shared, identity: identity) else { return nil }
        self.runtime = runtime; self.core = runtime.core; self.bridge = runtime.bridge; self.identity = identity
        self.core.setProfile(AppCoreCode.profileMobile)
        device.fact = { [weak self] fact in
            guard let self else { return }
            switch fact {
            case .foreground:
                AppDeviceLink.shared.resumeFromBackground()
                self.bridge.lifecycle(foreground: true)
            case .background:
                self.keepRejoin()
                self.bridge.lifecycle(foreground: false)
                AppDeviceLink.shared.suspendForBackground()
            case .memoryWarning:
                self.bridge.memoryWarning()
            default: break
            }
        }
    }

    /// Launches the core, rejoining the conversation kept at the last background. Attach the page first.
    func launch() {
        let rejoin = UserDefaults.standard.string(forKey: AppServices.rejoinKey) ?? ""
        kept = rejoin
        runtime.launch(rejoin: rejoin)
        // a pairing link the system opened (now, or before the core was up): dispatch.coax ACT_DEVICE_PAIR
        AppDeviceLink.shared.onLink = { [weak self] link in self?.core.action(AppServices.actDevicePair, text: link); self?.bridge.heard() }
    }
    private static let actDevicePair: Int64 = 19
    /// The page went into the core (a tap): perform what it asked, without waking the page again.
    func pump() { runtime.pump(); keepRejoin() }

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

    /// Experimental features (Settings), as on the desktop (UserDefaults `experimentalFeatures`, off
    /// unless set): log in, the account, Projects, Foreman and the Nostr keys. Off, the phone is local
    /// only: it works with the computers it is paired with, and never asks for a login.
    ///   defaults write com.openagents.app experimentalFeatures -bool YES   (Simulator: xcrun simctl spawn booted defaults write ...)
    private static let experimentalKey = "experimentalFeatures"
    static var experimental: Bool = UserDefaults.standard.bool(forKey: experimentalKey) {
        didSet { if experimental != oldValue { UserDefaults.standard.set(experimental, forKey: experimentalKey) } }
    }
    /// The login card's typed email, kept between launches (no secret: app preferences).
    private static let draftKey = "openagents.login-draft"
    static var keptDraft: String { UserDefaults.standard.string(forKey: draftKey) ?? "" }
    static func keepDraft(_ bytes: String) {
        if bytes.isEmpty { UserDefaults.standard.removeObject(forKey: draftKey) } else { UserDefaults.standard.set(bytes, forKey: draftKey) }
    }
    /// The unsent composer draft text, kept across background and relaunch (#13906).
    private static let composerDraftKey = "openagents.composer-draft"
    static var keptComposerDraft: String { UserDefaults.standard.string(forKey: composerDraftKey) ?? "" }
    static func keepComposerDraft(_ text: String) {
        if text.isEmpty { UserDefaults.standard.removeObject(forKey: composerDraftKey) } else { UserDefaults.standard.set(text, forKey: composerDraftKey) }
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
