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
    private var timer: DispatchSourceTimer?
    private var listener: NWListener?
    private var service: NetService?            // main thread only
    private var serving: (owner: AppTailnet.Owner, key: String, port: UInt16)?
    private var hostKey: String?
    private var pending = 0                     // pair processes waiting for a phone, at most two

    /// Starts watching for Tailscale (now, then every two minutes). Not in a scratch test launch.
    func start() {
        guard CoderTailnet.enabled, !TestLaunch.active else { return }
        queue.async { [self] in
            guard timer == nil else { return }
            let t = DispatchSource.makeTimerSource(queue: queue)
            t.schedule(deadline: .now() + 2, repeating: .seconds(120), leeway: .seconds(10))
            t.setEventHandler { [weak self] in self?.check() }
            t.resume(); timer = t
        }
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
        var env = ProcessInfo.processInfo.environment; env["BROWSER"] = "/usr/bin/true"; p.environment = env
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
        l.newConnectionLimit = 4
        l.newConnectionHandler = { [weak self] conn in self?.serve(conn, owner: owner, key: key) }
        l.stateUpdateHandler = { [weak self, weak l] state in
            guard let self, let l else { return }
            switch state {
            case .ready:
                let got = l.port?.rawValue ?? port
                self.serving = (owner, key, got)
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
        unadvertise()
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
        let remote: String
        if case .hostPort(let host, _) = conn.endpoint { remote = AppTailnet.plain("\(host)") } else { remote = "" }
        let label = CoderTailnet.label(owner)
        var answered = false
        let answer: (AppTailnet.Answer) -> Void = { a in
            guard !answered else { return }
            answered = true
            conn.send(content: AppTailnet.answerLine(a), completion: .contentProcessed { _ in conn.cancel() })
        }
        let refuse: (AppTailnet.Refusal) -> Void = { answer(.init(host: key, label: label, invitation: nil, refused: $0.rawValue)) }
        conn.start(queue: queue)
        guard AppTailnet.isTailnet(remote) else { refuse(.notTailnet); return }
        // one line within ten seconds (NIP-HOST)
        queue.asyncAfter(deadline: .now() + 10) { if !answered { answered = true; conn.cancel() } }
        CoderTailnet.readLine(conn, Data(), maxBytes: AppTailnet.requestMax) { [weak self] line in
            guard let self, !answered else { return }
            guard let line else { refuse(.malformed); return }
            let isDirect = AppTailnet.isDirectHello(line)
            if !isDirect && !AppTailnet.isRequest(line) { refuse(.malformed); return }
            let whois = CoderTailnet.cli.flatMap { CoderTailnet.output($0, ["whois", "--json", remote]) }
            if let why = AppTailnet.decide(remote: remote, whois: whois, owner: owner) {
                if isDirect {
                    answered = true
                    conn.send(content: AppTailnet.directAckLine(refused: why.rawValue), completion: .contentProcessed { _ in conn.cancel() })
                } else {
                    refuse(why)
                }
                return
            }
            if isDirect {
                answered = true
                guard CoderDaemonClient().ensureDaemon() else {
                    conn.send(content: AppTailnet.directAckLine(refused: AppTailnet.Refusal.unavailable.rawValue), completion: .contentProcessed { _ in conn.cancel() })
                    return
                }
                conn.send(content: AppTailnet.directAckLine(refused: nil), completion: .contentProcessed { [weak self] err in
                    if err != nil { conn.cancel(); return }
                    self?.serveDirectLoop(conn, buffer: Data())
                })
                return
            }
            guard let link = self.invitation(label: label) else { refuse(.unavailable); return }
            answer(.init(host: key, label: label, invitation: link + AppTailnet.linkMark, refused: nil))
        }
    }

    /// Serves sealed NIP-SESS NDJSON request events over an admitted direct tailnet TCP connection (#13795).
    private func serveDirectLoop(_ conn: NWConnection, buffer: Data) {
        CoderTailnet.readFrame(conn, buffer, maxBytes: 65536) { [weak self] line, rest in
            guard let self, let line else { conn.cancel(); return }
            let client = CoderDaemonClient()
            if var reply = client.forwardDirect(line) {
                reply.append(0x0A)
                conn.send(content: reply, completion: .contentProcessed { [weak self] err in
                    if err != nil { conn.cancel(); return }
                    self?.serveDirectLoop(conn, buffer: rest)
                })
            } else {
                // Expired or silent requests produce no reply; keep reading.
                self.serveDirectLoop(conn, buffer: rest)
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
        var env = ProcessInfo.processInfo.environment; env["PATH"] = CoderWorker.path; env["BROWSER"] = "/usr/bin/true"; p.environment = env
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
