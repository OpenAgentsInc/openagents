// Firm reason: 6 (shared Apple SDK service wiring for the compiled Coax core).
import Foundation

/// Owns SDK adapters while the shared Coax core owns requests, state and scheduling.
/// The GUI can retire without terminating any independently resident environment service.
@MainActor final class AppRuntime {
    let core: AppCoreC
    let bridge: AppCoreBridge
    private let effects: AppEffects
    private let platform: AppPlatform
    private let devices: AppDeviceService
    private let account: AppAccount
    let identity: AppIdentityService
    init?(auth: AppAuth, platform: AppPlatform, devices: AppDeviceService, identity: AppIdentityService) {
        guard let core = AppCoreC() else { return nil }
        self.core = core; self.platform = platform; self.devices = devices; self.identity = identity
        account = AppAccount(auth: auth)
        let http = AppHTTP(policy: .init(origin: auth.origin, admit: { method, path, bytes in
            oa_policy_http(method == "GET" ? AppCoreCode.get : method == "POST" ? AppCoreCode.post : 0, path, Int64(bytes)) != 0
        }), authorize: AppAccount.authorizer(auth))
        bridge = AppCoreBridge(core: core)
        effects = AppEffects(outbox: bridge, http: http, account: account, wake: AppWake(), platform: platform, devices: devices)
        effects.identity = identity; bridge.effects = effects
    }
    func launch(rejoin: String) { core.launch(rejoin: rejoin); bridge.heard() }
    func pump() { bridge.pump() }
    func retire() { bridge.retire(); effects.retire(); identity.clear(); core.after = nil }
}
