// Firm reason: 6 (AppKit lifecycle for the shared compiled GPU app).
import AppKit

private struct EmptyTokenStore: TokenStore {
    func load() -> AppAuth.Tokens? { nil }
    func save(_ tokens: AppAuth.Tokens) {}
    func clear() {}
}
@MainActor private final class EmptyDevices: AppDeviceService {
    func list() -> String { "" }
    func pair(_ link: String, done: @escaping (Bool, String) -> Void) { done(false, "Unavailable in qualification") }
    func call(host: String, body: Data, done: @escaping (AppCompletion.State, Data) -> Void) { done(.unsupported, Data()) }
}
@MainActor private final class QualifyingViewStore: AppViewStore {
    private var state = AppViewState()
    func load() -> AppViewState { state }
    func save(_ next: AppViewState) -> Bool { state = next; return true }
}
@MainActor final class GPUAppDelegate: NSObject, NSApplicationDelegate {
    var controller: GPUWindow?
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationWillTerminate(_ notification: Notification) { controller?.retire() }
    func application(_ app: NSApplication, open urls: [URL]) { for url in urls where url.scheme == "openagents" && url.host == "pair" { AppDeviceLink.shared.opened(url.absoluteString) } }
}
@main struct GPUDesktopApp {
    @MainActor static func main() {
        let inspect = CommandLine.arguments.contains("--inspect-controls") || CommandLine.arguments.contains("--inspect-program")
        guard inspect else { fputs("This target qualifies the compiled desktop SDK. Use --inspect-controls.\n", stderr); exit(2) }
        let app = NSApplication.shared; app.setActivationPolicy(.prohibited)
        let delegate = GPUAppDelegate(); app.delegate = delegate
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path else { exit(2) }
        do {
            let auth = AppAuth(store: EmptyTokenStore(), device: nil)
            let devices: AppDeviceService = EmptyDevices()
            let store = QualifyingViewStore()
            let controller = try GPUWindow(resources: resources, hidden: true, auth: auth, devices: devices, store: store)
            delegate.controller = controller
            // Effects complete on a later main-queue turn, including the initial auth state.
            DispatchQueue.main.async {
                do { try controller.inspectControls(); exit(0) }
                catch { fputs(("GPU qualification: " + error.localizedDescription + "\n"), stderr); exit(1) }
            }
            app.run()

        } catch { fputs(("GPU app: " + error.localizedDescription + "\n"), stderr); exit(1) }
    }
}
