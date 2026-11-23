// Firm reason: 6 (macOS Tailscale host listener on port 47109 + Bonjour advertiser + tailscale whois enforcement + relay-free direct-grant minter + NIP-44 Unix-socket forwarder + live push broadcaster + LaunchAgent manager).
import Foundation
import Network

@MainActor
final class TailscaleHostServer: ObservableObject {
    @Published var isListening = false
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

    func setWorkspacePath(_ path: String) {
        let clean = (path as NSString).expandingTildeInPath
        guard FileManager.default.fileExists(atPath: clean) else { return }
        workspacePath = clean
        UserDefaults.standard.set(clean, forKey: Self.workspaceDefaultsKey)
    }

    func refreshEnvironment() {
        daemonRunning = LocalSocketClient.shared.isSocketAvailable
        launchAgentInstalled = FileManager.default.fileExists(atPath: launchAgentURL.path)
        agyPath = Self.findExecutable(names: ["agy", "antigravity"], extraPaths: [
            "/opt/homebrew/bin/agy",
            "/usr/local/bin/agy",
            "\(FileManager.default.homeDirectoryForCurrentUser.path)/.local/bin/agy",
        ])
        coderHostPath = Self.findCoderHostBinary()
        hostKeyHex = Self.readLocalHostPublicKey() ?? ""

        Task.detached { [weak self] in
            let owner = Self.queryTailscaleOwner()
            await MainActor.run {
                guard let self else { return }
                self.owner = owner
                if let owner {
                    self.statusNote = self.daemonRunning
                        ? "Tailscale ready (\(owner.dns.isEmpty ? owner.ip : owner.dns))"
                        : "Tailscale ready — start local host daemon to serve Antigravity"
                } else {
                    self.statusNote = "Tailscale is not running or this Mac is tagged."
                }
            }
        }
    }

    func start() {
        refreshEnvironment()
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
                            self.listenPort = nwListener.port?.rawValue ?? AppTailnet.port
                            self.updateBonjourAdvertisement()
                        case .failed(let err):
                            self.isListening = false
                            self.statusNote = "Tailscale listener failed: \(err.localizedDescription)"
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
        isListening = false
        for (_, sub) in subscribers { sub.conn.cancel() }
        subscribers.removeAll()
        activeClients = 0
        runsWatcher?.cancel()
        runsWatcher = nil
    }

    private func updateBonjourAdvertisement() {
        guard let listener, let owner, MobileCore.isValidHostKey(hostKeyHex) else { return }
        let record = AppTailnetComputer(
            host: hostKeyHex,
            ip: owner.ip,
            dns: owner.dns,
            port: listenPort,
            name: owner.name.isEmpty ? (Host.current().localizedName ?? "Mac") : owner.name
        )
        let txt = NWTXTRecord(record.txt)
        listener.service = NWListener.Service(name: record.name, type: AppTailnet.service, domain: nil, txtRecord: txt)
    }

    // MARK: - Local coder-host daemon & LaunchAgent management

    func startLocalDaemon() {
        guard !LocalSocketClient.shared.isSocketAvailable else {
            daemonRunning = true
            return
        }
        guard let bin = coderHostPath else {
            statusNote = "coder-host binary not found. Run `apps/openagents-macos/install.sh` or put coder-host on PATH."
            return
        }
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: bin)
        proc.arguments = ["serve", "--no-relay"]
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

