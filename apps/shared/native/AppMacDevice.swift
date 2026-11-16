// Firm reason: 6 (macOS process, filesystem and scheduling effects for the resident environment CLI).
#if os(macOS)
import Foundation

/// SDK process carriage only. Fixed callers choose argv; no shell or protocol parser.
enum AppMacProcess {
    struct Command: Sendable {
        var executable: URL
        var arguments: [String]
        var limit: Int
        var timeoutMs: Int
        var environment: [String: String] = [:]
        var directory: URL? = nil
    }
    struct Result: Sendable {
        var started = false
        var status: Int32 = -1
        var bytes = Data()
        var cut = false
        var timedOut = false
        var readFailed = false
        var ok: Bool { started && status == 0 && !cut && !timedOut && !readFailed }
    }
    private final class Owner: @unchecked Sendable {
        let process: Process
        private let lock = NSLock()
        private var finished = false, expired = false
        init(_ process: Process) { self.process = process }
        func expire() {
            lock.lock(); defer { lock.unlock() }
            guard !finished else { return }; expired = true
            if process.isRunning { process.terminate() }
        }
        func finish() -> Bool { lock.lock(); defer { lock.unlock() }; finished = true; return expired }
    }
    static func run(_ command: Command) async -> Result {
        await Task.detached(priority: .utility) {
            guard command.limit >= 0, command.limit <= 2 * 1024 * 1024, command.timeoutMs > 0, command.timeoutMs <= 30000 else { return Result() }
            let process = Process(), pipe = Pipe()
            process.executableURL = command.executable; process.arguments = command.arguments; process.environment = command.environment; process.currentDirectoryURL = command.directory
            process.standardOutput = pipe; process.standardInput = FileHandle.nullDevice; process.standardError = FileHandle.nullDevice
            do { try process.run() } catch { return Result() }
            let owner = Owner(process), deadline = DispatchSource.makeTimerSource(queue: .global(qos: .utility))
            deadline.schedule(deadline: .now() + .milliseconds(command.timeoutMs))
            deadline.setEventHandler { owner.expire() }; deadline.resume()
            defer { deadline.cancel(); try? pipe.fileHandleForReading.close() }
            var result = Result(started: true)
            do {
                while let chunk = try pipe.fileHandleForReading.read(upToCount: 4096), !chunk.isEmpty {
                    guard chunk.count <= command.limit - result.bytes.count else { result.cut = true; if process.isRunning { process.terminate() }; break }
                    result.bytes.append(chunk)
                }
            } catch { result.readFailed = true; if process.isRunning { process.terminate() } }
            process.waitUntilExit(); result.status = process.terminationStatus; result.timedOut = owner.finish()
            return result
        }.value
    }
}

/// The GUI is a client of the existing resident service. It never owns Claude
/// processes, reads its signing key or changes requests produced by Coax.
@MainActor final class AppMacDevice: AppDeviceService {
    typealias Runner = @MainActor (AppMacProcess.Command) async -> AppMacProcess.Result
    private let home: URL
    private let peer: AppDeviceService
    private let run: Runner
    private var localHost: String?
    private var discovering: Task<String, Never>?
    init(home: URL? = nil, peer: AppDeviceService? = nil, run: Runner? = nil) {
        self.home = home ?? AppFileViewStore.folder.deletingLastPathComponent().deletingLastPathComponent()
        self.peer = peer ?? AppDeviceLink.shared
        self.run = run ?? { await AppMacProcess.run($0) }
    }
    private var executable: URL { home.appendingPathComponent(".openagents/bin/openagents-environment") }
    private var socket: URL { home.appendingPathComponent(".openagents/coder/host/service.sock") }
    private func command(_ arguments: [String], limit: Int, timeout: Int) -> AppMacProcess.Command {
        let env = ProcessInfo.processInfo.environment
        return .init(executable: executable, arguments: arguments, limit: limit, timeoutMs: timeout,
                     environment: ["HOME": home.path, "PATH": env["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin", "LANG": env["LANG"] ?? "en_US.UTF-8"], directory: home)
    }
    func list() -> String {
        let peers = peer.list()
        guard let localHost else { return peers }
        // Keep this computer once when it was also paired through the relay.
        let remote = peers.split(separator: "\n").filter { !$0.hasPrefix(localHost + "\t") }.map(String.init).joined(separator: "\n")
        return localHost + "\tThis computer\n" + (remote.isEmpty ? "" : remote + "\n")
    }
    func list(done: @escaping (String) -> Void) {
        if let discovering { Task { done(await discovering.value) }; return }
        let task = Task { [weak self] in
            guard let self else { return "" }
            if FileManager.default.fileExists(atPath: socket.path) {
                let answer = await run(command(["key"], limit: 512, timeout: 2500))
                let key = String(data: answer.bytes, encoding: .utf8)?.split(whereSeparator: \.isNewline).first.map(String.init) ?? ""
                localHost = answer.ok && oa_device_host(key) != 0 ? key : nil
            } else { localHost = nil }
            return list()
        }
        discovering = task
        Task { [weak self] in let rows = await task.value; self?.discovering = nil; done(rows) }
    }
    func pair(_ link: String, done: @escaping (Bool, String) -> Void) { peer.pair(link, done: done) }
    func call(host: String, body: Data, done: @escaping (AppCompletion.State, Data) -> Void) {
        guard host == localHost else { peer.call(host: host, body: body, done: done); return }
        Task {
            // Existing RuntimeRequest bytes; the CLI's existing Coax codec and
            // kernel-peer checks admit them. No GUI-owned process fallback.
            let file = AppPrivateFile(url: home.appendingPathComponent(".openagents/desktop/requests/" + UUID().uuidString + ".json"), limit: 24576)
            guard file.create(body) else { done(.failed, Data()); return }
            defer { file.remove() }
            let answer = await run(command(["local-rpc", "--request-file", file.url.path], limit: 65536, timeout: 20000))
            if answer.ok { done(.done, answer.bytes) } else { done(answer.started ? .unknown : .failed, Data()) }
        }
    }
}
#endif
