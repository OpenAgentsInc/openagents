// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The worker: Microcoder's process or the daemon's run, the Claude key, the OpenRouter models and the skills note.
import AppKit
import Darwin

/// The Microcoder worker (coder/tui/mcworker.coax, built into the bundle): a process that reads
/// one JSON line a message and writes one a line back. This class starts it, carries bytes each
/// way and says when it went away. Every line is written and read by the page's Coax.
///
/// When the unified daemon (`coder host serve`, DaemonClient.swift) owns runs, the daemon starts
/// the worker and keeps it (GitHub #13694): this class writes the run's ID.spec and appends lines
/// to ID.in, and follows ID.out, so the run goes on after Coder quits and Coder attaches to it again
/// when it opens. Without that daemon the worker is Coder's own child process, as before.
final class CoderWorker {
    var line: ((String) -> Void)?      // a line it wrote, on the main queue
    var gone: (() -> Void)?            // it exited, on the main queue
    private var process: Process?
    private var input: FileHandle?
    private var held = Data()

    /// The daemon that owns runs, when there is one (set at launch).
    static var daemon: CoderDaemonClient?
    /// The most workers the daemon runs at once (host-runs.coax DR_MAX), and what Coder says at it.
    static let runLimit = 12
    static let runLimitNote = "Coder runs up to \(runLimit) workers at once, sub-agents included. Stop a chat or wait for one to finish, then send again."
    /// Why the last start failed, in words for the page; nil for the usual "could not start".
    private(set) var failure: String?
    /// The daemon run this worker is, while it is one.
    private(set) var runID: String?
    private var runInput: FileHandle?
    private var tail: DispatchSourceTimer?
    private let tailQueue = DispatchQueue(label: "openagents.coder.worker-tail", qos: .userInitiated)
    private var tailHeld = Data()
    /// Bytes of ID.out already read: where a turn's lines start (CoderTurn.offset).
    private(set) var offset: UInt64 = 0
    private var touched = Date.distantPast

    var alive: Bool { process?.isRunning == true || runID != nil }
    var isDaemonRun: Bool { runID != nil }

