// Firm reason: 6 (macOS Tailscale host listener on port 47109 + Bonjour advertiser + NWPathMonitor + tailscale whois enforcement + relay-free direct-grant minter + NIP-44 Unix-socket forwarder + live push broadcaster + LaunchAgent manager + AntigravityAuthController).
import Darwin
import Foundation
import Network

@MainActor
final class TailscaleHostServer: ObservableObject {
    let authController = AntigravityAuthController()

    @Published var isListening = false
    @Published var headlessOwnsTailnet = false
    @Published var listenPort: UInt16 = AppTailnet.port
    @Published var owner: AppTailnet.Owner?
    @Published var hostKeyHex: String = ""
    @Published var activeClients: Int = 0
    @Published var daemonRunning: Bool = false
    @Published var launchAgentInstalled: Bool = false
    @Published var agyPath: String?
    @Published var coderHostPath: String?
    @Published var workspacePath: String
    @Published var statusNote: String = "Checking Tailscale & local host daemon…"

    private var listener: NWListener?
    private var pathMonitor: NWPathMonitor?
    private var lastPathSig: String?
    private var subscribers: [ObjectIdentifier: (conn: NWConnection, sessionFilter: String)] = [:]
    private var pushSeq: Int64 = 0
    private var daemonProcess: Process?
    private var runsWatcher: DispatchSourceFileSystemObject?
    private let bgQueue = DispatchQueue(label: "com.openagents.macos.hostserver", qos: .userInitiated)

    private static let workspaceDefaultsKey = "oa.macos.host.workspacePath"
    private static let launchAgentLabel = "com.openagents.coder-host"

    init() {
        let saved = UserDefaults.standard.string(forKey: Self.workspaceDefaultsKey)
        if let saved, !saved.isEmpty, FileManager.default.fileExists(atPath: saved) {
            self.workspacePath = saved
        } else {
            self.workspacePath = FileManager.default.homeDirectoryForCurrentUser.path
        }
        refreshEnvironment()
    }

