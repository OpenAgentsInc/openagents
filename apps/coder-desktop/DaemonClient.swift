// Local Unix-socket client for the unified OpenAgents daemon (`coder host serve`).
// Speaks the newline-framed `openagents.environment.daemon.v1alpha1` control plane
// over ~/.openagents/coder/host/service.sock so Coder Desktop and OpenAgents Mobile
// share daemon health, Foreman policy, and session metadata (pin, rename, archive).
// It also owns Coder's worker runs (GitHub #13694): StartRun, StopRun, ListRuns and
// ForgetRun, with each run's lines in files under host/runs (coder/tui/host-runs.coax).
import Darwin
import Foundation

struct DaemonDiscovery: Decodable {
    let schema: String?
    let host: String?
    let pid: Int?
    let socket: String?
    let status: String?
    let updated_at: Int?
}

struct DaemonForemanState: Decodable {
    let tier: Int?
    let prompt: String?
    let updated: Int?
}

struct DaemonSessionMeta: Decodable {
    let session_id: String
    enum CodingKeys: String, CodingKey { case session_id = "id", title, folder, pinned, archived, updated }
    let title: String?
    let folder: String?
    let pinned: Bool?
    let archived: Bool?
    let updated: Int?
}

/// One worker run the daemon owns: running, stopping, exited, stopped or lost.
struct DaemonRun: Decodable {
    let run_id: String
    let state: String
    let pid: Int?
    let started: Int?
    let ended: Int?
    let exit: Int?
    let out: Int?
    var isLive: Bool { state == "running" || state == "stopping" }
}

struct DaemonView: Decodable {
    let schema: String?
    let ok: Bool?
    let host: String?
    let label: String?
    let method: String?
    let pid: Int?
    let daemon_status: String?
    let foreman: DaemonForemanState?
    let meta: [DaemonSessionMeta]?
    let runs: [DaemonRun]?
    let status: String?
    let error: String?
    enum CodingKeys: String, CodingKey { case schema, ok, host, label, method, pid, daemon_status, foreman, meta = "sessions_meta", runs, status, error }
}

final class CoderDaemonClient {
    static let schema = "openagents.environment.daemon.v1alpha1"
    private let queue = DispatchQueue(label: "openagents.coder.daemon-client", qos: .utility)
    private let hostDir: URL

    init() {
        let fm = FileManager.default
        // the daemon's own rule (hs_path): $CODER_HOME, else $HOME, then .openagents/coder/host
        if let custom = ProcessInfo.processInfo.environment["CODER_HOME"], !custom.isEmpty {
            self.hostDir = URL(fileURLWithPath: custom, isDirectory: true)
                .appendingPathComponent(".openagents/coder/host", isDirectory: true)
        } else {
            self.hostDir = fm.homeDirectoryForCurrentUser
                .appendingPathComponent(".openagents/coder/host", isDirectory: true)
        }
    }

    var discoveryURL: URL { hostDir.appendingPathComponent("daemon.json") }
    var socketPath: String {
        if let d = readDiscovery(), let s = d.socket, !s.isEmpty { return s }
        return hostDir.appendingPathComponent("service.sock").path
    }

    func readDiscovery() -> DaemonDiscovery? {
        guard let data = try? Data(contentsOf: discoveryURL) else { return nil }
        return try? JSONDecoder().decode(DaemonDiscovery.self, from: data)
    }

    var isDaemonAvailable: Bool {
        FileManager.default.fileExists(atPath: socketPath)
    }

    func describeDaemon(completion: @escaping (DaemonView?) -> Void) {
        let req: [String: Any] = [
            "schema": Self.schema,
            "method": "DaemonHealth"
        ]
        sendAsync(req, completion: completion)
    }

    func readForeman(completion: @escaping (DaemonForemanState?) -> Void) {
        let req: [String: Any] = [
            "schema": Self.schema,
            "method": "ReadForeman"
        ]
        sendAsync(req) { view in
            completion(view?.foreman)
        }
    }