    /// The bundled program it runs: Microcoder's worker, or the Claude Code worker for a Claude sub-agent.
    var program = "mcworker"
    /// Additional environment variables injected only for this worker (used by the Foreman worker).
    var extraEnv: [String: String] = [:]
    /// The folders added to the system's short PATH for what the workers start.
    static var path: String {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let had = (ProcessInfo.processInfo.environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin").split(separator: ":").map(String.init)
        let more = [home + "/.local/bin", "/opt/homebrew/bin", "/usr/local/bin"].filter { !had.contains($0) }
        return (more + had).joined(separator: ":")
    }
    /// Whether Claude Code is installed: a claude command on that PATH.
    static var hasClaude: Bool { path.split(separator: ":").contains { FileManager.default.isExecutableFile(atPath: $0 + "/claude") } }

    /// The variables the worker gets on top of what it inherits, and the ones it loses.
    private func environment() -> (set: [String: String], unset: [String]) {
        var env: [String: String] = ["PATH": CoderWorker.path]
        for (k, v) in extraEnv { env[k] = k == "PATH" ? v + ":" + CoderWorker.path : v }
        // The Claude key goes to the Claude worker only: Microcoder's commands never see it.
        if program == "claudeworker", let key = CoderKey.current { env["CODER_CLAUDE_KEY"] = key; return (env, []) }
        return (env, ["CODER_CLAUDE_KEY"])
    }

    func start() -> Bool {
        if alive { return true }
        failure = nil
        guard let path = Bundle.main.path(forResource: program, ofType: nil) else { return false }
        if let daemon = CoderWorker.daemon, daemon.ownsRuns { return startRun(path, daemon) }
        let p = Process(), into = Pipe(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.currentDirectoryURL = FileManager.default.homeDirectoryForCurrentUser
        // An app opened from the Finder has the system's short PATH: the commands Microcoder runs also
        // find what is installed for this person (gamebuilder is in ~/.local/bin).
        var env = ProcessInfo.processInfo.environment
        let (set, unset) = environment()
        for (k, v) in set { env[k] = v }
        for k in unset { env[k] = nil }
        p.environment = env
        p.standardInput = into; p.standardOutput = out; p.standardError = FileHandle.nullDevice
        held = Data()
        out.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let bytes = handle.availableData
            if bytes.isEmpty { handle.readabilityHandler = nil; return }
            DispatchQueue.main.async { self?.read(bytes) }
        }
        p.terminationHandler = { [weak self] ended in
            DispatchQueue.main.async { guard let self, self.process === ended else { return }; self.process = nil; self.input = nil; self.gone?() }
        }
        do { try p.run() } catch { return false }
        process = p; input = into.fileHandleForWriting
        return true
    }
    /// The daemon starts the worker: its spec (mode 600, as it may hold the Claude key), an empty
    /// input, then StartRun. A refusal at the daemon's limit is said on the page (failure).
    private func startRun(_ path: String, _ daemon: CoderDaemonClient) -> Bool {
        let fm = FileManager.default, dir = daemon.runsDir
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let id = UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased()
        let (set, unset) = environment()
        let spec: [String: Any] = ["program": path, "cwd": fm.homeDirectoryForCurrentUser.path,
                                   "env": set.map { $0.key + "=" + $0.value }, "unset": unset]
        guard let data = try? JSONSerialization.data(withJSONObject: spec) else { return false }
        let specURL = dir.appendingPathComponent(id + ".spec")
        guard fm.createFile(atPath: specURL.path, contents: data, attributes: [.posixPermissions: 0o600]),
              fm.createFile(atPath: dir.appendingPathComponent(id + ".in").path, contents: nil, attributes: [.posixPermissions: 0o600]) else { return false }
        attach(id, from: 0)
        touch(force: true)
        if let why = daemon.startRun(id) {
            if why == "run-limit" { failure = CoderWorker.runLimitNote }
            try? fm.removeItem(at: specURL)
            detach(); return false
        }
        follow()
        return true
    }
    /// This worker is the daemon's run `id` again, from byte `from` of its output (a reopened Coder).
    func attach(_ id: String, from: UInt64) {
        guard let daemon = CoderWorker.daemon else { return }
        runID = id; offset = from; tailHeld = Data()
        // O_APPEND: the daemon appends the paired phone's lines to the same file (host-desktop.coax,
        // GitHub #13713), so every write lands at the end and no line overwrites another.
        let fd = open(daemon.runsDir.appendingPathComponent(id + ".in").path, O_WRONLY | O_APPEND | O_CLOEXEC)
        runInput = fd >= 0 ? FileHandle(fileDescriptor: fd, closeOnDealloc: true) : nil
    }
    /// The whole lines of the run's output not read yet, read now (a reattached run's replay).
    func readAvailable() -> [String] {
        guard let id = runID, let daemon = CoderWorker.daemon else { return [] }
        return tailQueue.sync { readLines(daemon.runsDir.appendingPathComponent(id + ".out")) }
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
    /// The run's state as the daemon keeps it: running, stopping, exited, stopped or lost.
    static func runState(_ id: String) -> String? {
        guard let daemon, let data = try? Data(contentsOf: daemon.runsDir.appendingPathComponent(id + ".state")),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return obj["state"] as? String
    }
    /// Coder is attached: the daemon leaves the worker's asks to it (host-runs.coax dr_detached).
    private func touch(force: Bool = false) {
        guard let id = runID, let daemon = CoderWorker.daemon, force || Date().timeIntervalSince(touched) > 1 else { return }
        touched = Date()
        let url = daemon.runsDir.appendingPathComponent(id + ".attached")
        if !FileManager.default.createFile(atPath: url.path, contents: Data("\(Int(touched.timeIntervalSince1970))".utf8), attributes: [.posixPermissions: 0o600]) { return }
    }
    /// Lines as the worker writes them, every 30 ms, until it ends or Coder lets it go.
    func follow() {
        guard let id = runID, let daemon = CoderWorker.daemon, tail == nil else { return }
        let out = daemon.runsDir.appendingPathComponent(id + ".out")
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
            // it ended: what it wrote is read, so the page hears it went away
            if let state = CoderWorker.runState(id), state != "running", state != "stopping" {
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.runID == id else { return }
                    self.tail?.cancel(); self.tail = nil
                    self.runID = nil; try? self.runInput?.close(); self.runInput = nil
                    self.gone?()
                }
            }
        }
        tail = timer
        timer.resume()
    }
    /// Coder lets the run go without stopping it (Coder quits): the daemon keeps it running.
    func detach() {
        tail?.cancel(); tail = nil
        try? runInput?.close(); runInput = nil
        if let id = runID, let daemon = CoderWorker.daemon { try? FileManager.default.removeItem(at: daemon.runsDir.appendingPathComponent(id + ".attached")) }
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
        if let runInput { try? runInput.write(contentsOf: data); return }
        guard let input else { return }
        try? input.write(contentsOf: data)
    }
    /// Closes its input first: a worker ends on that and stops what it started (the Claude worker
    /// kills the claude process group). Half a second later it is terminated if it is still there.
    /// A daemon run is stopped by the daemon the same way; with `forget` its files go once it ends.
    func stop(forget: Bool = true) {
        if let id = runID, let daemon = CoderWorker.daemon {
            detach()
            daemon.stopRun(id) { if forget { DispatchQueue.main.asyncAfter(deadline: .now() + 4) { daemon.forgetRun(id) } } }
            return
        }
        let p = process, i = input
        process = nil; input = nil
        try? i?.close()
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { if p?.isRunning == true { p?.terminate() } }
    }
}

