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
    /// Last terminal state observed when the worker exited (e.g. "lost", "exited", "exit-15").
    private(set) var lastEndState: String?
    /// Last process exit code (or 128 + signal on uncaught signal) observed when the worker exited.
    private(set) var lastExitCode: Int32?
    /// Last sanitized stderr line captured from the exited worker (e.g. coax trap location).
    private(set) var lastEndDetail: String?

    private(set) var runID: String?
    private var tail: DispatchSourceTimer?
    private let tailQueue = DispatchQueue(label: "openagents.coder.worker-tail", qos: .userInitiated)
    private var tailHeld = Data()
    private(set) var offset: UInt64 = 0
    private var touched = Date.distantPast

    var alive: Bool { process?.isRunning == true || runID != nil }
    var isDaemonRun: Bool { runID != nil }
    var program = "oa-worker"
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
    static var hasClaude: Bool {
        let fm = FileManager.default
        if path.split(separator: ":").contains(where: { fm.isExecutableFile(atPath: $0 + "/claude") }) {
            return true
        }
        let localFallback = fm.homeDirectoryForCurrentUser.path + "/.claude/local/claude"
        return fm.isExecutableFile(atPath: localFallback)
    }
    static var hasAntigravity: Bool { CoderAntigravity.resolvedBinaryPath() != nil }

    private var defaultAgentID: String {
        switch program {
        case "oa-worker", "flawedworker": return "openagents"
        case "agyworker": return "antigravity"
        case "inky": return "inky"
        case "claudeworker": return "claude"
        default: return "microcoder"
        }
    }

    private func extraEnvLines() -> String {
        var merged = extraEnv
        if merged["OA_AGENT_ID"] == nil { merged["OA_AGENT_ID"] = defaultAgentID }
        if merged["OA_DESKTOP_VERSION"] == nil { merged["OA_DESKTOP_VERSION"] = BundleMetadata.currentUpdateIdentifier }
        return merged.map { "\($0.key)=\($0.value)" }.joined(separator: "\n")
    }

    private func environment() -> (set: [String: String], unset: [String]) {
        var env: [String: String] = [
            "PATH": CoderWorker.path,
            "OA_AGENT_ID": defaultAgentID,
            "OA_DESKTOP_VERSION": BundleMetadata.currentUpdateIdentifier,
        ]
        for (k, v) in extraEnv { env[k] = v }
        // oasim (the iOS Simulator's command, #13811) is in Contents/Resources: on the PATH after the person's own
        if let res = Bundle.main.resourcePath, FileManager.default.isExecutableFile(atPath: res + "/oasim"), let p = env["PATH"], !p.split(separator: ":").contains(Substring(res)) { env["PATH"] = p + ":" + res }
        for (k, v) in ChatWorktree.caches(for: folder) where env[k] == nil && ProcessInfo.processInfo.environment[k] == nil { env[k] = v }
        if program == "claudeworker", let key = CoderKey.current { env["CODER_CLAUDE_KEY"] = key; return (env, []) }
        return (env, ["CODER_CLAUDE_KEY"])
    }

    /// Atomically materializes one bundled executable into the immutable versioned daemon bin folder.
    /// Compares size + modificationDate (falling back to byte-for-byte equality when timestamps differ)
    /// so same-size 16KB page-aligned arm64 Mach-O rebuilds never serve a stale crashing binary.
    private func materializeForDaemon(_ binName: String) -> String? {
        guard let bundled = Bundle.main.path(forResource: binName, ofType: nil) else { return nil }
        let fm = FileManager.default
        let verID = BundleMetadata.currentUpdateIdentifier
        let dir = fm.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/environment/bin/\(verID)", isDirectory: true)
        let dst = dir.appendingPathComponent(binName)
        let srcAttrs = try? fm.attributesOfItem(atPath: bundled)
        let srcSize = (srcAttrs?[.size] as? NSNumber)?.int64Value
        let srcMod = srcAttrs?[.modificationDate] as? Date
        if fm.isExecutableFile(atPath: dst.path) {
            let dstAttrs = try? fm.attributesOfItem(atPath: dst.path)
            let dstSize = (dstAttrs?[.size] as? NSNumber)?.int64Value
            let dstMod = dstAttrs?[.modificationDate] as? Date
            if let srcSize, let dstSize, srcSize == dstSize {
                let sameMod = (srcMod != nil && dstMod != nil && srcMod == dstMod)
                if sameMod || fm.contentsEqual(atPath: bundled, andPath: dst.path) {
                    return dst.path
                }
            }
        }
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
        let tmp = dir.appendingPathComponent(".\(binName).new.\(getpid()).\(UUID().uuidString.prefix(6))")
        try? fm.removeItem(at: tmp)
        do {
            try fm.copyItem(at: URL(fileURLWithPath: bundled), to: tmp)
            var attrs: [FileAttributeKey: Any] = [.posixPermissions: 0o755]
            if let srcMod { attrs[.modificationDate] = srcMod }
            try fm.setAttributes(attrs, ofItemAtPath: tmp.path)
            if rename(tmp.path, dst.path) == 0 {
                return dst.path
            }
            try? fm.removeItem(at: tmp)
        } catch {
            try? fm.removeItem(at: tmp)
        }
        return bundled
    }

    /// Resolves the executable path for `program`. For daemon-owned runs, prefers an immutable
    /// versioned copy under `~/.openagents/environment/bin/<versionID>/<program>` (materializing
    /// atomically from `Bundle.main` if needed) so replacing `/Applications/OpenAgents.app` never
    /// pulls binaries out from under active workers (#14095). For `claudeworker`, also materializes
    /// sibling `gitguard` beside it so its PreToolUse Bash guard resolves cleanly (#14104).
    private func resolvedProgramPath(forDaemon: Bool) -> String? {
        guard let bundled = Bundle.main.path(forResource: program, ofType: nil) else { return nil }
        guard forDaemon else { return bundled }
        if program == "claudeworker" {
            _ = materializeForDaemon("gitguard")
        }
        return materializeForDaemon(program) ?? bundled
    }

    func start() -> Bool {
        guard !alive else { return true }
        failure = nil
        lastEndState = nil
        lastExitCode = nil
        lastEndDetail = nil
        guard let bundledPath = resolvedProgramPath(forDaemon: false) else { failure = "This build of OpenAgents is missing its \(program)."; return false }
        if let daemon = CoderWorker.daemon, daemon.ownsRuns {
            let daemonPath = resolvedProgramPath(forDaemon: true) ?? bundledPath
            if startRun(daemonPath, daemon) { return true }
            if failure == Self.runLimitNote { return false }
        }
        return startChild(bundledPath)
    }

    /// The worker as this app's own child: when no daemon owns runs, or the daemon refused a run
    /// for a reason other than the run limit (the send still goes through; the reason is logged).
    private func startChild(_ path: String) -> Bool {
        if let why = failure { NSLog("Coder: the background service did not start a worker (%@); starting it directly", why) }
        let p = Process(), into = Pipe(), out = Pipe(), errPipe = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.currentDirectoryURL = FileManager.default.homeDirectoryForCurrentUser
        p.standardInput = into
        p.standardOutput = out
        p.standardError = errPipe
        var env = ProcessInfo.processInfo.environment
        let (set, unset) = environment()
        for (k, v) in set { env[k] = v }
        for k in unset { env.removeValue(forKey: k) }
        p.environment = env
        let errLock = NSLock()
        var errTail = Data()
        errPipe.fileHandleForReading.readabilityHandler = { handle in
            let chunk = handle.availableData
            if chunk.isEmpty { handle.readabilityHandler = nil; return }
            errLock.lock()
            errTail.append(chunk)
            if errTail.count > 4096 { errTail.removeFirst(errTail.count - 4096) }
            errLock.unlock()
        }
        out.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let bytes = handle.availableData
            if bytes.isEmpty { handle.readabilityHandler = nil; return }
            DispatchQueue.main.async { self?.read(bytes) }
        }
        p.terminationHandler = { [weak self] ended in
            errPipe.fileHandleForReading.readabilityHandler = nil
            let remErr = (try? errPipe.fileHandleForReading.readToEnd()) ?? Data()
            errLock.lock()
            if !remErr.isEmpty {
                errTail.append(remErr)
                if errTail.count > 4096 { errTail.removeFirst(errTail.count - 4096) }
            }
            let detail = Self.sanitizeStderrDetail(String(decoding: errTail, as: UTF8.self))
            errLock.unlock()
            let code: Int32 = ended.terminationReason == .uncaughtSignal ? (128 + ended.terminationStatus) : ended.terminationStatus
            DispatchQueue.main.async {
                guard let self, self.process === ended else { return }
                self.lastExitCode = code
                self.lastEndDetail = detail
                self.lastEndState = "exit-\(code)"
                self.process = nil; self.input = nil
                self.gone?()
            }
        }
        do { try p.run() } catch { failure = failure ?? "The worker did not start: \(error.localizedDescription)"; return false }
        process = p; input = into.fileHandleForWriting; held = Data(); failure = nil
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
            failure = why.isEmpty ? "The background service didn't start the worker." : why
            return false
        }
        attach(id, from: 0)
        follow()
        return true
    }

    func startAsync(_ completion: @escaping (Bool) -> Void) {
        guard !alive else { completion(true); return }
        failure = nil
        lastEndState = nil
        lastExitCode = nil
        lastEndDetail = nil
        guard let bundledPath = resolvedProgramPath(forDaemon: false) else { failure = "This build of OpenAgents is missing its \(program)."; completion(false); return }
        if let daemon = CoderWorker.daemon, daemon.ownsRuns {
            let daemonPath = resolvedProgramPath(forDaemon: true) ?? bundledPath
            startRunAsync(daemonPath, daemon, completion: completion)
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
                    self.failure = why.isEmpty ? "The background service didn't start the worker." : why
                    completion(self.failure != Self.runLimitNote && self.startChild(path))
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
        lastEndState = nil
        lastExitCode = nil
        lastEndDetail = nil
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

    /// The run's worker has nothing to do: empty/startup output, its queue with no active item
    /// and no items, or its turn's ended line (as host-runs.coax dr_idle_line reads it).
    static func runIdle(_ id: String) -> Bool {
        guard let daemon else { return false }
        let path = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(".out") { cext in
                    DesktopCore.callString(4096) { oa_dc_run_file(cdir, cid, cext, $0) }
                }
            }
        }
        guard !path.isEmpty, let h = FileHandle(forReadingAtPath: path) else { return false }
        defer { try? h.close() }
        let size = (try? h.seekToEnd()) ?? 0
        if size == 0 { return true }
        // Try a 64 KiB tail first; if an ended line exceeds 64 KiB (up to MW_LINE = 1 MiB), widen once.
        for window: UInt64 in [65_536, 1_048_576] {
            let offset = size > window ? size - window : 0
            try? h.seek(toOffset: offset)
            guard let data = try? h.readToEnd(), let text = String(data: data, encoding: .utf8),
                  let last = text.split(separator: "\n").last.map(String.init) else { continue }
            if offset == 0 || last.hasPrefix("{\"kind\":") {
                return isIdleLine(last)
            }
        }
        return false
    }
    static func isIdleLine(_ line: String) -> Bool {
        guard line.hasPrefix("{\"kind\":"), let data = line.data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        let kind = o["kind"] as? String ?? ""
        if kind == "ended" || kind == "ready" || kind == "journal" || kind == "aware" { return true }
        guard kind == "queue" else { return false }
        return (o["active"] as? String ?? "x").isEmpty && ((o["items"] as? [Any])?.isEmpty ?? false)
    }
    /// True when a detached unnamed run's `.out` file has had zero writes for at least `seconds`.
    static func runOutStale(_ id: String, olderThan seconds: TimeInterval) -> Bool {
        guard let daemon, id.count == 32 else { return false }
        let outURL = daemon.runsDir.appendingPathComponent(id + ".out")
        let target = FileManager.default.fileExists(atPath: outURL.path) ? outURL : daemon.runsDir.appendingPathComponent(id + ".state")
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: target.path),
              let mtime = attrs[.modificationDate] as? Date else { return false }
        return Date().timeIntervalSince(mtime) >= seconds
    }
    /// True when `id` is a phone-originated daemon run (runs/ID.phone exists; host-phone.coax).
    /// Desktop's launch/sweep orphan cleanup must never stop or forget phone runs.
    static func isPhoneRun(_ id: String) -> Bool {
        guard let daemon, id.count == 32 else { return false }
        return FileManager.default.fileExists(atPath: daemon.runsDir.appendingPathComponent(id + ".phone").path)
    }
    /// Canonical lowercase UUID ("8-4-4-4-12") for a 32-hex phone run id, so CoderStore and
    /// host-desktop.coax share one chat identity with zero duplicates.
    static func phoneChatUUID(from runID: String) -> String? {
        guard runID.count == 32, runID.allSatisfy({ ($0 >= "0" && $0 <= "9") || ($0 >= "a" && $0 <= "f") }) else { return nil }
        let a = runID.prefix(8)
        let b = runID.dropFirst(8).prefix(4)
        let c = runID.dropFirst(12).prefix(4)
        let d = runID.dropFirst(16).prefix(4)
        let e = runID.dropFirst(20).prefix(12)
        return "\(a)-\(b)-\(c)-\(d)-\(e)"
    }
    /// Lowercase 32-hex run id for a canonical hyphenated UUID chat id, or nil.
    static func hexRunID(from chatID: String) -> String? {
        guard chatID.count == 36 else { return nil }
        let stripped = chatID.replacingOccurrences(of: "-", with: "").lowercased()
        guard stripped.count == 32, stripped.allSatisfy({ ($0 >= "0" && $0 <= "9") || ($0 >= "a" && $0 <= "f") }) else { return nil }
        return stripped
    }
    /// Every run Coder stops or leaves running when no chat holds it, one line each, so a lost reply
    /// can be traced (Application Support/Coder/run-stops.log).
    static func logStop(_ id: String, _ why: String) {
        let url = CoderTurn.dir.deletingLastPathComponent().appendingPathComponent("run-stops.log")
        let line = ISO8601DateFormatter().string(from: Date()) + " " + id + " " + why + "\n"
        if let h = try? FileHandle(forWritingTo: url) { h.seekToEndOfFile(); h.write(Data(line.utf8)); try? h.close() }
        else { try? Data(line.utf8).write(to: url) }
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

    static func signalName(forExitCode code: Int32) -> String? {
        guard code > 128 else { return nil }
        switch code - 128 {
        case 4: return "SIGILL"
        case 5: return "SIGTRAP"
        case 6: return "SIGABRT"
        case 8: return "SIGFPE"
        case 9: return "SIGKILL"
        case 10: return "SIGBUS"
        case 11: return "SIGSEGV"
        case 13: return "SIGPIPE"
        case 15: return "SIGTERM"
        default: return "signal \(code - 128)"
        }
    }

    static func sanitizeStderrDetail(_ raw: String) -> String? {
        let lines = raw.split(whereSeparator: { $0.isNewline })
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
        guard var last = lines.last else { return nil }
        // Trim long worktree prefixes down to the repo-relative source path when present.
        if let range = last.range(of: "/coder/") {
            last = String(last[last.index(after: range.lowerBound)...])
        } else if let range = last.range(of: "/coax/") {
            last = String(last[last.index(after: range.lowerBound)...])
        } else if let range = last.range(of: "/openagents/") {
            last = String(last[last.index(after: range.lowerBound)...])
        }
        if last.count > 180 { last = String(last.prefix(180)) + "…" }
        return last
    }

    /// Reads the exit code from `runs/<id>.state` and the last stderr line from `runs/<id>.err`
    /// written by `coder-host` (host-runs.coax dr_finish) when a daemon-owned worker ends.
    static func runEndDiagnostics(_ id: String) -> (exitCode: Int32?, detail: String?) {
        guard let daemon else { return (nil, nil) }
        var exitCode: Int32?
        let statePath = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(".state") { cext in
                    DesktopCore.callString(4096) { oa_dc_run_file(cdir, cid, cext, $0) }
                }
            }
        }
        if !statePath.isEmpty,
           let data = try? Data(contentsOf: URL(fileURLWithPath: statePath)),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let ex = obj["exit"] as? NSNumber {
            exitCode = ex.int32Value
        }
        var detail: String?
        let errPath = DesktopCore.withCoaxString(daemon.hostDirPath) { cdir in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(".err") { cext in
                    DesktopCore.callString(4096) { oa_dc_run_file(cdir, cid, cext, $0) }
                }
            }
        }
        if !errPath.isEmpty,
           let data = try? Data(contentsOf: URL(fileURLWithPath: errPath)),
           !data.isEmpty {
            detail = sanitizeStderrDetail(String(decoding: data, as: UTF8.self))
        }
        return (exitCode, detail)
    }

    /// True when the worker terminated due to a deterministic fatal crash/trap (SIGABRT/SIGSEGV/etc.)
    /// rather than a transient daemon restart ("lost") or graceful exit.
    var isFatalCrash: Bool {
        if let code = lastExitCode, code > 128 {
            let sig = code - 128
            if sig == 4 || sig == 5 || sig == 6 || sig == 8 || sig == 10 || sig == 11 {
                return true
            }
        }
        if let d = lastEndDetail?.lowercased() {
            if d.contains("slice out of bounds") || d.contains("index out of bounds")
                || d.contains("coax_trap") || d.contains("stack overflow")
                || d.contains("fatal error") || d.contains("assertion failed") {
                return true
            }
        }
        return false
    }

    var crashSignature: String {
        let codePart = lastExitCode.map { String($0) } ?? (lastEndState ?? "exited")
        let detailPart = lastEndDetail ?? ""
        return "\(program)|\(codePart)|\(detailPart)"
    }

    var formattedEndReason: String {
        var parts: [String] = []
        if let code = lastExitCode {
            if let sig = Self.signalName(forExitCode: code) {
                parts.append("\(sig), exit \(code)")
            } else if code != 0 {
                parts.append("exit \(code)")
            } else {
                parts.append(lastEndState ?? "exited")
            }
        } else {
            parts.append(lastEndState ?? "exited")
        }
        if let detail = lastEndDetail, !detail.isEmpty {
            return "\(parts.joined(separator: ", ")): \(detail)"
        }
        return parts.joined(separator: ", ")
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
                // Drain any final lines written just before the worker exited so a completed turn's
                // `ended` event is never lost to the 300ms poll window and misreported as a crash.
                let finalLines = self.readLines(out)
                let diag = CoderWorker.runEndDiagnostics(id)
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.runID == id else { return }
                    for l in finalLines { self.line?(l) }
                    guard self.runID == id else { return }
                    self.lastEndState = state
                    self.lastExitCode = diag.exitCode
                    self.lastEndDetail = diag.detail
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

enum CoderGemini {
    static var file: URL {
        let p = DesktopCore.withCoaxString("gemini-key") { cn in
            DesktopCore.callString(4096) { oa_dc_coder_file(cn, $0) }
        }
        return URL(fileURLWithPath: p)
    }
    static var saved: String? {
        let s = DesktopCore.withCoaxString("gemini-key") { cn in
            DesktopCore.callString(4096) { oa_dc_key_read(cn, $0) }
        }
        return s.isEmpty ? nil : s
    }
    static func save(_ key: String) -> Bool {
        DesktopCore.withCoaxString("gemini-key") { cn in
            DesktopCore.withCoaxString(key) { ck in
                oa_dc_key_save(cn, ck) == 1
            }
        }
    }
}

func coderApplyOpenRouter(_ page: UnsafeMutableRawPointer?) { cd_openrouter(page, CoderOpenRouter.main, CoderOpenRouter.sub) }

/// What the bundled workers report about this Mac, asked of the workers themselves so the app and a
/// run never disagree: the sign-in Gemini models run on (`mcworker --auth`: Vertex AI from a service
/// account or ADC, else a Gemini API key) and the model Antigravity really opens a session on with
/// the current settings (`agyworker --probe`), never "default".
enum CoderProbes {
    private static let d = UserDefaults.standard
    private static var authKnown = false
    private static var authValue = (kind: "none", text: "")
    /// Runs a bundled worker with args and returns its first stdout line ("" when it failed).
    static func ask(_ program: String, _ args: [String], env extra: [String: String] = [:], timeout: TimeInterval) -> String {
        guard let path = Bundle.main.path(forResource: program, ofType: nil) else { return "" }
        let p = Process(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.arguments = args
        p.currentDirectoryURL = FileManager.default.homeDirectoryForCurrentUser
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = CoderWorker.path
        for (k, v) in extra { env[k] = v }
        p.environment = env
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return "" }
        let deadline = Date().addingTimeInterval(timeout)
        while p.isRunning && Date() < deadline { Thread.sleep(forTimeInterval: 0.02) }
        if p.isRunning { p.terminate(); return "" }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)?.split(separator: "\n").first.map(String.init) ?? ""
    }
    /// The sign-in Gemini models run on: ("vertex", "Vertex AI (service account, project P)"),
    /// ("key", "API key ending 1a2b (Settings)") or ("none", ""). Asked once, then on refreshGeminiAuth.
    static var geminiAuth: (kind: String, text: String) {
        if !authKnown { authValue = readAuth(); authKnown = true }
        return authValue
    }
    static var geminiReady: Bool { geminiAuth.kind == "vertex" || geminiAuth.kind == "key" }
    private static func readAuth() -> (kind: String, text: String) {
        let line = ask("mcworker", ["--auth"], timeout: 10)
        let parts = line.split(separator: "\t", maxSplits: 1).map(String.init)
        guard let kind = parts.first, kind == "vertex" || kind == "key" else { return ("none", "") }
        return (kind, parts.count > 1 ? parts[1] : "")
    }
    static func refreshGeminiAuth(_ done: @escaping () -> Void) {
        DispatchQueue.global(qos: .userInitiated).async {
            let v = readAuth()
            DispatchQueue.main.async { authValue = v; authKnown = true; done() }
        }
    }
    /// The settings a probe answered for: a different model, effort, binary or profile asks again.
    private static var agySignature: String { [CoderAntigravity.model, CoderAntigravity.effort, CoderAntigravity.bin, CoderAntigravity.profile].joined(separator: "|") }
    /// The model agy opens a session on with the current settings: (id, name), ("", "") not known yet.
    static var agySeen: (id: String, name: String) {
        guard d.string(forKey: "agy.seen.for") == agySignature else { return ("", "") }
        return (d.string(forKey: "agy.seen.id") ?? "", d.string(forKey: "agy.seen.name") ?? "")
    }
    private static var probing = false
    private static var probedFor: String?   // the settings asked about since launch (agy can change its default on update)
    static func refreshAgyModel(_ done: @escaping () -> Void) {
        let sig = agySignature
        guard CoderWorker.hasAntigravity, !probing, probedFor != sig else { return }
        probing = true; probedFor = sig
        var env = CoderAntigravity.workerEnv()
        if env["CODER_AGY_BIN"] == nil, let agy = CoderAntigravity.resolvedBinaryPath() { env["CODER_AGY_BIN"] = agy }   // the agy the picker found
        DispatchQueue.global(qos: .utility).async {
            let line = ask("agyworker", ["--probe"], env: env, timeout: 60)
            let parts = line.split(separator: "\t", maxSplits: 1).map(String.init)
            DispatchQueue.main.async {
                probing = false
                if let id = parts.first, !id.isEmpty {
                    d.set(sig, forKey: "agy.seen.for"); d.set(id, forKey: "agy.seen.id"); d.set(parts.count > 1 ? parts[1] : id, forKey: "agy.seen.name")
                }
                done()
            }
        }
    }
    static func recordAgySeen(id: String, name: String) {
        guard !id.isEmpty else { return }
        d.set(agySignature, forKey: "agy.seen.for")
        d.set(id, forKey: "agy.seen.id")
        d.set(name.isEmpty ? id : name, forKey: "agy.seen.name")
    }
    /// Both told to a page.
    static func apply(_ page: UnsafeMutableRawPointer?) {
        let a = geminiAuth, seen = agySeen
        cd_settings_gauth(page, a.kind, a.text)
        cd_agy_seen(page, seen.id, seen.name)
    }
}

