// Firm reason: 6 (Bonjour, Network.framework, and the tailscale and coder-host processes: Mac platform effects).
// Coder's half of zero-config pairing with the phone over Tailscale (GitHub #13778; the rules and
// the steps are apps/shared/native/AppTailnet.swift, the protocol NIP-HOST's tailnet admission).
//
// When Tailscale runs on this Mac, Coder listens on the Mac's own tailnet address (port 47109, or
// any free one) and advertises `_openagents-coder._tcp` on the local network with that address,
// the Mac's MagicDNS name, the port and this computer's host key in the TXT record. A phone that
// connects gets a pairing link only when this Mac's `tailscale whois` names it as an untagged
// device of the Mac's own Tailscale user; the link is the one `coder-host pair` prints, so the
// phone pairs over the relay exactly as with a scanned link and the daemon signs its grant.
//
// Without Tailscale (not installed, stopped, logged out, or this Mac a tagged device) nothing
// listens and nothing is advertised, and Coder says nothing about it. It is on unless turned off:
//   defaults write com.openagents.desktop tailnetPairing -bool false
// Pairing over Tailscale gives the phone the working folder Coder uses (your home folder until
// you choose one), as `coder host pair --workspace` does.
import AppKit
import Foundation
import Network

final class CoderTailnet: NSObject {
    static let shared = CoderTailnet()
    /// UserDefaults key: false turns zero-config pairing off. Unset is on.
    static let enabledKey = "tailnetPairing"
    static var enabled: Bool { UserDefaults.standard.object(forKey: enabledKey) as? Bool ?? true }

    private let queue = DispatchQueue(label: "openagents.coder.tailnet", qos: .utility)
    private let workQueue = DispatchQueue(label: "openagents.coder.tailnet.work", qos: .userInitiated, attributes: .concurrent)
    private var timer: DispatchSourceTimer?
    private var pathMonitor: NWPathMonitor?
    private var lastPathSig: String?
    private var listener: NWListener?
    private var service: NetService?            // main thread only
    private var serving: (owner: AppTailnet.Owner, key: String, port: UInt16)?
    private var hostKey: String?
    private var pending = 0                     // pair processes waiting for a phone, at most two
    private var activeConnections = 0           // concurrent active client connections on queue
    private var subscribers: [ObjectIdentifier: (conn: NWConnection, sessionFilter: String)] = [:]
    private var pushSeq: Int64 = 0
    private var runsWatcher: DispatchSourceFileSystemObject?
    private static let maxConcurrentConnections = 32

    /// Starts watching for Tailscale (now, then every two minutes, plus NWPathMonitor). Not in a scratch test launch.
    func start() {
        guard CoderTailnet.enabled, !TestLaunch.active else { return }
        queue.async { [self] in
            startPathMonitorIfNeeded()
            guard timer == nil else { return }
            let t = DispatchSource.makeTimerSource(queue: queue)
            t.schedule(deadline: .now(), repeating: .seconds(120), leeway: .seconds(5))
            t.setEventHandler { [weak self] in self?.check() }
            t.resume(); timer = t
        }
    }

    private func startPathMonitorIfNeeded() {
        guard pathMonitor == nil else { return }
        let m = NWPathMonitor()
        m.pathUpdateHandler = { [weak self] path in
            guard let self else { return }
            let ifaces = path.availableInterfaces.map(\.name).sorted().joined(separator: ",")
            let sig = "\(path.status)|\(ifaces)"
            let prev = self.lastPathSig
            self.lastPathSig = sig
            if let prev, prev != sig, CoderTailnet.enabled {
                self.check()
            }
        }
        m.start(queue: queue)
        pathMonitor = m
    }

    /// Re-evaluates Tailscale listener state immediately after Settings -> Connections changes exposure mode.
    func reload() {
        queue.async { [self] in
            if !CoderTailnet.enabled {
                stop()
                timer?.cancel(); timer = nil
            } else {
                if timer == nil && !TestLaunch.active {
                    let t = DispatchSource.makeTimerSource(queue: queue)
                    t.schedule(deadline: .now() + 120, repeating: .seconds(120), leeway: .seconds(10))
                    t.setEventHandler { [weak self] in self?.check() }
                    t.resume(); timer = t
                }
                check()
            }
        }
    }

