// Firm reason: 6 (Swift-to-C host wrapper over coax/lib/agent/desktop/worker.coax and store.coax; GitHub #13794).
import Foundation

/// Talks to the agent worker via coax/lib/agent/desktop/worker.coax when the daemon owns runs,
/// or via a direct child process fallback when no daemon is running.
final class CoderWorker {
    var line: ((String) -> Void)?      // a line it wrote, on the main queue
    var gone: (() -> Void)?            // it exited, on the main queue
    private var process: Process?
    private var input: FileHandle?
    private var held = Data()

    /// The daemon that owns worker runs when one is running (GitHub #13694).
    static var daemon: CoderDaemonClient?
    /// The most workers Coder starts at once; also enforced by the daemon.
    static let runLimit = 12
    static let runLimitNote = "Coder runs up to \(runLimit) workers at once, sub-agents included. Stop a chat or wait for one to finish, then send again."
    /// Why the last start() failed, when the daemon refused it.
    private(set) var failure: String?

    private(set) var runID: String?
    private var tail: DispatchSourceTimer?
    private let tailQueue = DispatchQueue(label: "openagents.coder.worker-tail", qos: .userInitiated)
    private var tailHeld = Data()
    private(set) var offset: UInt64 = 0
    private var touched = Date.distantPast

    var alive: Bool { process?.isRunning == true || runID != nil }
    var isDaemonRun: Bool { runID != nil }
    var program = "mcworker"
    var extraEnv: [String: String] = [:]
    var folder = ""