    func updateForeman(tier: Int, prompt: String? = nil, completion: ((Bool) -> Void)? = nil) {
        let now = Int(Date().timeIntervalSince1970 * 1000)
        var req: [String: Any] = [
            "schema": Self.schema,
            "method": "UpdateForeman",
            "command_id": Self.newCommandID(),
            "tier": tier,
            "issued_at": now,
            "expires_at": now + 60_000
        ]
        if let prompt { req["text"] = prompt }
        sendAsync(req) { view in
            completion?(view?.ok == true)
        }
    }

    func listSessionMeta(completion: @escaping ([DaemonSessionMeta]) -> Void) {
        let req: [String: Any] = [
            "schema": Self.schema,
            "method": "ListSessionMeta"
        ]
        sendAsync(req) { view in
            completion(view?.meta ?? [])
        }
    }

    func updateSessionMeta(sessionID: String, title: String? = nil, folder: String? = nil, pinned: Bool? = nil, archived: Bool? = nil, completion: ((Bool) -> Void)? = nil) {
        guard !sessionID.isEmpty else { completion?(false); return }
        let now = Int(Date().timeIntervalSince1970 * 1000)
        var req: [String: Any] = [
            "schema": Self.schema,
            "method": "UpdateSessionMeta",
            "session_id": sessionID,
            "command_id": Self.newCommandID(),
            "issued_at": now,
            "expires_at": now + 60_000
        ]
        if let title { req["title"] = title }
        if let folder { req["folder"] = folder }
        if let pinned { req["pinned"] = pinned }
        if let archived { req["archived"] = archived }
        sendAsync(req) { view in
            completion?(view?.ok == true)
        }
    }

    // MARK: worker runs (GitHub #13694)

    /// The folder the daemon keeps runs in: ID.spec, ID.in, ID.out, ID.state, ID.attached.
    var runsDir: URL { hostDir.appendingPathComponent("runs", isDirectory: true) }
    /// Whether the running daemon owns worker runs: it answered ListRuns.
    private(set) var ownsRuns = false