    var launchAgentURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/\(Self.launchAgentLabel).plist")
    }

    private var currentHostLabel: String {
        let raw = owner?.name.isEmpty == false ? (owner?.name ?? "Mac") : (Host.current().localizedName ?? "Mac")
        let clamped = raw.truncatedToUTF8Bytes(80).trimmingCharacters(in: .whitespacesAndNewlines)
        return clamped.isEmpty ? "Mac" : clamped
    }

    func setWorkspacePath(_ path: String, devicePubkey: String = "") {
        let clean = (path as NSString).expandingTildeInPath
        guard FileManager.default.fileExists(atPath: clean) else { return }
        workspacePath = clean
        UserDefaults.standard.set(clean, forKey: Self.workspaceDefaultsKey)
        bootstrapEnvironmentIfPossible(devicePubkey: devicePubkey, updatePrimary: true)
    }

    func refreshEnvironment(probeAuth: Bool = false) {
        daemonRunning = LocalSocketClient.shared.isSocketAvailable
        launchAgentInstalled = FileManager.default.fileExists(atPath: launchAgentURL.path)
        agyPath = AntigravityAuthController.findAgyBinary()
        coderHostPath = Self.findCoderHostBinary()

        let hostBin = coderHostPath
        Task.detached { [weak self] in
            let owner = Self.queryTailscaleOwner()
            let key = Self.readLocalHostPublicKey(coderHostBin: hostBin) ?? ""
            await MainActor.run {
                guard let self else { return }
                self.owner = owner
                if !key.isEmpty {
                    self.hostKeyHex = key
                }
                self.updateBonjourAdvertisement()
                if let owner {
                    self.statusNote = self.daemonRunning
                        ? "Tailscale ready (\(owner.dns.isEmpty ? owner.ip : owner.dns))"
                        : "Tailscale ready — start local host daemon to serve Antigravity"
                } else {
                    self.statusNote = "Tailscale is not running or this Mac is tagged."
                }
            }
        }
        if probeAuth {
            Task { await authController.probe() }
        }
    }

    func start(devicePubkey: String = "") {
        refreshEnvironment(probeAuth: authController.probeResult == nil)
        startPathMonitorIfNeeded()
        if !devicePubkey.isEmpty {
            bootstrapEnvironmentIfPossible(devicePubkey: devicePubkey, updatePrimary: false)
        }
        guard listener == nil else { return }
        do {
            let port = NWEndpoint.Port(rawValue: AppTailnet.port) ?? .any
            let nwListener = try NWListener(using: .tcp, on: port)
            nwListener.stateUpdateHandler = { [weak self] state in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        guard let self else { return }
                        switch state {
                        case .ready:
                            self.isListening = true
                            self.headlessOwnsTailnet = false
                            self.listenPort = nwListener.port?.rawValue ?? AppTailnet.port
                            self.updateBonjourAdvertisement()
                        case .failed(let err):
                            // Headless `coder-host serve --tailnet-listen` already owns TCP :47109 (#14003).
                            if case .posix(let code) = err, code == .EADDRINUSE, LocalSocketClient.shared.isSocketAvailable {
                                self.isListening = true
                                self.headlessOwnsTailnet = true
                                self.listenPort = AppTailnet.port
                                self.statusNote = "Headless coder-host serving Tailscale :\(AppTailnet.port) (stays online when app quits)"
                            } else {
                                self.isListening = false
                                self.headlessOwnsTailnet = false
                                self.statusNote = "Tailscale listener failed: \(err.localizedDescription)"
                            }
                            self.listener?.cancel()
                            self.listener = nil
                        case .cancelled:
                            self.isListening = false
                        default:
                            break
                        }
                    }
                }
            }
            nwListener.newConnectionHandler = { [weak self] conn in
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        self?.acceptConnection(conn)
                    }
                }
            }
            nwListener.start(queue: .main)
            listener = nwListener
            startWatchingRunsForPush()
        } catch {
            statusNote = "Could not bind Tailscale port \(AppTailnet.port): \(error.localizedDescription)"
        }
    }

    func stop() {
        listener?.cancel()
        listener = nil
        pathMonitor?.cancel()
        pathMonitor = nil
        isListening = false
        for (_, sub) in subscribers { sub.conn.cancel() }
        subscribers.removeAll()
        activeClients = 0
        runsWatcher?.cancel()
        runsWatcher = nil
    }

    private func startPathMonitorIfNeeded() {
        guard pathMonitor == nil else { return }
        let monitor = NWPathMonitor()
        monitor.pathUpdateHandler = { [weak self] path in
            let ifaces = path.availableInterfaces.map(\.name).sorted().joined(separator: ",")
            let sig = "\(path.status)|\(ifaces)"
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return }
                    let prev = self.lastPathSig
                    self.lastPathSig = sig
                    if let prev, prev != sig {
                        self.refreshEnvironment()
                    }
                }
            }
        }
        monitor.start(queue: DispatchQueue(label: "com.openagents.macos.hostpath", qos: .utility))
        pathMonitor = monitor
    }

    private func updateBonjourAdvertisement() {
        guard let listener, let owner, MobileCore.isValidHostKey(hostKeyHex) else { return }
        let record = AppTailnetComputer(
            host: hostKeyHex,
            ip: owner.ip,
            dns: owner.dns,
            port: listenPort,
            name: currentHostLabel
        )
        let txt = NWTXTRecord(record.txt)
        listener.service = NWListener.Service(name: record.name, type: AppTailnet.service, domain: nil, txtRecord: txt)
    }

    // MARK: - Local coder-host daemon & LaunchAgent management

    /// Bootstraps `~/.openagents/coder/host/environment.json` on a fresh Mac (or updates it when the
    /// user chooses a new workspace root) via Coax `coder-host direct-grant` (#14001).
    func bootstrapEnvironmentIfPossible(devicePubkey: String, updatePrimary: Bool) {
        let envJSON = LocalSocketClient.shared.hostDirectory.appendingPathComponent("environment.json")
        if !updatePrimary && FileManager.default.fileExists(atPath: envJSON.path) {
            return
        }
        guard let hostBin = coderHostPath,
              MobileCore.isValidHostKey(devicePubkey) else { return }
        let ws = workspacePath
        let label = currentHostLabel
        bgQueue.async { [weak self] in
            var args = [
                "direct-grant",
                "--device", devicePubkey,
                "--workspace", ws,
                "--label", label,
                "--client-label", "OpenAgents macOS",
            ]
            if updatePrimary {
                args.append("--update-primary")
            }
            _ = Self.runCommand(hostBin, args, timeoutSeconds: 10)
            let key = Self.readLocalHostPublicKey(coderHostBin: hostBin) ?? ""
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return }
                    if !key.isEmpty {
                        self.hostKeyHex = key
                        self.updateBonjourAdvertisement()
                    }
                }
            }
        }
    }

    func startLocalDaemon(devicePubkey: String = "") {
        guard !LocalSocketClient.shared.isSocketAvailable else {
            daemonRunning = true
            return
        }
        guard let bin = coderHostPath else {
            statusNote = "coder-host binary not found. Run `apps/openagents-macos/install.sh` or put coder-host on PATH."
            return
        }
        if !devicePubkey.isEmpty {
            bootstrapEnvironmentIfPossible(devicePubkey: devicePubkey, updatePrimary: false)
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: bin)
        proc.arguments = ["serve", "--no-relay", "--tailnet-listen"]
        proc.environment = Self.daemonEnvironment()
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            daemonProcess = proc
            statusNote = "Starting local headless host daemon…"
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
                self?.refreshEnvironment()
            }
        } catch {
            statusNote = "Failed to launch coder-host: \(error.localizedDescription)"
        }
    }

    func stopInAppDaemonIfRunning() {
        guard let proc = daemonProcess else { return }
        if proc.isRunning {
            proc.terminate()
            let deadline = Date().addingTimeInterval(2.0)
            while proc.isRunning && Date() < deadline {
                usleep(50_000)
            }
        }
        daemonProcess = nil
    }

    func installLaunchAgent(devicePubkey: String = "") {
        guard let bin = coderHostPath else {
            statusNote = "Build or install coder-host first to register LaunchAgent."
            return
        }
        if !devicePubkey.isEmpty {
            bootstrapEnvironmentIfPossible(devicePubkey: devicePubkey, updatePrimary: false)
        }
        // Stop in-app daemon first so launchd owns service.sock cleanly without a bind collision (#14001).
        stopInAppDaemonIfRunning()

        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let envPath = "/opt/homebrew/bin:/usr/local/bin:\(home)/.local/bin:\(home)/.openagents/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        let plist: [String: Any] = [
            "Label": Self.launchAgentLabel,
            "ProgramArguments": [bin, "serve", "--no-relay", "--tailnet-listen"],
            "EnvironmentVariables": [
                "PATH": envPath,
                "HOME": home,
                "TAILSCALE_BE_CLI": "1",
                "TERM": "dumb",
                "BROWSER": "/usr/bin/true",
            ],
            "RunAtLoad": true,
            "KeepAlive": true,
            "StandardOutPath": LocalSocketClient.shared.hostDirectory.appendingPathComponent("launchd.out.log").path,
            "StandardErrorPath": LocalSocketClient.shared.hostDirectory.appendingPathComponent("launchd.err.log").path,
        ]
        do {
            let dir = launchAgentURL.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
            try data.write(to: launchAgentURL, options: .atomic)
            let domain = "gui/\(getuid())"
            _ = Self.runCommand("/bin/launchctl", ["bootout", "\(domain)/\(Self.launchAgentLabel)"], timeoutSeconds: 8)
            let boot = Self.runCommand("/bin/launchctl", ["bootstrap", domain, launchAgentURL.path], timeoutSeconds: 8)
            if boot == nil {
                _ = Self.runCommand("/bin/launchctl", ["unload", launchAgentURL.path], timeoutSeconds: 8)
                _ = Self.runCommand("/bin/launchctl", ["load", launchAgentURL.path], timeoutSeconds: 8)
            }
            launchAgentInstalled = true
            statusNote = "Installed LaunchAgent (\(Self.launchAgentLabel)) — serves Antigravity in background."
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) { [weak self] in
                self?.refreshEnvironment()
            }
        } catch {
            statusNote = "Could not install LaunchAgent: \(error.localizedDescription)"
        }
    }

    func removeLaunchAgent() {
        let domain = "gui/\(getuid())"
        let bootout = Self.runCommand("/bin/launchctl", ["bootout", "\(domain)/\(Self.launchAgentLabel)"], timeoutSeconds: 8)
        if bootout == nil {
            _ = Self.runCommand("/bin/launchctl", ["unload", launchAgentURL.path], timeoutSeconds: 8)
        }
        try? FileManager.default.removeItem(at: launchAgentURL)
        launchAgentInstalled = false
        statusNote = "Removed background LaunchAgent."
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            self?.refreshEnvironment()
        }
    }

    // MARK: - Incoming Tailscale connection handling

    private func acceptConnection(_ conn: NWConnection) {
        let remoteIP = Self.extractIP(from: conn.endpoint)
        conn.stateUpdateHandler = { [weak self] state in
            switch state {
            case .ready:
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        self?.readFirstLine(conn: conn, remoteIP: remoteIP, buffer: Data())
                    }
                }
            case .failed, .cancelled:
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        self?.removeSubscriber(conn)
                    }
                }
            default:
                break
            }
        }
        conn.start(queue: .main)
    }

    private func readFirstLine(conn: NWConnection, remoteIP: String, buffer: Data) {
        conn.receive(minimumIncompleteLength: 1, maximumLength: AppTailnet.answerMax) { [weak self] data, _, complete, error in
            var buf = buffer
            if let data { buf.append(data) }
            if let nl = buf.firstIndex(of: 0x0A) {
                let line = Data(buf[buf.startIndex..<nl])
                let rest = Data(buf[buf.index(after: nl)...])
                DispatchQueue.main.async {
                    MainActor.assumeIsolated {
                        self?.handleFirstFrame(conn: conn, remoteIP: remoteIP, line: line, remaining: rest)
                    }
                }
                return
            }
            if buf.count > AppTailnet.answerMax || error != nil || complete {
                conn.cancel()
                return
            }
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    self?.readFirstLine(conn: conn, remoteIP: remoteIP, buffer: buf)
                }
            }
        }
    }

    private func handleFirstFrame(conn: NWConnection, remoteIP: String, line: Data, remaining: Data) {
        let ownerSnapshot = owner
        let workspace = workspacePath
        let hostBin = coderHostPath
        let hostLabel = currentHostLabel

        // 1. Relay-free Direct Tailscale Grant request (`openagents.host-tailnet-grant-request.v1`)
        if let req = AppTailnet.parseDirectGrantRequest(line) {
            bgQueue.async { [weak self] in
                var effectiveOwner = ownerSnapshot
                if effectiveOwner == nil, let refreshed = Self.queryTailscaleOwner() {
                    effectiveOwner = refreshed
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated {
                            self?.applyRefreshedOwner(refreshed)
                        }
                    }
                }
                let whois = Self.queryTailscaleWhois(remoteIP: remoteIP)
                if let refusal = AppTailnet.decide(remote: remoteIP, whois: whois, owner: effectiveOwner) {
                    let ans = AppTailnet.DirectGrantAnswer(host: "", label: hostLabel, grant: nil, epoch: 0, expires: 0, refused: refusal.rawValue)
                    let reply = AppTailnet.directGrantAnswerLine(ans)
                    conn.send(content: reply, completion: .contentProcessed { _ in conn.cancel() })
                    return
                }
                guard let effectiveHostBin = hostBin ?? Self.findCoderHostBinary() else {
                    let ans = AppTailnet.DirectGrantAnswer(host: "", label: hostLabel, grant: nil, epoch: 0, expires: 0, refused: AppTailnet.Refusal.unavailable.rawValue)
                    conn.send(content: AppTailnet.directGrantAnswerLine(ans), completion: .contentProcessed { _ in conn.cancel() })
                    return
                }
                let safeClientLabel = req.label.truncatedToUTF8Bytes(80)
                let out = Self.runCommand(effectiveHostBin, [
                    "direct-grant",
                    "--device", req.device,
                    "--workspace", workspace,
                    "--label", hostLabel,
                    "--client-label", safeClientLabel,
                ], timeoutSeconds: 10)
                if var data = out?.data(using: .utf8), !data.isEmpty {
                    if let nl = data.firstIndex(of: 0x0A) { data = Data(data[data.startIndex..<nl]) }
                    if AppTailnet.directGrantAnswer(data) != nil {
                        var framed = data
                        framed.append(0x0A)
                        conn.send(content: framed, completion: .contentProcessed { _ in conn.cancel() })
                        return
                    }
                }
                let ans = AppTailnet.DirectGrantAnswer(host: "", label: hostLabel, grant: nil, epoch: 0, expires: 0, refused: AppTailnet.Refusal.unavailable.rawValue)
                conn.send(content: AppTailnet.directGrantAnswerLine(ans), completion: .contentProcessed { _ in conn.cancel() })
            }
            return
        }

        // 2. Direct Tailscale multiplexed transport (`openagents.history-observer-direct.v1`)
        if AppTailnet.isDirectHello(line) {
            bgQueue.async { [weak self] in
                var effectiveOwner = ownerSnapshot
                if effectiveOwner == nil, let refreshed = Self.queryTailscaleOwner() {
                    effectiveOwner = refreshed
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated {
                            self?.applyRefreshedOwner(refreshed)
                        }
                    }
                }
                let whois = Self.queryTailscaleWhois(remoteIP: remoteIP)
                let refusal = AppTailnet.decide(remote: remoteIP, whois: whois, owner: effectiveOwner)
                let ack = AppTailnet.directAckLine(refused: refusal?.rawValue)
                if refusal != nil {
                    conn.send(content: ack, completion: .contentProcessed { _ in conn.cancel() })
                    return
                }
                conn.send(content: ack, completion: .contentProcessed { err in
                    if err != nil {
                        conn.cancel()
                        return
                    }
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated {
                            self?.receiveMultiplexedFrames(conn: conn, buffer: remaining)
                        }
                    }
                })
            }
            return
        }

        conn.cancel()
    }

    private func applyRefreshedOwner(_ refreshed: AppTailnet.Owner) {
        owner = refreshed
        updateBonjourAdvertisement()
        if !headlessOwnsTailnet {
            statusNote = daemonRunning
                ? "Tailscale ready (\(refreshed.dns.isEmpty ? refreshed.ip : refreshed.dns))"
                : "Tailscale ready — start local host daemon to serve Antigravity"
        }
    }

    private func receiveMultiplexedFrames(conn: NWConnection, buffer: Data) {
        var buf = buffer
        while let nl = buf.firstIndex(of: 0x0A) {
            let line = Data(buf[buf.startIndex..<nl])
            buf.removeSubrange(buf.startIndex...nl)
            handleMultiplexedLine(conn: conn, line: line)
        }
        if buf.count > 262_144 {
            removeSubscriber(conn)
            conn.cancel()
            return
        }
        let carry = buf
        conn.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, complete, error in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    guard let self else { return }
                    var next = carry
                    if let data, !data.isEmpty { next.append(data) }
                    // Drain any complete buffered lines BEFORE closing on EOF (#14001).
                    while let nl = next.firstIndex(of: 0x0A) {
                        let line = Data(next[next.startIndex..<nl])
                        next.removeSubrange(next.startIndex...nl)
                        self.handleMultiplexedLine(conn: conn, line: line)
                    }
                    if error != nil || complete {
                        self.removeSubscriber(conn)
                        conn.cancel()
                        return
                    }
                    self.receiveMultiplexedFrames(conn: conn, buffer: next)
                }
            }
        }
    }

    private func handleMultiplexedLine(conn: NWConnection, line: Data) {
        // Real-time push subscription frame (`openagents.host-tailnet-subscribe.v1`)
        if let sessionFilter = AppTailnet.parseSubscribe(line) {
            let key = ObjectIdentifier(conn)
            subscribers[key] = (conn, sessionFilter)
            activeClients = subscribers.count
            return
        }
        // Sealed NIP-44 kind-3188 RPC event -> forward to local `service.sock` and write sealed reply back
        Task { [weak self] in
            let result = await LocalSocketClient.shared.exchangeLine(line)
            switch result {
            case .success(var reply) where !reply.isEmpty:
                if reply.last != 0x0A { reply.append(0x0A) }
                conn.send(content: reply, completion: .contentProcessed { _ in })
            default:
                // Close connection promptly on local daemon failure so remote clients don't hang 18s (#14001).
                self?.removeSubscriber(conn)
                conn.cancel()
            }
        }
    }

    private func removeSubscriber(_ conn: NWConnection) {
        let key = ObjectIdentifier(conn)
        if subscribers.removeValue(forKey: key) != nil {
            activeClients = subscribers.count
        }
    }

    // MARK: - Live push broadcast when ~/.openagents/coder/host/runs changes

    private func startWatchingRunsForPush() {
        runsWatcher?.cancel()
        let dir = LocalSocketClient.shared.runsDirectoryURL
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let fd = open(dir.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .extend, .rename, .delete, .attrib],
            queue: DispatchQueue(label: "com.openagents.macos.pushwatch", qos: .utility)
        )
        src.setEventHandler { [weak self] in
            DispatchQueue.main.async {
                MainActor.assumeIsolated {
                    self?.broadcastPush(kind: "sessions_changed", session: "")
                }
            }
        }
        src.setCancelHandler { close(fd) }
        runsWatcher = src
        src.resume()
    }

    func broadcastPush(kind: String, session: String, revision: Int64 = 0, busy: Bool = false, hasInteraction: Bool = false) {
        guard !subscribers.isEmpty else { return }
        pushSeq += 1
        let ev = AppTailnet.PushEvent(
            seq: pushSeq,
            kind: kind,
            session: session,
            revision: revision,
            busy: busy,
            hasInteraction: hasInteraction
        )
        let frame = AppTailnet.pushLine(ev)
        guard !frame.isEmpty else { return }
        for (_, sub) in subscribers {
            if sub.sessionFilter.isEmpty || session.isEmpty || sub.sessionFilter == session {
                sub.conn.send(content: frame, completion: .contentProcessed { _ in })
            }
        }
    }

    // MARK: - Helpers (tailscale CLI, host pubkey from daemon.json, bounded command runner)

    nonisolated static func daemonEnvironment(base: [String: String] = ProcessInfo.processInfo.environment) -> [String: String] {
        var env = base
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let cur = env["PATH"] ?? "/usr/bin:/bin"
        env["PATH"] = "/opt/homebrew/bin:/usr/local/bin:\(home)/.local/bin:\(home)/.openagents/bin:\(cur)"
        // macOS Tailscale.app's bundled binary enters GUI mode and hangs when launched under
        // LaunchServices (no TERM) unless TAILSCALE_BE_CLI=1 is set.
        env["TAILSCALE_BE_CLI"] = "1"
        if env["TERM"]?.isEmpty != false {
            env["TERM"] = "dumb"
        }
        env["BROWSER"] = "/usr/bin/true"
        return env
    }

    nonisolated private static func extractIP(from endpoint: NWEndpoint) -> String {
        if case .hostPort(let host, _) = endpoint {
            return AppTailnet.plain("\(host)")
        }
        return ""
    }

    nonisolated private static func queryTailscaleOwner() -> AppTailnet.Owner? {
        guard let bin = findTailscaleBinary(),
              let out = runCommand(bin, ["status", "--peers=false", "--json"], timeoutSeconds: 8),
              let data = out.data(using: .utf8) else { return nil }
        return AppTailnet.owner(status: data)
    }

    nonisolated private static func queryTailscaleWhois(remoteIP: String) -> Data? {
        guard !remoteIP.isEmpty,
              let bin = findTailscaleBinary(),
              let out = runCommand(bin, ["whois", "--json", remoteIP], timeoutSeconds: 8) else { return nil }
        return out.data(using: .utf8)
    }

    nonisolated private static func findTailscaleBinary() -> String? {
        findExecutable(names: ["tailscale"], extraPaths: [
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
            "/opt/homebrew/bin/tailscale",
            "/usr/local/bin/tailscale",
        ])
    }

    nonisolated private static func findCoderHostBinary() -> String? {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        var paths: [String] = []
        if let res = Bundle.main.resourceURL {
            paths.append(res.appendingPathComponent("coder-host").path)
        }
        paths.append(contentsOf: [
            "\(home)/.openagents/bin/coder-host",
            "/Applications/OpenAgents.app/Contents/Resources/coder-host",
            "/opt/homebrew/bin/coder-host",
            "/usr/local/bin/coder-host",
        ])
        return findExecutable(names: ["coder-host"], extraPaths: paths)
    }

    nonisolated private static func findExecutable(names: [String], extraPaths: [String]) -> String? {
        let fm = FileManager.default
        for p in extraPaths where fm.isExecutableFile(atPath: p) {
            return p
        }
        let pathEnv = ProcessInfo.processInfo.environment["PATH"] ?? "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
        for dir in pathEnv.split(separator: ":") {
            for name in names {
                let candidate = "\(dir)/\(name)"
                if fm.isExecutableFile(atPath: candidate) { return candidate }
            }
        }
        return nil
    }

    /// Reads the host's 64-hex public key from `daemon.json` (`"host"`), falling back to `host.pub`
    /// or `coder-host key` so Bonjour advertising works reliably on macOS (#14001).
    nonisolated static func readLocalHostPublicKey(coderHostBin: String? = nil) -> String? {
        let hostDir = LocalSocketClient.shared.hostDirectory
        let daemonJSON = hostDir.appendingPathComponent("daemon.json")
        if let data = try? Data(contentsOf: daemonJSON),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            for keyName in ["host", "host_pubkey", "pubkey", "public_key"] {
                if let val = obj[keyName] as? String {
                    let trimmed = val.trimmingCharacters(in: .whitespacesAndNewlines)
                    if AppTailnet.isHostKey(trimmed) { return trimmed }
                }
            }
        }
        let pubURL = hostDir.appendingPathComponent("host.pub")
        if let raw = try? String(contentsOf: pubURL, encoding: .utf8) {
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            if AppTailnet.isHostKey(trimmed) { return trimmed }
        }
        if let bin = coderHostBin,
           let out = runCommand(bin, ["key"], timeoutSeconds: 5) {
            for token in out.split(whereSeparator: \.isWhitespace) {
                let candidate = String(token).trimmingCharacters(in: .whitespacesAndNewlines)
                if AppTailnet.isHostKey(candidate) { return candidate }
            }
        }
        return nil
    }

    /// Bounded subprocess runner with non-blocking stdout drain + strict timeout (#14001).
    nonisolated static func runCommand(_ executable: String, _ args: [String], timeoutSeconds: Int = 10) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: executable)
        proc.arguments = args
        proc.environment = daemonEnvironment()
        let pipe = Pipe()
        proc.standardInput = FileHandle.nullDevice
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice

        let outLock = NSLock()
        var outData = Data()
        let readGroup = DispatchGroup()
        readGroup.enter()
        DispatchQueue.global(qos: .utility).async {
            let d = pipe.fileHandleForReading.readDataToEndOfFile()
            outLock.lock()
            outData = d
            outLock.unlock()
            readGroup.leave()
        }

        let done = DispatchSemaphore(value: 0)
        proc.terminationHandler = { _ in done.signal() }
        do {
            try proc.run()
        } catch {
            return nil
        }

        if done.wait(timeout: .now() + .seconds(max(1, timeoutSeconds))) == .timedOut {
            proc.terminate()
            _ = done.wait(timeout: .now() + .seconds(1))
            return nil
        }
        _ = readGroup.wait(timeout: .now() + .seconds(2))
        guard proc.terminationStatus == 0 else { return nil }
        outLock.lock()
        let result = String(data: outData, encoding: .utf8)
        outLock.unlock()
        return result
    }
}