    static var path: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let had = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        return DesktopCore.withCoaxString(home) { ch in
            DesktopCore.withCoaxString(had) { cp in
                DesktopCore.callString(8192) { oa_dc_worker_path(ch, cp, $0) }
            }
        }
    }
    static var hasClaude: Bool { path.split(separator: ":").contains { FileManager.default.isExecutableFile(atPath: $0 + "/claude") } }

    private func extraEnvLines() -> String {
        extraEnv.map { "\($0.key)=\($0.value)" }.joined(separator: "\n")
    }

    private func environment() -> (set: [String: String], unset: [String]) {
        var env: [String: String] = ["PATH": CoderWorker.path]
        for (k, v) in extraEnv { env[k] = v }
        for (k, v) in ChatWorktree.caches(for: folder) where env[k] == nil && ProcessInfo.processInfo.environment[k] == nil { env[k] = v }
        if program == "claudeworker", let key = CoderKey.current { env["CODER_CLAUDE_KEY"] = key; return (env, []) }
        return (env, ["CODER_CLAUDE_KEY"])
    }

    func start() -> Bool {
        guard !alive else { return true }
        failure = nil
        guard let path = Bundle.main.path(forResource: program, ofType: nil) else { return false }
        if let daemon = CoderWorker.daemon, daemon.ownsRuns { return startRun(path, daemon) }
        let p = Process(), into = Pipe(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.currentDirectoryURL = FileManager.default.homeDirectoryForCurrentUser
        p.standardInput = into
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        var env = ProcessInfo.processInfo.environment
        let (set, unset) = environment()
        for (k, v) in set { env[k] = v }
        for k in unset { env.removeValue(forKey: k) }
        p.environment = env
        out.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let bytes = handle.availableData
            if bytes.isEmpty { handle.readabilityHandler = nil; return }
            DispatchQueue.main.async { self?.read(bytes) }
        }
        p.terminationHandler = { [weak self] ended in
            DispatchQueue.main.async { guard let self, self.process === ended else { return }; self.process = nil; self.input = nil; self.gone?() }
        }
        do { try p.run() } catch { return false }
        process = p; input = into.fileHandleForWriting; held = Data()
        return true
    }

    private func startRun(_ path: String, _ daemon: CoderDaemonClient) -> Bool {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let hadPath = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        let claudeKey = CoderKey.current ?? ""
        let extra = extraEnvLines()
        var idBuf = [UInt8](repeating: 0, count: 128)
        var failBuf = [UInt8](repeating: 0, count: 512)
        let ok = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(path) { cpath in
                DesktopCore.withCoaxString(program) { cprog in
                    DesktopCore.withCoaxString(folder) { cfolder in
                        DesktopCore.withCoaxString(extra) { cextra in
                            DesktopCore.withCoaxString(home) { chome in
                                DesktopCore.withCoaxString(hadPath) { chad in
                                    DesktopCore.withCoaxString(claudeKey) { ckey in
                                        idBuf.withUnsafeMutableBufferPointer { ib in
                                            failBuf.withUnsafeMutableBufferPointer { fb in
                                                oa_dc_worker_start_run(
                                                    cdir, cpath, cprog, cfolder, cextra, chome, chad, ckey,
                                                    coax_v_u8(data: ib.baseAddress, count: Int64(ib.count)),
                                                    coax_v_u8(data: fb.baseAddress, count: Int64(fb.count))
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let why = DesktopCore.cString(from: failBuf)
        let id = DesktopCore.cString(from: idBuf)
        guard ok == 1, !id.isEmpty else {
            failure = why == "run_limit" ? Self.runLimitNote : (why.isEmpty ? "failed" : why)
            return false
        }
        attach(id, from: 0)
        follow()
        return true
    }

    func startAsync(_ completion: @escaping (Bool) -> Void) {
        guard !alive else { completion(true); return }
        failure = nil
        guard let path = Bundle.main.path(forResource: program, ofType: nil) else { completion(false); return }
        if let daemon = CoderWorker.daemon, daemon.ownsRuns {
            startRunAsync(path, daemon, completion: completion)
            return
        }
        completion(start())
    }

    private func startRunAsync(_ path: String, _ daemon: CoderDaemonClient, completion: @escaping (Bool) -> Void) {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let hadPath = ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        let claudeKey = CoderKey.current ?? ""
        let extra = extraEnvLines()
        let prog = program
        let fld = folder
        let dir = daemon.hostDirPath
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            var idBuf = [UInt8](repeating: 0, count: 128)
            var failBuf = [UInt8](repeating: 0, count: 512)
            let ok = DesktopCore.withCoaxString(dir) { cdir in
                DesktopCore.withCoaxString(path) { cpath in
                    DesktopCore.withCoaxString(prog) { cprog in
                        DesktopCore.withCoaxString(fld) { cfolder in
                            DesktopCore.withCoaxString(extra) { cextra in
                                DesktopCore.withCoaxString(home) { chome in
                                    DesktopCore.withCoaxString(hadPath) { chad in
                                        DesktopCore.withCoaxString(claudeKey) { ckey in
                                            idBuf.withUnsafeMutableBufferPointer { ib in
                                                failBuf.withUnsafeMutableBufferPointer { fb in
                                                    oa_dc_worker_start_run(
                                                        cdir, cpath, cprog, cfolder, cextra, chome, chad, ckey,
                                                        coax_v_u8(data: ib.baseAddress, count: Int64(ib.count)),
                                                        coax_v_u8(data: fb.baseAddress, count: Int64(fb.count))
                                                    )
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            let why = DesktopCore.cString(from: failBuf)
            let id = DesktopCore.cString(from: idBuf)
            DispatchQueue.main.async {
                guard let self else {
                    if ok == 1, !id.isEmpty { daemon.stopRun(id) { daemon.forgetRun(id) } }
                    completion(false)
                    return
                }
                guard ok == 1, !id.isEmpty else {
                    self.failure = why == "run_limit" ? Self.runLimitNote : (why.isEmpty ? "failed" : why)
                    completion(false)
                    return
                }
                self.attach(id, from: 0)
                self.follow()
                completion(true)
            }
        }
    }

    func attach(_ id: String, from: UInt64) {
        guard CoderWorker.daemon != nil else { return }
        runID = id; offset = from; tailHeld = Data()
        touch(force: true)
    }

    func readAvailable() -> [String] {
        guard let id = runID, let daemon = CoderWorker.daemon else { return [] }
        let outPath = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(".out") { cext in
                    DesktopCore.callString(4096) { oa_dc_run_file(cdir, cid, cext, $0) }
                }
            }
        }
        guard !outPath.isEmpty else { return [] }
        return tailQueue.sync { readLines(URL(fileURLWithPath: outPath)) }
    }

    private func readLines(_ url: URL) -> [String] {
        guard let h = try? FileHandle(forReadingFrom: url) else { return [] }
        defer { try? h.close() }
        guard (try? h.seek(toOffset: offset)) != nil, let bytes = try? h.readToEnd(), !bytes.isEmpty else { return [] }
        offset += UInt64(bytes.count)
        tailHeld.append(bytes)
        var lines: [String] = []
        while let end = tailHeld.firstIndex(of: 0x0a) {
            let one = tailHeld.subdata(in: tailHeld.startIndex..<end)
            tailHeld.removeSubrange(tailHeld.startIndex...end)
            if let text = String(data: one, encoding: .utf8), !text.isEmpty { lines.append(text) }
        }
        return lines
    }

    static func runState(_ id: String) -> String? {
        guard let daemon else { return nil }
        let s = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.callString(128) { oa_dc_run_state(cdir, cid, $0) }
            }
        }
        return s.isEmpty ? nil : s
    }

    private func touch(force: Bool = false) {
        guard let id = runID, let daemon = CoderWorker.daemon, force || Date().timeIntervalSince(touched) > 1 else { return }
        touched = Date()
        DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                _ = oa_dc_worker_touch_run(cdir, cid, force ? 1 : 0)
            }
        }
    }

    func follow() {
        guard let id = runID, let daemon = CoderWorker.daemon, tail == nil else { return }
        let outPath = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(".out") { cext in
                    DesktopCore.callString(4096) { oa_dc_run_file(cdir, cid, cext, $0) }
                }
            }
        }
        guard !outPath.isEmpty else { return }
        let out = URL(fileURLWithPath: outPath)
        let timer = DispatchSource.makeTimerSource(queue: tailQueue)
        var quiet = 0
        timer.schedule(deadline: .now(), repeating: .milliseconds(30))
        timer.setEventHandler { [weak self] in
            guard let self else { return }
            let lines = self.readLines(out)
            if !lines.isEmpty {
                quiet = 0
                DispatchQueue.main.async { [weak self] in guard let self, self.runID == id else { return }; for l in lines { self.line?(l) } }
                return
            }
            quiet += 1
            if quiet % 10 != 0 { return }
            DispatchQueue.main.async { [weak self] in self?.touch() }
            if let state = CoderWorker.runState(id), state != "running", state != "stopping" {
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.runID == id else { return }
                    self.tail?.cancel(); self.tail = nil
                    self.runID = nil
                    self.gone?()
                }
            }
        }
        tail = timer
        timer.resume()
    }

    func detach() {
        tail?.cancel(); tail = nil
        if let id = runID, let daemon = CoderWorker.daemon {
            DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
                DesktopCore.withCoaxString(id) { cid in
                    _ = oa_dc_worker_detach_run(cdir, cid)
                }
            }
        }
        runID = nil
    }

    private func read(_ bytes: Data) {
        held.append(bytes)
        while let end = held.firstIndex(of: 0x0a) {
            let one = held.subdata(in: held.startIndex..<end)
            held.removeSubrange(held.startIndex...end)
            if let text = String(data: one, encoding: .utf8), !text.isEmpty { line?(text) }
        }
    }

    func write(_ bytes: [CChar], count: Int) {
        guard alive, count > 0 else { return }
        let data = bytes.prefix(count).withUnsafeBytes { Data($0) }
        if let id = runID, let daemon = CoderWorker.daemon {
            let text = String(decoding: data, as: UTF8.self)
            DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
                DesktopCore.withCoaxString(id) { cid in
                    DesktopCore.withCoaxString(text) { ct in
                        _ = oa_dc_worker_write_run(cdir, cid, ct)
                    }
                }
            }
            return
        }
        guard let input else { return }
        try? input.write(contentsOf: data)
    }

    func stop(forget: Bool = true) {
        if let id = runID, let daemon = CoderWorker.daemon {
            tail?.cancel(); tail = nil
            runID = nil
            DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
                DesktopCore.withCoaxString(id) { cid in
                    _ = oa_dc_worker_stop_run(cdir, cid, forget ? 1 : 0)
                }
            }
            return
        }
        let p = process, i = input
        process = nil; input = nil
        try? i?.close()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { if p?.isRunning == true { p?.terminate() } }
    }
}

enum CoderKey {
    static var file: URL {
        let p = DesktopCore.withCoaxString("claude-key") { cn in
            DesktopCore.callString(4096) { oa_dc_coder_file(cn, $0) }
        }
        return URL(fileURLWithPath: p)
    }
    static var fromLaunch: String? {
        let key = (ProcessInfo.processInfo.environment["CODER_CLAUDE_KEY"] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return key.isEmpty ? nil : key
    }
    static var saved: String? {
        let s = DesktopCore.withCoaxString("claude-key") { cn in
            DesktopCore.callString(4096) { oa_dc_key_read(cn, $0) }
        }
        return s.isEmpty ? nil : s
    }
    static var current: String? {
        let s = DesktopCore.callString(4096) { oa_dc_claude_key($0) }
        return s.isEmpty ? nil : s
    }
    static func save(_ key: String) -> Bool {
        DesktopCore.withCoaxString("claude-key") { cn in
            DesktopCore.withCoaxString(key) { ck in
                oa_dc_key_save(cn, ck) == 1
            }
        }
    }
    static func tail(_ key: String) -> String {
        DesktopCore.withCoaxString(key) { ck in
            DesktopCore.callString(64) { oa_dc_key_tail(ck, $0) }
        }
    }
}

enum CoderOpenRouter {
    static var main: String { UserDefaults.standard.string(forKey: "openRouterMain") ?? "" }
    static var sub: String { UserDefaults.standard.string(forKey: "openRouterSub") ?? "" }
    static func setModels(main: String, sub: String) {
        UserDefaults.standard.set(main, forKey: "openRouterMain"); UserDefaults.standard.set(sub, forKey: "openRouterSub")
    }
    static var file: URL {
        let p = DesktopCore.withCoaxString("openrouter-key") { cn in
            DesktopCore.callString(4096) { oa_dc_coder_file(cn, $0) }
        }
        return URL(fileURLWithPath: p)
    }
    static var saved: String? {
        let s = DesktopCore.withCoaxString("openrouter-key") { cn in
            DesktopCore.callString(4096) { oa_dc_key_read(cn, $0) }
        }
        return s.isEmpty ? nil : s
    }
    static var hasKey: Bool {
        saved != nil || FileManager.default.fileExists(atPath: FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("work/.secrets/openrouter.env").path)
    }
    static func save(_ key: String) -> Bool {
        DesktopCore.withCoaxString("openrouter-key") { cn in
            DesktopCore.withCoaxString(key) { ck in
                oa_dc_key_save(cn, ck) == 1
            }
        }
    }
    static func unfit(_ ids: [String], done: @escaping ([String]?) -> Void) {
        let wanted = ids.filter { !$0.isEmpty }
        guard !wanted.isEmpty, let url = URL(string: "https://openrouter.ai/api/v1/models") else { done([]); return }
        URLSession.shared.dataTask(with: url) { data, _, _ in
            var fit = Set<String>()
            guard let data, let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let models = root["data"] as? [[String: Any]] else {
                DispatchQueue.main.async { done(nil) }; return
            }
            for m in models {
                if let id = m["id"] as? String, let params = m["supported_parameters"] as? [String], params.contains("structured_outputs") { fit.insert(id) }
            }
            DispatchQueue.main.async { done(wanted.filter { !fit.contains($0) }) }
        }.resume()
    }
}

func coderApplyOpenRouter(_ page: UnsafeMutableRawPointer?) { cd_openrouter(page, CoderOpenRouter.main, CoderOpenRouter.sub) }

func coderSkillsNote() -> String {
    let root = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/skills")
    let names = ((try? FileManager.default.contentsOfDirectory(atPath: root.path)) ?? []).sorted()
    var rows: [String] = []
    for name in names where rows.count < 16 {
        let file = root.appendingPathComponent(name).appendingPathComponent("SKILL.md")
        guard let text = try? String(contentsOf: file, encoding: .utf8), text.hasPrefix("---") else { continue }
        var description = ""
        for line in text.split(separator: "\n").dropFirst() {
            if line.hasPrefix("---") { break }
            if line.hasPrefix("description:") { description = String(line.dropFirst(12)).trimmingCharacters(in: .whitespaces) }
        }
        rows.append("- \(name) (\(file.path)): \(description.prefix(300))")
    }
    if rows.isEmpty { return "" }
    return "Skills installed on this computer. When a message is about one, read its file with cat first, then do what it says:\n" + rows.joined(separator: "\n") + "\n\nThe message:\n"
}