    /// Launches `agyworker --connect` (or `agy auth login`) to sign this Mac into Antigravity OAuth.
    func connectAntigravityAuth() {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let worker = Self.findExecutable(names: ["agyworker"], extraPaths: [
            "\(home)/.openagents/bin/agyworker",
            "/Applications/OpenAgents.app/Contents/Resources/agyworker",
        ])
        if let worker {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: worker)
            proc.arguments = ["--connect"]
            try? proc.run()
            statusNote = "Opening Antigravity sign-in via agyworker --connect…"
            return
        }
        if let agy = agyPath {
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: agy)
            proc.arguments = ["auth", "login"]
            try? proc.run()
            statusNote = "Opening Antigravity CLI login…"
            return
        }
        statusNote = "Install the Antigravity CLI (`agy`) first."
    }

    func installLaunchAgent() {
        guard let bin = coderHostPath else {
            statusNote = "Build or install coder-host first to register LaunchAgent."
            return
        }
        let plist: [String: Any] = [
            "Label": Self.launchAgentLabel,
            "ProgramArguments": [bin, "serve", "--no-relay"],
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
            _ = Self.runCommand("/bin/launchctl", ["unload", launchAgentURL.path])
            _ = Self.runCommand("/bin/launchctl", ["load", launchAgentURL.path])
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
        _ = Self.runCommand("/bin/launchctl", ["unload", launchAgentURL.path])
        try? FileManager.default.removeItem(at: launchAgentURL)
        launchAgentInstalled = false
        statusNote = "Removed background LaunchAgent."
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
        let hostLabel = owner?.name.isEmpty == false ? (owner?.name ?? "Mac") : (Host.current().localizedName ?? "Mac")

        // 1. Relay-free Direct Tailscale Grant request (`openagents.host-tailnet-grant-request.v1`)
        if let req = AppTailnet.parseDirectGrantRequest(line) {
            bgQueue.async {
                let whois = Self.queryTailscaleWhois(remoteIP: remoteIP)
                if let refusal = AppTailnet.decide(remote: remoteIP, whois: whois, owner: ownerSnapshot) {
                    let ans = AppTailnet.DirectGrantAnswer(host: "", label: hostLabel, grant: nil, epoch: 0, expires: 0, refused: refusal.rawValue)
                    let reply = AppTailnet.directGrantAnswerLine(ans)
                    conn.send(content: reply, completion: .contentProcessed { _ in conn.cancel() })
                    return
                }
                guard let hostBin else {
                    let ans = AppTailnet.DirectGrantAnswer(host: "", label: hostLabel, grant: nil, epoch: 0, expires: 0, refused: AppTailnet.Refusal.unavailable.rawValue)
                    conn.send(content: AppTailnet.directGrantAnswerLine(ans), completion: .contentProcessed { _ in conn.cancel() })
                    return
                }
                let out = Self.runCommand(hostBin, [
                    "direct-grant",
                    "--device", req.device,
                    "--workspace", workspace,
                    "--label", hostLabel,
                    "--client-label", req.label,
                ])
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
                let whois = Self.queryTailscaleWhois(remoteIP: remoteIP)
                let refusal = AppTailnet.decide(remote: remoteIP, whois: whois, owner: ownerSnapshot)
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
        Task {
            let result = await LocalSocketClient.shared.exchangeLine(line)
            if case .success(var reply) = result, !reply.isEmpty {
                if reply.last != 0x0A { reply.append(0x0A) }
                conn.send(content: reply, completion: .contentProcessed { _ in })
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

    // MARK: - Helpers (tailscale CLI, host pubkey, executable discovery)

    nonisolated private static func extractIP(from endpoint: NWEndpoint) -> String {
        if case .hostPort(let host, _) = endpoint {
            return AppTailnet.plain("\(host)")
        }
        return ""
    }

    nonisolated private static func queryTailscaleOwner() -> AppTailnet.Owner? {
        guard let bin = findTailscaleBinary(),
              let out = runCommand(bin, ["status", "--json"]),
              let data = out.data(using: .utf8) else { return nil }
        return AppTailnet.owner(status: data)
    }

    nonisolated private static func queryTailscaleWhois(remoteIP: String) -> Data? {
        guard !remoteIP.isEmpty,
              let bin = findTailscaleBinary(),
              let out = runCommand(bin, ["whois", "--json", remoteIP]) else { return nil }
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
        return findExecutable(names: ["coder-host"], extraPaths: [
            "\(home)/.openagents/bin/coder-host",
            "/opt/homebrew/bin/coder-host",
            "/usr/local/bin/coder-host",
        ])
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

    nonisolated private static func readLocalHostPublicKey() -> String? {
        let pubURL = LocalSocketClient.shared.hostDirectory.appendingPathComponent("host.pub")
        if let raw = try? String(contentsOf: pubURL, encoding: .utf8) {
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            if AppTailnet.isHostKey(trimmed) { return trimmed }
        }
        return nil
    }

    nonisolated private static func runCommand(_ executable: String, _ args: [String]) -> String? {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: executable)
        proc.arguments = args
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            proc.waitUntilExit()
            guard proc.terminationStatus == 0 else { return nil }
            return String(data: data, encoding: .utf8)
        } catch {
            return nil
        }
    }
}