/// The key Claude sub-agents use in place of this Mac's Claude Code login: an Anthropic API key, or
/// a Claude Code token from `claude setup-token`. CODER_CLAUDE_KEY in Coder's launch environment
/// wins; else the key set with Coder > Claude Key, kept in ~/Library/Application Support/Coder/claude-key,
/// readable by this user only. nil: the login is used.
enum CoderKey {
    static var file: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        return base.appendingPathComponent("Coder/claude-key")
    }
    static var fromLaunch: String? {
        let key = (ProcessInfo.processInfo.environment["CODER_CLAUDE_KEY"] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return key.isEmpty ? nil : key
    }
    static var saved: String? {
        let key = ((try? String(contentsOf: file, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return key.isEmpty ? nil : key
    }
    static var current: String? { fromLaunch ?? saved }
    /// Keeps a key (mode 600), or removes the kept one when key is empty. False when it could not be written.
    static func save(_ key: String) -> Bool {
        let key = key.trimmingCharacters(in: .whitespacesAndNewlines)
        if key.isEmpty { try? FileManager.default.removeItem(at: file); return saved == nil }
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        guard FileManager.default.createFile(atPath: file.path, contents: Data(key.utf8), attributes: [.posixPermissions: 0o600]) else { return false }
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
        return true
    }
    /// The end of a key, for showing which one is set: "…" and its last 4 characters.
    static func tail(_ key: String) -> String { "…" + key.suffix(4) }
}

/// The OpenRouter models Coder offers (Coder > OpenRouter Models): main is a composer picker entry
/// for Microcoder, sub a sub-agent level. Each is an OpenRouter id (VENDOR/MODEL), kept in the
/// app's defaults. The key is kept in ~/Library/Application Support/Coder/openrouter-key, readable by
/// this user only; the Microcoder worker reads it there (coder/tui/microcoder_keys.coax) and puts it
/// in curl's environment only. Without it the worker uses ~/work/.secrets/openrouter.env.
enum CoderOpenRouter {
    static var main: String { UserDefaults.standard.string(forKey: "openRouterMain") ?? "" }
    static var sub: String { UserDefaults.standard.string(forKey: "openRouterSub") ?? "" }
    static func setModels(main: String, sub: String) {
        UserDefaults.standard.set(main, forKey: "openRouterMain"); UserDefaults.standard.set(sub, forKey: "openRouterSub")
    }
    static var file: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        return base.appendingPathComponent("Coder/openrouter-key")
    }
    static var saved: String? {
        let key = ((try? String(contentsOf: file, encoding: .utf8)) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return key.isEmpty ? nil : key
    }
    /// Whether the worker finds a key: the one set here, or ~/work/.secrets/openrouter.env.
    static var hasKey: Bool {
        saved != nil || FileManager.default.fileExists(atPath: FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("work/.secrets/openrouter.env").path)
    }
    /// Keeps a key (mode 600), or removes the kept one when key is empty. False when it could not be written.
    static func save(_ key: String) -> Bool {
        let key = key.trimmingCharacters(in: .whitespacesAndNewlines)
        if key.isEmpty { try? FileManager.default.removeItem(at: file); return saved == nil }
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        guard FileManager.default.createFile(atPath: file.path, contents: Data(key.utf8), attributes: [.posixPermissions: 0o600]) else { return false }
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
        return true
    }
    /// The models among ids that OpenRouter does not list as taking a strict JSON schema
    /// (structured_outputs), which every Microcoder step sends. nil when the list could not be read.
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
/// Gives a page the OpenRouter models set now.
func coderApplyOpenRouter(_ page: UnsafeMutableRawPointer?) { cd_openrouter(page, CoderOpenRouter.main, CoderOpenRouter.sub) }
// The skills installed for this person (~/.claude/skills/NAME/SKILL.md), in words for Microcoder:
// each one's name, its file and its description. The first 16 skills, 300 characters of each
// description. "" when there are none.
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