/// Antigravity CLI (`agy`) detection, version check (>= 1.5.0), and persisted settings (#13803).
enum CoderAntigravity {
    private static let d = UserDefaults.standard
    private static let kBin = "agy.bin"
    private static let kMode = "agy.mode"
    private static let kModel = "agy.model"
    private static let kEffort = "agy.effort"
    private static let kProfile = "agy.profile"
    private static var cachedPath: String?
    private static var cachedVersion: String?
    private static var cacheValid = false

    static var bin: String {
        get { (d.string(forKey: kBin) ?? "").trimmingCharacters(in: .whitespacesAndNewlines) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kBin); invalidateCache() }
    }
    static var mode: String {
        get {
            let raw = (d.string(forKey: kMode) ?? "auto").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            return (raw == "default" || raw == "plan") ? raw : "auto"
        }
        set {
            let raw = newValue.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            d.set((raw == "default" || raw == "plan") ? raw : "auto", forKey: kMode)
        }
    }
    static var modeIndex: Int {
        get {
            switch mode {
            case "default": return 1
            case "plan": return 2
            default: return 0
            }
        }
        set {
            switch newValue {
            case 1: mode = "default"
            case 2: mode = "plan"
            default: mode = "auto"
            }
        }
    }
    static var model: String {
        get { (d.string(forKey: kModel) ?? "").trimmingCharacters(in: .whitespacesAndNewlines) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kModel) }
    }
    static var effort: String {
        get {
            let raw = (d.string(forKey: kEffort) ?? "").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            return (raw == "low" || raw == "medium" || raw == "high") ? raw : ""
        }
        set {
            let raw = newValue.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            d.set((raw == "low" || raw == "medium" || raw == "high") ? raw : "", forKey: kEffort)
        }
    }
    static var profile: String {
        get { (d.string(forKey: kProfile) ?? "").trimmingCharacters(in: .whitespacesAndNewlines) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kProfile) }
    }

    static var defaultProfileURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/coder/antigravity-profile", isDirectory: true)
    }
    static var envFileURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/coder/antigravity.env", isDirectory: false)
    }
    static var effectiveProfilePath: String {
        let custom = profile
        if !custom.isEmpty { return (custom as NSString).expandingTildeInPath }
        return defaultProfileURL.path
    }

    static func invalidateCache() {
        cacheValid = false
        cachedPath = nil
        cachedVersion = nil
    }

    static func resolvedBinaryPath() -> String? {
        let fm = FileManager.default
        let saved = bin
        if !saved.isEmpty {
            let expanded = (saved as NSString).expandingTildeInPath
            if fm.isExecutableFile(atPath: expanded) { return expanded }
        }
        if let custom = ProcessInfo.processInfo.environment["CODER_AGY_BIN"], !custom.isEmpty {
            let expanded = (custom as NSString).expandingTildeInPath
            if fm.isExecutableFile(atPath: expanded) { return expanded }
        }
        let home = fm.homeDirectoryForCurrentUser.path
        var candidates = [
            "\(home)/.local/bin/agy",
            "\(home)/.antigravity/antigravity/bin/agy",
            "/Applications/Antigravity.app/Contents/Resources/app/bin/agy",
            "/opt/homebrew/bin/agy", "/usr/local/bin/agy", "/usr/bin/agy"
        ]
        // The workers' own PATH (~/.local/bin, Homebrew, ...), not only this process's: an app opened
        // from the Dock or Finder gets launchd's bare PATH, and the official installer puts agy in
        // ~/.local/bin, so the model picker showed "No model" with agy installed.
        let launchPath = ProcessInfo.processInfo.environment["PATH"] ?? ""
        for dir in (CoderWorker.path + ":" + launchPath).split(separator: ":") where !dir.isEmpty {
            candidates.append("\(dir)/agy")
        }
        for p in candidates where fm.isExecutableFile(atPath: p) { return p }
        return nil
    }

    static func detectedVersion() -> String? {
        let path = resolvedBinaryPath()
        if cacheValid && cachedPath == path { return cachedVersion }
        cachedPath = path
        cacheValid = true
        guard let path else { cachedVersion = nil; return nil }
        cachedVersion = probeVersion(at: path)
        return cachedVersion
    }

    private static func probeVersion(at path: String) -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: path)
        p.arguments = ["--version"]
        let out = Pipe()
        p.standardOutput = out
        p.standardError = out
        do { try p.run() } catch { return nil }
        p.waitUntilExit()
        let data = out.fileHandleForReading.readDataToEndOfFile()
        guard let text = String(data: data, encoding: .utf8) else { return nil }
        // Extract first semver-like X.Y.Z token
        let range = NSRange(text.startIndex..<text.endIndex, in: text)
        guard let re = try? NSRegularExpression(pattern: "\\b([0-9]+\\.[0-9]+\\.[0-9]+)\\b"),
              let m = re.firstMatch(in: text, options: [], range: range),
              let r = Range(m.range(at: 1), in: text) else { return nil }
        return String(text[r])
    }

    static func isVersionSupported(_ version: String) -> Bool {
        let parts = version.split(separator: ".").compactMap { Int($0) }
        guard parts.count >= 2 else { return false }
        if parts[0] != 1 { return parts[0] > 1 }
        return parts[1] >= 5
    }

    static func statusSummary() -> String {
        guard resolvedBinaryPath() != nil else { return "Not installed" }
        guard let v = detectedVersion() else { return "Installed" }
        return isVersionSupported(v) ? "v\(v) (ready)" : "v\(v) (needs >= 1.5.0)"
    }

    static func modelSummary() -> String {
        let m = model.isEmpty ? "Default" : model
        return effort.isEmpty ? m : "\(m) · \(effort)"
    }

    static func hasEnvADC() -> Bool {
        guard let text = try? String(contentsOf: envFileURL, encoding: .utf8) else { return false }
        for raw in text.split(separator: "\n") {
            let line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            if line.contains("=") { return true }
        }
        return false
    }

    static func authSummary() -> String {
        if hasEnvADC() { return "antigravity.env (ADC)" }
        if !profile.isEmpty { return "Custom profile" }
        if FileManager.default.fileExists(atPath: effectiveProfilePath) { return "Isolated profile (ready)" }
        return "Isolated profile"
    }

    static func workerEnv() -> [String: String] {
        var env: [String: String] = ["CODER_AGY_MODE": mode, "CODER_AGY_INTERACTIVE": "1"]
        if !bin.isEmpty { env["CODER_AGY_BIN"] = (bin as NSString).expandingTildeInPath }
        if !model.isEmpty { env["CODER_AGY_MODEL"] = model }
        if !effort.isEmpty { env["CODER_AGY_EFFORT"] = effort }
        if !profile.isEmpty { env["CODER_AGY_PROFILE_DIR"] = (profile as NSString).expandingTildeInPath }
        return env
    }

    @discardableResult
    static func launchSignIn() -> Bool {
        guard let agy = resolvedBinaryPath() else { return false }
        let profileDir = effectiveProfilePath
        try? FileManager.default.createDirectory(atPath: profileDir, withIntermediateDirectories: true)
        let p = Process()
        p.executableURL = URL(fileURLWithPath: agy)
        p.arguments = ["acp", "--gemini-dir", profileDir, "--app-data-dir", "antigravity-acp", "--login"]
        var env = ProcessInfo.processInfo.environment
        for k in ["GOOGLE_APPLICATION_CREDENTIALS", "GEMINI_API_KEY", "GOOGLE_API_KEY", "GOOGLE_CLOUD_PROJECT", "GOOGLE_CLOUD_QUOTA_PROJECT", "CLOUDSDK_CORE_PROJECT"] {
            env.removeValue(forKey: k)
        }
        env["GEMINI_HOME"] = profileDir
        env["AGY_CLI_DISABLE_AUTO_UPDATE"] = "1"
        if env["SSH_CONNECTION"] == nil { env["SSH_CONNECTION"] = "127.0.0.1 0 127.0.0.1 0" }
        p.environment = env
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        do { try p.run(); return true } catch { return false }
    }
}

/// What a chat is told about the tools the app ships (ple-run; docs/desktop/steer-ple-from-chat.md):
/// the words are Coax's (dc_tools_note); this finds the program. Gym is experimental, and so is this.
func coderToolsNote() -> String {
    guard Experimental.on, let res = Bundle.main.resourcePath else { return "" }
    var out = ""
    let tool = (res as NSString).appendingPathComponent("ple-run")
    if FileManager.default.isExecutableFile(atPath: tool) {
        out += DesktopCore.withCoaxString(tool) { t in DesktopCore.callString(4096) { oa_dc_tools_note(t, $0) } }
    }
    let leanTool = (res as NSString).appendingPathComponent("lean-run")
    if FileManager.default.isExecutableFile(atPath: leanTool) {
        out += DesktopCore.withCoaxString(leanTool) { t in DesktopCore.callString(2048) { oa_dc_lean_tools_note(t, $0) } }
    }
    return out
}

func coderSkillsNote(repoFolder: URL? = nil) -> String {
    return SkillsCatalog.enabledSkillsNote(repoFolder: repoFolder)
}
