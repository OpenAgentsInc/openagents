import Foundation

enum PylonSupervisorMode: String {
    case disconnected = "Disconnected"
    case connectedExisting = "Connected"
    case bootingBundled = "Booting"
    case bundledRunning = "Bundled"
    case stopped = "Stopped"
}

struct PylonSupervisorSnapshot: Equatable {
    var mode: PylonSupervisorMode = .disconnected
    var controlURL: URL
    var pylonHome: URL
    var identitySummary = "Unknown"
    var accountsSummary = "Not loaded"
    var capacitySummary = "Offline"
    var assignmentsSummary = "Not loaded"
    var lastError: String?
    var logLines: [String] = []

    var pylonStatusText: String {
        switch mode {
        case .connectedExisting: return "Attached"
        case .bootingBundled: return "Booting"
        case .bundledRunning: return "Running"
        case .stopped: return "Stopped"
        case .disconnected: return "Unavailable"
        }
    }

    var providerStatusText: String {
        capacitySummary.contains("available=1") || capacitySummary.contains("ready=1") ? "Online" : "Offline"
    }
}

struct PylonSupervisorConfiguration: Equatable {
    var controlURL: URL
    var bundledPylonEntry: URL
    var bundledBunExecutable: URL
    var appManagedPylonHome: URL
    var existingPylonHome: URL
    var appleFmBridgePath: URL
    var openAgentsBaseURL: String
    var controlTokenOverride: String?

    static func `default`(
        bundle: Bundle = .main,
        fileManager: FileManager = .default,
        environment: [String: String] = ProcessInfo.processInfo.environment
    ) -> PylonSupervisorConfiguration {
        let controlURL = URL(string: environment["PYLON_CONTROL_URL"] ?? "http://127.0.0.1:\(environment["PYLON_CONTROL_PORT"] ?? "4716")")!
        let resources = bundle.resourceURL ?? URL(fileURLWithPath: ".")
        let support = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? fileManager.temporaryDirectory
        let appHome = support.appendingPathComponent("OpenAgents", isDirectory: true)
            .appendingPathComponent("KhalaDesktop", isDirectory: true)
            .appendingPathComponent("Pylon", isDirectory: true)
        let defaultExistingHome = fileManager.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("pylon", isDirectory: true)
        return PylonSupervisorConfiguration(
            controlURL: controlURL,
            bundledPylonEntry: resources.appendingPathComponent("app/pylon-node/index.js"),
            bundledBunExecutable: bundle.executableURL?.deletingLastPathComponent().appendingPathComponent("bun") ?? URL(fileURLWithPath: "/usr/bin/env"),
            appManagedPylonHome: appHome,
            existingPylonHome: URL(fileURLWithPath: environment["PYLON_HOME"] ?? defaultExistingHome.path),
            appleFmBridgePath: resources.appendingPathComponent("app/apple-fm-bridge/foundation-bridge"),
            openAgentsBaseURL: environment["PYLON_OPENAGENTS_BASE_URL"] ?? "https://openagents.com",
            controlTokenOverride: environment["PYLON_CONTROL_TOKEN"]
        )
    }
}

protocol PylonProcessLaunching {
    func launchPylonNode(configuration: PylonSupervisorConfiguration) throws -> PylonChildProcess
}

protocol PylonChildProcess: AnyObject {
    var isRunning: Bool { get }
    var terminationHandler: (() -> Void)? { get set }
    func terminate()
}

final class ProcessPylonLauncher: PylonProcessLaunching {
    func launchPylonNode(configuration: PylonSupervisorConfiguration) throws -> PylonChildProcess {
        try FileManager.default.createDirectory(at: configuration.appManagedPylonHome, withIntermediateDirectories: true)
        let process = Process()
        process.executableURL = configuration.bundledBunExecutable
        if configuration.bundledBunExecutable.lastPathComponent == "env" {
            process.arguments = ["bun", configuration.bundledPylonEntry.path, "node"]
        } else {
            process.arguments = [configuration.bundledPylonEntry.path, "node"]
        }
        var environment = ProcessInfo.processInfo.environment
        environment["PYLON_HOME"] = configuration.appManagedPylonHome.path
        environment["PYLON_APPLE_FM_SUPERVISE"] = "1"
        environment["OPENAGENTS_APPLE_FM_BRIDGE_PATH"] = configuration.appleFmBridgePath.path
        environment["PYLON_ASSIGNMENT_WORKER"] = "1"
        environment["PYLON_OPENAGENTS_BASE_URL"] = configuration.openAgentsBaseURL
        environment.removeValue(forKey: "CODEX_HOME")
        process.environment = environment
        process.standardOutput = Pipe()
        process.standardError = Pipe()
        try process.run()
        return ProcessChild(process: process)
    }
}

private final class ProcessChild: PylonChildProcess {
    private let process: Process
    var terminationHandler: (() -> Void)?
    var isRunning: Bool { process.isRunning }

    init(process: Process) {
        self.process = process
        process.terminationHandler = { [weak self] _ in self?.terminationHandler?() }
    }

    func terminate() { if process.isRunning { process.terminate() } }
}

@MainActor
final class PylonSupervisor: ObservableObject {
    @Published private(set) var snapshot: PylonSupervisorSnapshot