    /// A daemon that answers, started from the bundled `coder-host serve` when none does,
    /// and whether it owns runs. Waits up to `wait` seconds for a daemon it started.
    @discardableResult func ensureDaemon(wait: TimeInterval = 4) -> Bool {
        if probeRuns() { return true }
        guard let host = Bundle.main.path(forResource: "coder-host", ofType: nil) else { return false }
        try? FileManager.default.createDirectory(at: hostDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        guard spawnDetached(host, ["serve"], log: hostDir.appendingPathComponent("daemon.log").path) else { return false }
        let until = Date().addingTimeInterval(wait)
        while Date() < until {
            usleep(100_000)
            if probeRuns() { return true }
        }
        return false
    }
    private func probeRuns() -> Bool {
        let view = sendSync(["schema": Self.schema, "method": "ListRuns"])
        ownsRuns = view?.ok == true
        return ownsRuns
    }
    /// The daemon in a session of its own, so it outlives Coder: stdin from /dev/null,
    /// stdout and stderr appended to the daemon's log.
    private func spawnDetached(_ path: String, _ args: [String], log: String) -> Bool {
        var actions: posix_spawn_file_actions_t?
        var attrs: posix_spawnattr_t?
        posix_spawn_file_actions_init(&actions); defer { posix_spawn_file_actions_destroy(&actions) }
        posix_spawnattr_init(&attrs); defer { posix_spawnattr_destroy(&attrs) }
        posix_spawn_file_actions_addopen(&actions, 0, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_addopen(&actions, 1, log, O_WRONLY | O_CREAT | O_APPEND, 0o600)
        posix_spawn_file_actions_adddup2(&actions, 1, 2)
        posix_spawnattr_setflags(&attrs, Int16(POSIX_SPAWN_SETSID))
        let argv: [UnsafeMutablePointer<CChar>?] = ([path] + args).map { strdup($0) } + [nil]
        defer { for a in argv { free(a) } }
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = CoderWorker.path
        env["BROWSER"] = env["BROWSER"] ?? "/usr/bin/true"
        let envp: [UnsafeMutablePointer<CChar>?] = env.map { strdup($0.key + "=" + $0.value) } + [nil]
        defer { for e in envp { free(e) } }
        var pid: pid_t = 0
        guard posix_spawn(&pid, path, &actions, &attrs, argv, envp) == 0 else { return false }
        Self.spawnedPID = pid
        return true
    }
    /// The `coder-host serve` this Coder started, if it started one: a scratch test launch stops it by
    /// this PID when it quits (TestLaunch in AppDelegate.swift, GitHub #13752).
    static var spawnedPID: pid_t = 0
    /// StartRun: the daemon starts the worker its ID.spec names. nil when it runs, else why not.
    func startRun(_ id: String) -> String? {
        guard let view = sendSync(["schema": Self.schema, "method": "StartRun", "run_id": id,
                                   "command_id": Self.newCommandID()]) else { return "unreachable" }
        return view.ok == true ? nil : (view.error ?? "failed")
    }
    func stopRun(_ id: String, then: (() -> Void)? = nil) {
        sendAsync(["schema": Self.schema, "method": "StopRun", "run_id": id, "command_id": Self.newCommandID()]) { _ in then?() }
    }
    func forgetRun(_ id: String) {
        sendAsync(["schema": Self.schema, "method": "ForgetRun", "run_id": id, "command_id": Self.newCommandID()]) { _ in }
    }
    func listRuns() -> [DaemonRun]? {
        guard let view = sendSync(["schema": Self.schema, "method": "ListRuns"]), view.ok == true else { return nil }
        return view.runs ?? []
    }

    private static func newCommandID() -> String {
        UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
    }

    private func sendAsync(_ payload: [String: Any], completion: @escaping (DaemonView?) -> Void) {
        queue.async { [weak self] in
            guard let self else { return }
            let result = self.sendSync(payload)
            DispatchQueue.main.async { completion(result) }
        }
    }

    private func sendSync(_ payload: [String: Any]) -> DaemonView? {
        let path = socketPath
        guard FileManager.default.fileExists(atPath: path) else { return nil }
        guard let json = try? JSONSerialization.data(withJSONObject: payload, options: []) else { return nil }
        var line = json
        line.append(0x0A)

        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { return nil }
        defer { close(fd) }

        var tv = timeval(tv_sec: 2, tv_usec: 0)
        setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, socklen_t(MemoryLayout<timeval>.size))

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = path.utf8CString
        let maxLen = MemoryLayout.size(ofValue: addr.sun_path)
        guard pathBytes.count <= maxLen else { return nil }
        withUnsafeMutablePointer(to: &addr.sun_path) { ptr in
            ptr.withMemoryRebound(to: CChar.self, capacity: maxLen) { dst in
                for i in 0..<pathBytes.count { dst[i] = pathBytes[i] }
            }
        }

        let addrLen = socklen_t(MemoryLayout<sockaddr_un>.size)
        let rc = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                connect(fd, sa, addrLen)
            }
        }
        guard rc == 0 else { return nil }

        let sent: Bool = line.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return false }
            var offset = 0
            while offset < raw.count {
                let w = Darwin.write(fd, base.advanced(by: offset), raw.count - offset)
                if w <= 0 { return false }
                offset += w
            }
            return true
        }
        guard sent else { return nil }

        var response = Data()
        var buf = [UInt8](repeating: 0, count: 4096)
        while response.count < 1_048_576 {
            let r = Darwin.read(fd, &buf, buf.count)
            if r <= 0 { break }
            if let nl = buf[0..<r].firstIndex(of: 0x0A) {
                response.append(contentsOf: buf[0..<nl])
                break
            }
            response.append(contentsOf: buf[0..<r])
        }
        guard !response.isEmpty else { return nil }
        return try? JSONDecoder().decode(DaemonView.self, from: response)
    }
}