    /// Snapshot of current Tailscale listener status for Settings -> Connections.
    func statusSnapshot(completion: @escaping (String) -> Void) {
        queue.async { [self] in
            let text: String
            if !CoderTailnet.enabled {
                text = "Local-only (127.0.0.1)"
            } else if let s = serving {
                let dns = s.owner.dns.isEmpty ? s.owner.ip : s.owner.dns
                text = "Tailscale · \(dns):\(s.port)"
            } else if CoderTailnet.cli == nil {
                text = "Tailscale CLI not installed"
            } else {
                text = "Tailscale · waiting for tailnet"
            }
            DispatchQueue.main.async { completion(text) }
        }
    }

    /// Generates a pairing invitation link on demand for Settings -> Connections -> Pair….
    func createPairingLink(completion: @escaping (String?) -> Void) {
        queue.async { [self] in
            let owner = CoderTailnet.cli.flatMap { CoderTailnet.output($0, ["status", "--json"]) }.flatMap(AppTailnet.owner(status:))
            let label = owner.map { CoderTailnet.label($0) } ?? (Host.current().localizedName ?? "My Mac")
            let link = invitation(label: label)
            DispatchQueue.main.async { completion(link) }
        }
    }

    // ------------------------------------------------------------------ Tailscale

    /// The tailscale command line: the Mac app's own binary, or a Homebrew or standalone install.
    static let cli: String? = [
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale", "/opt/homebrew/bin/tailscale", "/usr/local/bin/tailscale",
    ].first { FileManager.default.isExecutableFile(atPath: $0) }