    private let configuration: PylonSupervisorConfiguration
    private let launcher: PylonProcessLaunching
    private let session: URLSession
    private var child: PylonChildProcess?
    private var token: String?
    private var ownsChild = false

    init(
        configuration: PylonSupervisorConfiguration = .default(),
        launcher: PylonProcessLaunching = ProcessPylonLauncher(),
        session: URLSession = .shared
    ) {
        self.configuration = configuration
        self.launcher = launcher
        self.session = session
        self.snapshot = PylonSupervisorSnapshot(
            controlURL: configuration.controlURL,
            pylonHome: configuration.appManagedPylonHome
        )
    }

    func start() async {
        if await attachExisting() {
            await refreshReadiness()
            return
        }
        await bootBundled()
        await refreshReadiness()
    }

    func stop() {
        guard ownsChild else {
            snapshot.mode = .stopped
            return
        }
        child?.terminationHandler = nil
        child?.terminate()
        child = nil
        ownsChild = false
        snapshot.mode = .stopped
        appendLog("Stopped bundled Pylon.")
    }

    func refreshReadiness() async {
        guard token != nil else { return }
        async let accounts = controlCommand(["type": "accounts.list"])
        async let accountStatus = controlCommand(["type": "accounts.status"])
        async let appleFm = controlCommand(["type": "apple_fm.status"])
        async let assignments = controlCommand(["type": "assignments.poll"])
        let results = await (accounts, accountStatus, appleFm, assignments)
        snapshot.accountsSummary = summarize(results.0) ?? summarize(results.1) ?? "No accounts projected"
        snapshot.capacitySummary = summarize(results.2) ?? "Apple FM unavailable"
        snapshot.assignmentsSummary = summarize(results.3) ?? "No assignments"
        snapshot.identitySummary = "Control: \(configuration.controlURL.host ?? "loopback")"
    }

    private func attachExisting() async -> Bool {
        guard await healthIsReachable() else { return false }
        guard let resolved = readExistingControlToken() else {
            snapshot.lastError = "Pylon is running, but no local control token was available."
            return false
        }
        token = resolved
        ownsChild = false
        snapshot.mode = .connectedExisting
        snapshot.pylonHome = configuration.existingPylonHome
        appendLog("Attached to existing Pylon at \(configuration.controlURL.absoluteString).")
        return true
    }

    private func bootBundled() async {
        snapshot.mode = .bootingBundled
        snapshot.pylonHome = configuration.appManagedPylonHome
        do {
            child = try launcher.launchPylonNode(configuration: configuration)
            ownsChild = true
            child?.terminationHandler = { [weak self] in
                Task { @MainActor in self?.handleChildExit() }
            }
            token = readControlToken(from: configuration.appManagedPylonHome)
            snapshot.mode = .bundledRunning
            appendLog("Launched bundled Pylon with app-managed PYLON_HOME.")
        } catch {
            snapshot.mode = .disconnected
            snapshot.lastError = error.localizedDescription
            appendLog("Bundled Pylon launch failed.")
        }
    }

    private func handleChildExit() {
        guard ownsChild else { return }
        appendLog("Bundled Pylon exited; attempting one recovery launch.")
        ownsChild = false
        child = nil
        Task { await bootBundled() }
    }

    private func healthIsReachable() async -> Bool {
        do {
            let (data, response) = try await session.data(from: configuration.controlURL.appendingPathComponent("health"))
            guard (response as? HTTPURLResponse)?.statusCode == 200 else { return false }
            let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            return object?["ok"] as? Bool == true
        } catch {
            return false
        }
    }

    private func controlCommand(_ command: [String: Any]) async -> Any? {
        guard let token else { return nil }
        do {
            var request = URLRequest(url: configuration.controlURL.appendingPathComponent("command"))
            request.httpMethod = "POST"
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONSerialization.data(withJSONObject: command)
            let (data, response) = try await session.data(for: request)
            guard (response as? HTTPURLResponse)?.statusCode == 200 else { return nil }
            let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            return object?["result"]
        } catch {
            return nil
        }
    }

    private func readControlToken(from home: URL) -> String? {
        let tokenURL = home.appendingPathComponent("control-token")
        guard let text = try? String(contentsOf: tokenURL, encoding: .utf8) else { return nil }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.count >= 16 ? trimmed : nil
    }

    private func readExistingControlToken() -> String? {
        let override = configuration.controlTokenOverride?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let override, override.count >= 16 { return override }
        return readControlToken(from: configuration.existingPylonHome)
    }

    private func summarize(_ value: Any?) -> String? {
        guard let value else { return nil }
        if let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
           let text = String(data: data, encoding: .utf8) {
            return redact(text)
        }
        return redact(String(describing: value))
    }

    private func redact(_ text: String) -> String {
        text.replacingOccurrences(of: #"Bearer\s+[A-Za-z0-9._-]+"#, with: "Bearer [redacted]", options: .regularExpression)
            .replacingOccurrences(of: #""token"\s*:\s*"[^"]+""#, with: #""token":"[redacted]""#, options: .regularExpression)
            .replacingOccurrences(of: #"oa_agent_[A-Za-z0-9._-]+"#, with: "oa_agent_[redacted]", options: .regularExpression)
    }

    private func appendLog(_ line: String) {
        snapshot.logLines.append(redact(line))
        if snapshot.logLines.count > 20 { snapshot.logLines.removeFirst(snapshot.logLines.count - 20) }
    }
}
