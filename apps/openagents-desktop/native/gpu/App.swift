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
        let resident = CommandLine.arguments.contains("--inspect-resident")
        let inspect = resident || CommandLine.arguments.contains("--inspect-controls") || CommandLine.arguments.contains("--inspect-program")
        let app = NSApplication.shared; app.setActivationPolicy(inspect ? .prohibited : .regular)
        let delegate = GPUAppDelegate(); app.delegate = delegate
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path else { exit(2) }
        do {
            let auth = inspect ? AppAuth(store: EmptyTokenStore(), device: nil) : AppAuth()
            let mac = !inspect || resident ? AppMacDevice() : nil
            let devices: AppDeviceService
            if let mac { devices = mac } else { devices = EmptyDevices() }
            let store: AppViewStore = inspect ? QualifyingViewStore() : AppFileViewStore()
            let controller = try GPUWindow(resources: resources, hidden: inspect, auth: auth, devices: devices, store: store)
            delegate.controller = controller
            // Effects complete on a later main-queue turn, including the initial auth state.
            if resident, let mac { Task {
                do { try await inspectResident(mac); controller.retire(); exit(0) }
                catch { fputs(("Resident qualification: " + error.localizedDescription + "\n"), stderr); controller.retire(); exit(1) }
            } } else if inspect { DispatchQueue.main.async {
                do { try controller.inspectControls(); exit(0) }
                catch { fputs(("GPU qualification: " + error.localizedDescription + "\n"), stderr); exit(1) }
            } }
            app.run()

        } catch { fputs(("GPU app: " + error.localizedDescription + "\n"), stderr); exit(1) }
    }
    @MainActor private static func inspectResident(_ devices: AppMacDevice) async throws {
        _ = await withCheckedContinuation { continuation in devices.list { continuation.resume(returning: $0) } }
        guard let host = devices.localPublicKey else { throw AppAuth.Failure.said("No installed resident service was found.") }
        for method in ["DescribeEnvironment", "ListSessions"] {
            var bytes = [UInt8](repeating: 0, count: 24576)
            let n = oa_environment_read(method, "", "", &bytes, Int32(bytes.count))
            guard n > 0 else { throw AppAuth.Failure.said("The shared request codec refused the probe.") }
            let reply = await withCheckedContinuation { continuation in devices.call(host: host, body: Data(bytes.prefix(Int(n)))) { continuation.resume(returning: ($0, $1)) } }
            let count = reply.1.withUnsafeBytes { p in oa_environment_count(host, method, p.bindMemory(to: UInt8.self).baseAddress, Int32(reply.1.count)) }
            guard reply.0 == .done, count >= 0 else { throw AppAuth.Failure.said("The resident service did not return the matching read-only view.") }
            print("Mac resident: \(method) authenticated and decoded; \(count) session summaries; no turn started")
        }
    }
}