    /// One short command's standard output, or nil when it fails or takes longer than `timeout`.
    static func output(_ path: String, _ args: [String], timeout: TimeInterval = 5, limit: Int = 1 << 20) -> Data? {
        let p = Process(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: path); p.arguments = args
        p.standardOutput = out; p.standardError = FileHandle.nullDevice; p.standardInput = FileHandle.nullDevice
        var env = ProcessInfo.processInfo.environment
        env["BROWSER"] = "/usr/bin/true"
        env["TAILSCALE_BE_CLI"] = "1"
        if env["TERM"]?.isEmpty != false { env["TERM"] = "dumb" }
        p.environment = env
        do { try p.run() } catch { return nil }
        let killer = DispatchWorkItem { if p.isRunning { p.terminate() } }
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout, execute: killer)
        let data = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit(); killer.cancel()
        return p.terminationReason == .exit && p.terminationStatus == 0 && data.count <= limit ? data : nil
    }

    private func check() {
        let owner = CoderTailnet.cli.flatMap { CoderTailnet.output($0, ["status", "--json"]) }.flatMap(AppTailnet.owner(status:))
        guard CoderTailnet.enabled, let owner, let key = key() else { stop(); return }
        if let s = serving, s.owner == owner, s.key == key { return }
        stop()
        listen(owner: owner, key: key)
    }

    /// This computer's host key (`coder-host key`), the one its pairing links name.
    private func key() -> String? {
        if let hostKey { return hostKey }
        guard let host = Bundle.main.path(forResource: "coder-host", ofType: nil),
              let out = CoderTailnet.output(host, ["key"]),
              let line = String(decoding: out, as: UTF8.self).split(whereSeparator: \.isNewline).first.map(String.init),
              AppTailnet.isHostKey(line) else { return nil }
        hostKey = line
        return line
    }

    // ------------------------------------------------------------------ listening and advertising

    private func listen(owner: AppTailnet.Owner, key: String, port: UInt16 = AppTailnet.port) {
        let params = NWParameters.tcp
        params.allowLocalEndpointReuse = true
        // the Mac's own tailnet address only, never a wildcard or LAN address (NIP-HOST)
        guard let p = NWEndpoint.Port(rawValue: port) else { return }
        params.requiredLocalEndpoint = .hostPort(host: NWEndpoint.Host(owner.ip), port: p)
        guard let l = try? NWListener(using: params) else { if port != 0 { listen(owner: owner, key: key, port: 0) }; return }
        // Keep newConnectionLimit at InfiniteConnectionLimit: NWListener decrements a finite limit
        // permanently on every accept without replenishing it, suspending the inbox after N accepts.
        // Concurrency is bounded instead by activeConnections in serve(_:owner:key:).
        l.newConnectionHandler = { [weak self] conn in self?.serve(conn, owner: owner, key: key) }
        l.stateUpdateHandler = { [weak self, weak l] state in
            guard let self, let l else { return }
            switch state {
            case .ready:
                let got = l.port?.rawValue ?? port
                self.serving = (owner, key, got)
                self.startWatchingRunsForPush()
                self.advertise(AppTailnetComputer(host: key, ip: owner.ip, dns: owner.dns, port: got, name: CoderTailnet.label(owner)))
            case .failed:
                l.cancel()
                if self.listener === l { self.listener = nil; self.serving = nil; self.unadvertise() }
                // the usual port is taken (another Coder, or a daemon serving it): any free one
                if port != 0 { self.listen(owner: owner, key: key, port: 0) }
            default: break
            }
        }
        listener = l
        l.start(queue: queue)
    }

    func stopListening() {
        queue.async { self.stop() }
    }

    private func stop() {
        listener?.cancel(); listener = nil; serving = nil
        runsWatcher?.cancel(); runsWatcher = nil
        for (_, sub) in subscribers { sub.conn.cancel() }
        subscribers.removeAll()
        unadvertise()
    }

    private func startWatchingRunsForPush() {
        runsWatcher?.cancel()
        let dir = CoderDaemonClient().runsDir
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let fd = open(dir.path, O_EVTONLY)
        guard fd >= 0 else { return }
        let src = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fd,
            eventMask: [.write, .extend, .rename, .delete, .attrib],
            queue: queue
        )
        src.setEventHandler { [weak self] in
            self?.broadcastPush(kind: "sessions_changed", session: "")
        }
        src.setCancelHandler { close(fd) }
        runsWatcher = src
        src.resume()
    }

    private func broadcastPush(kind: String, session: String, revision: Int64 = 0, busy: Bool = false, hasInteraction: Bool = false) {
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

    /// The name the phone shows: this Mac's name, as one DNS label at most.
    static func label(_ owner: AppTailnet.Owner) -> String {
        let local = AppTailnetComputer.name(Host.current().localizedName ?? "")
        if !local.isEmpty { return local }
        let ts = AppTailnetComputer.name(owner.name)
        return ts.isEmpty ? "My Mac" : ts
    }

    private func advertise(_ c: AppTailnetComputer) {
        let txt = NetService.data(fromTXTRecord: c.txt.mapValues { Data($0.utf8) })
        DispatchQueue.main.async { [self] in
            service?.stop()
            // NetService publishes the record with no socket of its own; the port it names is the listener's
            let s = NetService(domain: "local.", type: AppTailnet.service + ".", name: c.name, port: Int32(c.port))
            s.setTXTRecord(txt)
            s.publish()
            service = s
        }
    }
    private func unadvertise() { DispatchQueue.main.async { [self] in service?.stop(); service = nil } }

    // ------------------------------------------------------------------ one phone's request

    private func serve(_ conn: NWConnection, owner: AppTailnet.Owner, key: String) {
        if activeConnections >= Self.maxConcurrentConnections {
            conn.cancel()
            return
        }
        activeConnections += 1
        let q = queue
        var released = false
        let closeConn: () -> Void = { [weak self] in
            q.async {
                guard !released else { return }
                released = true
                if let self {
                    if self.activeConnections > 0 { self.activeConnections -= 1 }
                    self.subscribers.removeValue(forKey: ObjectIdentifier(conn))
                }
                conn.cancel()
            }
        }
        let remote: String
        if case .hostPort(let host, _) = conn.endpoint { remote = AppTailnet.plain("\(host)") } else { remote = "" }
        let label = CoderTailnet.label(owner)
        var answered = false
        let answer: (AppTailnet.Answer) -> Void = { a in
            guard !answered else { return }
            answered = true
            conn.send(content: AppTailnet.answerLine(a), completion: .contentProcessed { _ in closeConn() })
        }
        let refuse: (AppTailnet.Refusal) -> Void = { answer(.init(host: key, label: label, invitation: nil, refused: $0.rawValue)) }
        conn.start(queue: queue)
        guard AppTailnet.isTailnet(remote) else { refuse(.notTailnet); return }
        // one line within ten seconds (NIP-HOST)
        queue.asyncAfter(deadline: .now() + 10) { if !answered { answered = true; closeConn() } }
        CoderTailnet.readFrame(conn, Data(), maxBytes: AppTailnet.requestMax) { [weak self] line, rest in
            guard let self, !answered else { return }
            guard let line else { refuse(.malformed); return }
            let grantReq = AppTailnet.parseDirectGrantRequest(line)
            let isDirect = grantReq == nil && AppTailnet.isDirectHello(line)
            if grantReq == nil && !isDirect && !AppTailnet.isRequest(line) { refuse(.malformed); return }
            answered = true
            // Offload tailscale whois, ensureDaemon, direct-grant, and pairing subprocesses off the serial listener queue.
            self.workQueue.async { [weak self] in
                guard let self else { closeConn(); return }
                let whois = CoderTailnet.cli.flatMap { CoderTailnet.output($0, ["whois", "--json", remote]) }
                if let why = AppTailnet.decide(remote: remote, whois: whois, owner: owner) {
                    if grantReq != nil {
                        let ans = AppTailnet.DirectGrantAnswer(host: "", label: label, grant: nil, epoch: 0, expires: 0, refused: why.rawValue)
                        conn.send(content: AppTailnet.directGrantAnswerLine(ans), completion: .contentProcessed { _ in closeConn() })
                    } else if isDirect {
                        conn.send(content: AppTailnet.directAckLine(refused: why.rawValue), completion: .contentProcessed { _ in closeConn() })
                    } else {
                        conn.send(content: AppTailnet.answerLine(.init(host: key, label: label, invitation: nil, refused: why.rawValue)), completion: .contentProcessed { _ in closeConn() })
                    }
                    return
                }
                if let grantReq {
                    let reply = self.mintDirectGrant(request: grantReq, label: label)
                    conn.send(content: reply, completion: .contentProcessed { _ in closeConn() })
                    return
                }
                if isDirect {
                    guard CoderDaemonClient().ensureDaemon() else {
                        conn.send(content: AppTailnet.directAckLine(refused: AppTailnet.Refusal.unavailable.rawValue), completion: .contentProcessed { _ in closeConn() })
                        return
                    }
                    conn.send(content: AppTailnet.directAckLine(refused: nil), completion: .contentProcessed { [weak self] err in
                        if err != nil { closeConn(); return }
                        self?.serveDirectLoop(conn, buffer: rest, close: closeConn)
                    })
                    return
                }
                self.queue.async { [weak self] in
                    guard let self else { closeConn(); return }
                    guard let link = self.invitation(label: label) else {
                        conn.send(content: AppTailnet.answerLine(.init(host: key, label: label, invitation: nil, refused: AppTailnet.Refusal.unavailable.rawValue)), completion: .contentProcessed { _ in closeConn() })
                        return
                    }
                    conn.send(content: AppTailnet.answerLine(.init(host: key, label: label, invitation: link + AppTailnet.linkMark, refused: nil)), completion: .contentProcessed { _ in closeConn() })
                }
            }
        }
    }

    /// Mints a relay-free signed grant via `coder-host direct-grant` for an admitted same-owner Tailscale device (#13993).
    private func mintDirectGrant(request: AppTailnet.DirectGrantRequest, label: String) -> Data {
        let unavailable = AppTailnet.directGrantAnswerLine(.init(host: "", label: label, grant: nil, epoch: 0, expires: 0, refused: AppTailnet.Refusal.unavailable.rawValue))
        guard let host = Bundle.main.path(forResource: "coder-host", ofType: nil) else { return unavailable }
        _ = CoderDaemonClient().ensureDaemon()
        var isDir: ObjCBool = false
        let kept = UserDefaults.standard.string(forKey: "folder") ?? ""
        let folder = !kept.isEmpty && FileManager.default.fileExists(atPath: kept, isDirectory: &isDir) && isDir.boolValue
            ? kept : FileManager.default.homeDirectoryForCurrentUser.path
        let safeClientLabel = CoderTailnet.clampUTF8(request.label, maxBytes: 80)
        guard var out = CoderTailnet.output(host, [
            "direct-grant",
            "--device", request.device,
            "--workspace", folder,
            "--label", label,
            "--client-label", safeClientLabel.isEmpty ? "OpenAgents iOS" : safeClientLabel,
        ], timeout: 10), !out.isEmpty else {
            return unavailable
        }
        if let nl = out.firstIndex(of: 0x0A) { out = Data(out[out.startIndex..<nl]) }
        guard AppTailnet.directGrantAnswer(out) != nil else { return unavailable }
        out.append(0x0A)
        return out
    }

    private static func clampUTF8(_ s: String, maxBytes: Int) -> String {
        var out = ""
        for ch in s {
            let piece = String(ch)
            if out.utf8.count + piece.utf8.count > maxBytes { break }
            out.append(ch)
        }
        return out.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// Serves multiplexed NIP-SESS NDJSON RPCs and live push subscriptions over an admitted direct tailnet TCP connection (#13795, #13993).
    private func serveDirectLoop(_ conn: NWConnection, buffer: Data, close: @escaping () -> Void) {
        CoderTailnet.readFrame(conn, buffer, maxBytes: 262_144) { [weak self] line, rest in
            guard let self, let line else { close(); return }
            if let sessionFilter = AppTailnet.parseSubscribe(line) {
                self.queue.async { [weak self] in
                    self?.subscribers[ObjectIdentifier(conn)] = (conn, sessionFilter)
                }
                self.serveDirectLoop(conn, buffer: rest, close: close)
                return
            }
            // Keep reading subsequent pipelined frames while forwarding this NIP-44 RPC concurrently.
            self.serveDirectLoop(conn, buffer: rest, close: close)
            self.workQueue.async { [weak self] in
                guard let self else { return }
                let client = CoderDaemonClient()
                if var reply = client.forwardDirect(line) {
                    reply.append(0x0A)
                    self.queue.async {
                        conn.send(content: reply, completion: .contentProcessed { err in
                            if err != nil { close() }
                        })
                    }
                }
            }
        }
    }

    static func readLine(_ conn: NWConnection, _ have: Data, maxBytes: Int = AppTailnet.requestMax, _ done: @escaping (Data?) -> Void) {
        readFrame(conn, have, maxBytes: maxBytes) { line, _ in done(line) }
    }

    static func readFrame(_ conn: NWConnection, _ have: Data, maxBytes: Int, _ done: @escaping (Data?, Data) -> Void) {
        if let nl = have.firstIndex(of: 0x0A) {
            let line = Data(have[have.startIndex..<nl])
            let rest = Data(have[have.index(after: nl)...])
            done(line, rest)
            return
        }
        if have.count > maxBytes { done(nil, Data()); return }
        conn.receive(minimumIncompleteLength: 1, maximumLength: min(maxBytes + 1, 65536)) { data, _, complete, error in
            var got = have
            if let data { got.append(data) }
            if let nl = got.firstIndex(of: 0x0A) {
                let line = Data(got[got.startIndex..<nl])
                let rest = Data(got[got.index(after: nl)...])
                done(line, rest)
                return
            }
            if got.count > maxBytes || error != nil || complete { done(nil, Data()); return }
            readFrame(conn, got, maxBytes: maxBytes, done)
        }
    }

    /// A fresh pairing link from `coder-host pair`, which then waits (five minutes at most) for the
    /// phone to ask with its code and allows it. Runs on `queue`.
    private func invitation(label: String) -> String? {
        guard pending < 2, let host = Bundle.main.path(forResource: "coder-host", ofType: nil) else { return nil }
        // the daemon records the phone's request from the relay; start it if none answers
        guard CoderDaemonClient().ensureDaemon() else { return nil }
        var isDir: ObjCBool = false
        let kept = UserDefaults.standard.string(forKey: "folder") ?? ""
        let folder = !kept.isEmpty && FileManager.default.fileExists(atPath: kept, isDirectory: &isDir) && isDir.boolValue
            ? kept : FileManager.default.homeDirectoryForCurrentUser.path
        let p = Process(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: host)
        p.arguments = ["pair", "--workspace", folder, "--label", label, "--wait", "300"]
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = CoderWorker.path
        env["BROWSER"] = "/usr/bin/true"
        env["TAILSCALE_BE_CLI"] = "1"
        if env["TERM"]?.isEmpty != false { env["TERM"] = "dumb" }
        p.environment = env
        p.standardOutput = out; p.standardError = FileHandle.nullDevice; p.standardInput = FileHandle.nullDevice
        let got = DispatchSemaphore(value: 0)
        let lock = NSLock()
        var text = "", link: String?
        out.fileHandleForReading.readabilityHandler = { h in
            let chunk = h.availableData
            if chunk.isEmpty { h.readabilityHandler = nil; got.signal(); return }
            lock.lock(); defer { lock.unlock() }
            guard link == nil else { return }
            text += String(decoding: chunk, as: UTF8.self)
            if let line = text.split(whereSeparator: \.isNewline).map(String.init).first(where: { $0.hasPrefix(AppTailnet.linkPrefix) }) {
                link = line; got.signal()
            }
        }
        p.terminationHandler = { [weak self] _ in self?.queue.async { self?.pending -= 1 } }
        do { try p.run() } catch { out.fileHandleForReading.readabilityHandler = nil; return nil }
        pending += 1
        _ = got.wait(timeout: .now() + 8)
        lock.lock(); let found = link; lock.unlock()
        if found == nil, p.isRunning { p.terminate() }
        return found
    }
}
