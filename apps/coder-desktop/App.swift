// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
import AppKit
import Darwin

/// The Microcoder worker (coder/tui/mcworker.coax, built into the bundle): a process that reads
/// one JSON line a message and writes one a line back. This class starts it, carries bytes each
/// way and says when it went away. Every line is written and read by the page's Coax.
final class CoderWorker {
    var line: ((String) -> Void)?      // a line it wrote, on the main queue
    var gone: (() -> Void)?            // it exited, on the main queue
    private var process: Process?
    private var input: FileHandle?
    private var held = Data()

    var alive: Bool { process?.isRunning == true }

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

    func start() -> Bool {
        if alive { return true }
        guard let path = Bundle.main.path(forResource: program, ofType: nil) else { return false }
        let p = Process(), into = Pipe(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.currentDirectoryURL = FileManager.default.homeDirectoryForCurrentUser
        // An app opened from the Finder has the system's short PATH: the commands Microcoder runs also
        // find what is installed for this person (gamebuilder is in ~/.local/bin).
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = CoderWorker.path
        for (k, v) in extraEnv {
            if k == "PATH" {
                env["PATH"] = v + ":" + (env["PATH"] ?? CoderWorker.path)
            } else {
                env[k] = v
            }
        }
        // The Claude key goes to the Claude worker only: Microcoder's commands never see it.
        env["CODER_CLAUDE_KEY"] = program == "claudeworker" ? CoderKey.current : nil
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
    private func read(_ bytes: Data) {
        held.append(bytes)
        while let end = held.firstIndex(of: 0x0a) {
            let one = held.subdata(in: held.startIndex..<end)
            held.removeSubrange(held.startIndex...end)
            if let text = String(data: one, encoding: .utf8), !text.isEmpty { line?(text) }
        }
    }
    func write(_ bytes: [CChar], count: Int) {
        guard alive, count > 0, let input else { return }
        let data = bytes.prefix(count).withUnsafeBytes { Data($0) }
        try? input.write(contentsOf: data)
    }
    /// Closes its input first: a worker ends on that and stops what it started (the Claude worker
    /// kills the claude process group). Half a second later it is terminated if it is still there.
    func stop() {
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

/// The window: the compiled page fills it. The platform editor sits over the composer's text box
/// and hands Coax each whole draft; what the page asks for (send, stop, a new chat, copy) is done here.
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

/// One kept chat in the list: its id (its file's name), its title, when it was created, when it last changed,
/// its working folder, and whether the user gave it a custom title that should not be overwritten on subsequent saves.
struct CoderChat: Codable {
    var id: String
    var title: String
    var created: Double? = nil
    var updated: Double
    var folder: String
    var renamed: Bool? = nil
    var pinned: Int? = nil
    var managedBy: String? = nil

    enum CodingKeys: String, CodingKey {
        case id, title, created, updated, folder, renamed, pinned
        case managedBy = "managed_by"
    }
}
/// The chats kept on this Mac: ~/Library/Application Support/Coder/chats, one file a chat (ID.json,
/// the page's text) and index.json, the list, newest created first.
final class CoderStore {
    private let dir: URL
    private(set) var list: [CoderChat] = []
    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        dir = base.appendingPathComponent("Coder/chats")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: dir.appendingPathComponent("index.json")), let kept = try? JSONDecoder().decode([CoderChat].self, from: data) {
            list = kept.enumerated().sorted { a, b in
                let ta = a.element.created ?? a.element.updated
                let tb = b.element.created ?? b.element.updated
                return ta != tb ? ta > tb : a.offset < b.offset
            }.map(\.element)
        }
    }
    /// One-line title: collapses whitespace/tabs/newlines and clamps cleanly to 120 UTF-8 bytes (CP_CHAT_TITLE).
    static func cleanTitle(_ raw: String) -> String {
        let collapsed = raw.components(separatedBy: .whitespacesAndNewlines).filter { !$0.isEmpty }.joined(separator: " ")
        var out = ""
        var bytes = 0
        for ch in collapsed {
            let n = ch.utf8.count
            if bytes + n > 120 { break }
            out.append(ch)
            bytes += n
        }
        return out
    }
    private func saveIndex() {
        if let data = try? JSONEncoder().encode(list) { try? data.write(to: dir.appendingPathComponent("index.json"), options: .atomic) }
    }
    var pinnedList: [CoderChat] { list.filter { $0.id != Foreman.chatID && $0.pinned != nil }.sorted { ($0.pinned ?? 0) < ($1.pinned ?? 0) } }
    var recentList: [CoderChat] { list.filter { $0.id != Foreman.chatID && $0.pinned == nil } }
    var orderedList: [CoderChat] { pinnedList + recentList }
    /// The list as the page reads it: one chat a line, its id, a tab, its title.
    var lines: String { orderedList.map { $0.id + "\t" + $0.title }.joined(separator: "\n") }
    func text(_ id: String) -> String? { try? String(contentsOf: dir.appendingPathComponent(id + ".json"), encoding: .utf8) }
    func folder(_ id: String) -> String? { list.first { $0.id == id }?.folder }
    func title(_ id: String) -> String? { list.first { $0.id == id }?.title }
    func meta(_ id: String) -> CoderChat? { list.first { $0.id == id } }
    func isPinned(_ id: String) -> Bool { list.first { $0.id == id }?.pinned != nil }
    func isManagedByForeman(_ id: String) -> Bool { list.first { $0.id == id }?.managedBy == Foreman.managedTag }
    private func normalizePins() {
        let ids = pinnedList.map(\.id)
        for (order, id) in ids.enumerated() {
            if let i = list.firstIndex(where: { $0.id == id }) { list[i].pinned = order }
        }
    }
    /// Pins or unpins a chat, persisting its state and order in index.json.
    func togglePin(_ id: String) {
        guard let i = list.firstIndex(where: { $0.id == id }) else { return }
        if list[i].pinned != nil {
            list[i].pinned = nil
            normalizePins()
        } else {
            let next = (list.compactMap(\.pinned).max() ?? -1) + 1
            list[i].pinned = next
        }
        saveIndex()
    }
    /// Reorders a pinned chat to the given 0-based index in the Pinned section.
    func movePinned(_ id: String, to index: Int) {
        var ids = pinnedList.map(\.id)
        guard let from = ids.firstIndex(of: id), !ids.isEmpty else { return }
        let to = max(0, min(ids.count - 1, index))
        guard from != to else { return }
        let moved = ids.remove(at: from)
        ids.insert(moved, at: to)
        for (order, cid) in ids.enumerated() {
            if let i = list.firstIndex(where: { $0.id == cid }) { list[i].pinned = order }
        }
        saveIndex()
    }
    /// Renames a kept chat without changing its place in the list.
    func rename(_ id: String, to title: String) {
        let clean = Self.cleanTitle(title)
        guard !clean.isEmpty, let i = list.firstIndex(where: { $0.id == id }) else { return }
        list[i].title = clean
        list[i].renamed = true
        saveIndex()
    }
    /// Deletes a chat: its file and its place in the list. It can't be brought back.
    func remove(_ id: String) {
        try? FileManager.default.removeItem(at: dir.appendingPathComponent(id + ".json"))
        list.removeAll { $0.id == id }
        normalizePins()
        saveIndex()
    }
    /// Keeps a chat's text. A new chat joins the top of the list; an existing chat updates in place
    /// so chats stay ordered chronologically by creation time (newest created first).
    func keep(id: String, title: String, folder: String, text: String, managedBy: String? = nil) {
        guard (try? text.write(to: dir.appendingPathComponent(id + ".json"), atomically: true, encoding: .utf8)) != nil else { return }
        let now = Date().timeIntervalSince1970
        if let i = list.firstIndex(where: { $0.id == id }) {
            if list[i].created == nil { list[i].created = list[i].updated }
            if list[i].renamed != true && id != Foreman.chatID { list[i].title = title }
            list[i].updated = now
            list[i].folder = folder
            if let managedBy { list[i].managedBy = managedBy }
        } else {
            let effectiveTitle = id == Foreman.chatID ? Foreman.title : title
            list.insert(CoderChat(id: id, title: effectiveTitle, created: now, updated: now, folder: folder, managedBy: managedBy), at: 0)
        }
        saveIndex()
    }
    func registerChat(id: String, title: String, folder: String, managedBy: String? = nil) {
        let now = Date().timeIntervalSince1970
        let clean = Self.cleanTitle(title)
        let effectiveTitle = clean.isEmpty ? "New chat" : clean
        if let i = list.firstIndex(where: { $0.id == id }) {
            list[i].title = effectiveTitle
            list[i].renamed = true
            list[i].updated = now
            list[i].folder = folder
            if let managedBy { list[i].managedBy = managedBy }
        } else {
            list.insert(CoderChat(id: id, title: effectiveTitle, created: now, updated: now, folder: folder, renamed: true, managedBy: managedBy), at: 0)
        }
        saveIndex()
    }
}

/// One chat's run: the page that reads its lines, its worker and its sub-agents' workers. The chat
/// that is shown runs on the window's page. One whose reply goes on out of view, after New Chat or
/// a press on another chat, has a page with no window (cd_headless) until it ends or is opened again.
final class CoderRun {
    var page: UnsafeMutableRawPointer
    let worker = CoderWorker()
    var agents: [Int32: CoderWorker] = [:]
    let session = "coder-desktop-" + UUID().uuidString.lowercased()
    var folder: URL                    // its working folder, once it is out of view
    var chat: String?                  // its kept chat's id, once it is out of view
    init(page: UnsafeMutableRawPointer, folder: URL) { self.page = page; self.folder = folder }
    func stop() {
        worker.gone = nil; worker.line = nil; worker.stop()
        for one in agents.values { one.gone = nil; one.line = nil; one.stop() }
        agents = [:]
    }
}

final class CoderWindow: NSObject, NSWindowDelegate {
    let window: NSWindow
    private let page: UnsafeMutableRawPointer
    private let canvas: NSView
    private let editor: KitMacEditor
    private var run: CoderRun                    // the shown chat's run, on the window's page
    private var runs: [String: CoderRun] = [:]   // chats whose replies run out of view, by id
    private var unread: Set<String> = []         // background chats with unread output or a completed turn
    private var last = CACurrentMediaTime()      // when the clock last ticked, for their pages' seconds
    private var line: CGFloat
    private var clock: Timer?
    private var then: String?   // --then TEXT: sent in a new chat once --send's reply ends
    private var also: String?   // --new TEXT: sent in a new chat while --send's reply runs
    private var showAgent: Int?   // --agent N: the Nth sub-agent's transcript opens as soon as there is one
    private var owed = 3   // frames still to draw after a change
    private var restoreEditorFocus = false
    private let store = CoderStore()
    private var chat: String?          // the id of the chat that is shown, once it is kept
    private var recallDue = false      // a chat was opened: the worker is told its turns ahead of the next message
    private var choosingFolder = false // prevents stacking folder sheets while one is opening or open
    private let launchBinary = CoderWindow.currentBinaryIdentity()
    private let foremanServer = ForemanControlServer()
    private let foremanEvents = ForemanEventBus()
    private let foremanAudit = ForemanAuditLog()
    private let daemonClient = CoderDaemonClient()
    private var lastForegroundRunning = false
    var foremanTier: ForemanTier = ForemanTier.load()
    private let updater = CoderUpdater()

    init?(resources: String) {
        guard let page = cd_open(resources), let raw = cd_view(page) else { return nil }
        self.page = page
        run = CoderRun(page: page, folder: FileManager.default.homeDirectoryForCurrentUser)
        canvas = Unmanaged<NSView>.fromOpaque(raw).takeUnretainedValue()
        // the text size chosen last time (a row of the page's sizes; 2 is the usual)
        if UserDefaults.standard.object(forKey: "zoom") != nil { _ = cd_zoom_set(page, Int32(UserDefaults.standard.integer(forKey: "zoom"))) }
        // the sidebars' widths as dragged last time
        cd_sides_set(page, UserDefaults.standard.float(forKey: "sideWidth"), UserDefaults.standard.float(forKey: "paneWidth"))
        var style = [Float](repeating: 0, count: 14); _ = cd_style(page, &style)
        editor = KitMacEditor(style: style); line = CGFloat(style[1])
        // Most of the screen, up to 1500 by 1000 points.
        let screen = NSScreen.main?.visibleFrame.size ?? NSSize(width: 1440, height: 900)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: min(1500, (screen.width * 0.86).rounded()), height: min(1000, (screen.height * 0.88).rounded())),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        super.init()
        window.title = "Coder"; window.minSize = NSSize(width: 420, height: 320); window.isReleasedWhenClosed = false; window.delegate = self
        canvas.frame = window.contentLayoutRect; canvas.autoresizingMask = [.width, .height]
        window.contentView = canvas
        cd_on_change(page, { raw in guard let raw else { return }; Unmanaged<CoderWindow>.fromOpaque(raw).takeUnretainedValue().wake() },
                     Unmanaged.passUnretained(self).toOpaque())
        cd_skills(page, coderSkillsNote())
        cd_claude(page, CoderWorker.hasClaude ? 1 : 0)
        coderApplyOpenRouter(page)
        cd_chats(page, chatLines)
        syncForemanUI()
        foremanServer.handler = { [weak self] request, done in
            guard let self else {
                done(ForemanResponse(status: 503, body: ["error": "window unavailable"]))
                return
            }
            self.handleForemanAsyncRequest(request, done: done)
        }
        _ = foremanServer.start()
        syncFromDaemon()
        // the working folder: --folder PATH for this launch, else the one chosen last, else the home folder
        var isFolder: ObjCBool = false
        if let at = CommandLine.arguments.firstIndex(of: "--folder"), at + 1 < CommandLine.arguments.count,
           FileManager.default.fileExists(atPath: CommandLine.arguments[at + 1], isDirectory: &isFolder), isFolder.boolValue {
            setFolder(URL(fileURLWithPath: CommandLine.arguments[at + 1]), keep: false)
        } else if let kept = UserDefaults.standard.string(forKey: "folder"), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue {
            setFolder(URL(fileURLWithPath: kept), keep: false)
        } else { setFolder(folder, keep: false) }
        editor.committed = { [weak self] text in guard let self else { return }; _ = cd_set_text(self.page, text); self.wake() }
        // Return sends; Shift-Return and Option-Return make a new line.
        editor.command = { [weak self] selector in
            guard let self, selector == #selector(NSResponder.insertNewline(_:)) else { return false }
            if NSApp.currentEvent?.modifierFlags.contains(.shift) == true { return false }
            _ = cd_submit(self.page); self.wake(); return true
        }
        editor.setAccessibilityIdentifier("coder.composer"); editor.setAccessibilityLabel("Message")
        canvas.addSubview(editor)
        wire(run)
        canvas.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: canvas, queue: .main) { [weak self] _ in self?.wake() }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { [weak self] event in self?.rightClick(event) == true ? nil : event }
        let clock = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(clock, forMode: .common); self.clock = clock
        window.center(); window.makeKeyAndOrderFront(nil); window.makeFirstResponder(editor)
        // Coder --foreman opens the persistent Foreman chat at launch.
        if CommandLine.arguments.contains("--foreman") { openForeman() }
        // Coder --chat N opens the Nth most recent chat at launch (1 is the newest).
        if let at = CommandLine.arguments.firstIndex(of: "--chat"), at + 1 < CommandLine.arguments.count, let k = Int(CommandLine.arguments[at + 1]) { openChat(at: k) }
        // Coder --send TEXT sends TEXT at launch, for a check of the whole path with no typing.
        if let at = CommandLine.arguments.firstIndex(of: "--model"), at + 1 < CommandLine.arguments.count { _ = cd_pick_model(page, CommandLine.arguments[at + 1]) }
        if let at = CommandLine.arguments.firstIndex(of: "--send"), at + 1 < CommandLine.arguments.count, cd_set_text(page, CommandLine.arguments[at + 1]) != 0, cd_submit(page) != 0,
           let next = CommandLine.arguments.firstIndex(of: "--then"), next + 1 < CommandLine.arguments.count { then = CommandLine.arguments[next + 1] }
        // Coder --send A --new B sends B in a new chat while A's reply runs, which goes on out of view.
        if let at = CommandLine.arguments.firstIndex(of: "--new"), at + 1 < CommandLine.arguments.count { also = CommandLine.arguments[at + 1] }
        // Coder --agent N opens the transcript of the Nth sub-agent of the newest message that has one.
        if let at = CommandLine.arguments.firstIndex(of: "--agent"), at + 1 < CommandLine.arguments.count { showAgent = Int(CommandLine.arguments[at + 1]) }
        updater.onStateChange = { [weak self] _ in self?.syncUpdateUI() }
        syncUpdateUI()
        updater.startPeriodicChecks()
        tick()
    }

    func wake() { owed = 3 }

    /// Marks a background chat unread when it produces output or completes a turn while unfocused.
    private func markUnread(_ r: CoderRun) {
        guard r !== run, let id = r.chat, id != chat else { return }
        if unread.insert(id).inserted { cd_chats(page, chatLines) }
    }
    /// A run's worker lines go to its page, wherever that page is now.
    private func wire(_ r: CoderRun) {
        r.worker.line = { [weak self, weak r] text in guard let self, let r else { return }; cd_line(r.page, text); self.markUnread(r); self.pump(r) }
        r.worker.gone = { [weak self, weak r] in guard let self, let r else { return }; cd_failed(r.page, "Microcoder stopped running. Send the message again."); self.markUnread(r); self.pump(r) }
    }
    /// After any worker's line: answer what is owed, start the sub-agents the page asks for, and
    /// stop the workers of those that have ended. Microcoder hands tasks over in one line; each runs
    /// on a worker of its own, all at once, and the run's page reads every line. (Each count is
    /// taken before its write: see act.)
    private func stopRun(_ r: CoderRun) {
        var out = [CChar](repeating: 0, count: 4096)
        let n = cd_stop_line(&out, Int32(out.count))
        r.worker.write(out, count: Int(n))
        for (_, one) in r.agents { one.write(out, count: Int(n)) }
    }
    private func pump(_ r: CoderRun) {
        var out = [CChar](repeating: 0, count: 300_000)
        let page = r.page, cwd = (r === run ? folder : r.folder).path
        if r !== run && cd_action(page) == 3 { stopRun(r) }
        if cd_budget_exceeded(page) == 0 {
            while true {
                let j = cd_agent_next(page)
                if j < 0 { break }
                let one = CoderWorker()
                if cd_agent_claude(page, j) != 0 { one.program = "claudeworker" }
                one.line = { [weak self, weak r] text in guard let self, let r, r.agents[j] === one else { return }; cd_agent_line(r.page, j, text); self.markUnread(r); self.pump(r) }
                one.gone = { [weak self, weak r] in guard let self, let r, r.agents[j] === one else { return }; r.agents[j] = nil; cd_agent_failed(r.page, j, "The sub-agent's worker stopped running."); self.markUnread(r); self.pump(r) }
                guard one.start() else { cd_agent_failed(page, j, "The sub-agent's worker could not start."); continue }
                r.agents[j] = one
                let n = cd_agent_task(page, j, r.session + "-agent-\(j)-" + UUID().uuidString.lowercased(), cwd, &out, Int32(out.count))
                one.write(out, count: Int(n))
            }
        }
        for (j, one) in r.agents {
            let n = cd_agent_answer_line(page, j, &out, Int32(out.count))
            one.write(out, count: Int(n))
            if cd_agent_running(page, j) == 0 { r.agents[j] = nil; one.gone = nil; one.line = nil; one.stop() }
        }
        let n = cd_answer_line(page, &out, Int32(out.count)); r.worker.write(out, count: Int(n))
        wake()
    }
    /// The list as the page reads it: one chat a line, its id, a tab, its title, and optional tabs
    /// for busy, pinned, managed and unread flags.
    private var chatLines: String {
        store.orderedList.map { chat in
            let isBusy = runs[chat.id] != nil
            let isPinned = chat.pinned != nil
            let isManaged = chat.managedBy == Foreman.managedTag
            let isUnread = unread.contains(chat.id)
            return chat.id + "\t" + chat.title + "\t" + (isBusy ? "1" : "0") + "\t" + (isPinned ? "1" : "0") + "\t" + (isManaged ? "1" : "0") + "\t" + (isUnread ? "1" : "0")
        }.joined(separator: "\n")
    }
    private var isForemanShown: Bool { chat == Foreman.chatID }
    private func syncFromDaemon() {
        daemonClient.readForeman { [weak self] state in
            guard let self, let rawTier = state?.tier, let tier = ForemanTier(rawValue: rawTier) else { return }
            if self.foremanTier != tier {
                self.foremanTier = tier
                tier.save()
                self.syncForemanUI()
                self.wake()
            }
        }
        daemonClient.listSessionMeta { [weak self] items in
            guard let self, !items.isEmpty else { return }
            var changed = false
            for item in items {
                guard self.store.meta(item.session_id) != nil else { continue }
                if let title = item.title, !title.isEmpty, self.store.title(item.session_id) != title {
                    self.store.rename(item.session_id, to: title)
                    changed = true
                }
                if let pinned = item.pinned, self.store.isPinned(item.session_id) != pinned {
                    self.store.togglePin(item.session_id)
                    changed = true
                }
            }
            if changed {
                cd_chats(self.page, self.chatLines)
                self.wake()
            }
        }
    }
    private func syncForemanUI() {
        let busy = runs[Foreman.chatID] != nil || (chat == Foreman.chatID && cd_running(page) != 0)
        cd_foreman(page, busy ? 1 : 0, Int32(foremanTier.rawValue))
    }
    private func syncUpdateUI() {
        let st = updater.state
        cd_update(page, st.coaxCode, st.pillLabel, st.pillDetail)
        wake()
    }
    func checkForUpdates() {
        updater.checkNow(userInitiated: true) { [weak self] state in
            guard let self else { return }
            let alert = NSAlert()
            alert.messageText = "Coder Updates"
            switch state {
            case .hidden:
                alert.informativeText = "Coder is up to date."
                alert.beginSheetModal(for: self.window)
            case .error(let msg, _):
                alert.alertStyle = .warning
                alert.informativeText = msg
                alert.beginSheetModal(for: self.window)
            case .available, .downloading, .ready:
                break
            }
        }
    }
    private func performInstallAndRelaunch() {
        switch updater.applyStagedAndRelaunch() {
        case .success:
            retire()
            NSApp.terminate(nil)
        case .failure(let err):
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = "Update Failed"
            alert.informativeText = err.localizedDescription
            alert.beginSheetModal(for: window)
        }
    }
    private func handleUpdatePillClick() {
        switch updater.state {
        case .available(_, let source):
            if case .signedRelease(let manifest) = source {
                updater.beginDownload(for: manifest)
            }
        case .ready:
            let count = runningCount
            if count > 0 {
                let alert = NSAlert()
                alert.alertStyle = .warning
                alert.messageText = count == 1 ? "An agent task is still running" : "\(count) agent tasks are still running"
                alert.informativeText = "Restarting to install the update will stop all running agent tasks."
                alert.addButton(withTitle: "Cancel")
                alert.addButton(withTitle: "Restart & Update")
                alert.beginSheetModal(for: window) { [weak self] response in
                    guard response == .alertSecondButtonReturn else { return }
                    self?.performInstallAndRelaunch()
                }
                return
            }
            performInstallAndRelaunch()
        case .error:
            updater.checkNow(userInitiated: true)
        default:
            break
        }
    }
    private func syncSkillsNote(for pagePtr: UnsafeMutableRawPointer, isForeman: Bool) {
        let base = coderSkillsNote()
        if isForeman {
            let note = Foreman.systemPrompt(tier: foremanTier, workspace: folder.path, chatCount: store.orderedList.count, runningCount: runningCount, baseSkills: base)
            cd_skills(pagePtr, note)
        } else {
            cd_skills(pagePtr, base)
        }
    }
    private func configureWorkerEnv(_ r: CoderRun, isForeman: Bool) {
        var env: [String: String] = [:]
        if isForeman && foremanServer.port > 0 {
            env = [
                "CODER_FOREMAN_URL": foremanServer.baseURL,
                "CODER_FOREMAN_TOKEN": foremanServer.token,
                "CODER_FOREMAN_TIER": foremanTier.wireName
            ]
            if let res = Bundle.main.resourcePath {
                env["PATH"] = res
            }
        }
        let changed = r.worker.extraEnv != env
        r.worker.extraEnv = env
        if changed && r.worker.alive && cd_running(r.page) == 0 {
            r.worker.stop()
            if r === run { recallDue = true }
        }
    }
    func setForemanTier(_ tier: ForemanTier) {
        let previous = foremanTier
        foremanTier = tier
        tier.save()
        daemonClient.updateForeman(tier: tier.rawValue)
        if isForemanShown {
            syncSkillsNote(for: page, isForeman: true)
            configureWorkerEnv(run, isForeman: true)
        }
        if previous != tier {
            _ = foremanEvents.emit(type: "tier.changed", chatID: nil, data: ["from": previous.wireName, "to": tier.wireName])
        }
        syncForemanUI()
        wake()
    }
    func clearForemanHistory() {
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = "Clear Foreman history?"
        alert.informativeText = "This removes the Foreman conversation history. Managed chats are not deleted."
        alert.addButton(withTitle: "Clear History")
        alert.addButton(withTitle: "Cancel")
        alert.buttons[0].hasDestructiveAction = true
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self, answer == .alertFirstButtonReturn else { return }
            if self.chat == Foreman.chatID {
                if cd_running(self.page) != 0 { self.stopRun(self.run) }
                _ = cd_new(self.page)
                self.recallDue = false
                var out = [CChar](repeating: 0, count: 256)
                let n = cd_forget_line(&out, Int32(out.count)); self.run.worker.write(out, count: Int(n))
            } else if let back = self.runs.removeValue(forKey: Foreman.chatID) {
                self.stopRun(back); back.stop(); cd_close(back.page)
            }
            self.store.remove(Foreman.chatID)
            cd_chats(self.page, self.chatLines); self.syncForemanUI(); self.wake()
        }
    }
    func openForemanAuditLog() {
        let url = foremanAudit.fileURL
        let fm = FileManager.default
        if !fm.fileExists(atPath: url.path) {
            try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            fm.createFile(atPath: url.path, contents: Data())
        }
        NSWorkspace.shared.open(url)
    }
    func openForeman() {
        if chat == Foreman.chatID { window.makeFirstResponder(editor); wake(); return }
        guard detach() else { return }
        if let back = runs[Foreman.chatID] {
            guard cd_take(page, back.page) != 0 else { return }
            runs[Foreman.chatID] = nil; cd_close(back.page); back.page = page
            run.stop()
            run = back
            chat = Foreman.chatID; recallDue = false; cd_current(page, Foreman.chatID)
            setFolder(back.folder, keep: false)
            configureWorkerEnv(run, isForeman: true)
            syncSkillsNote(for: page, isForeman: true)
            cd_chats(page, chatLines); syncForemanUI()
            window.makeFirstResponder(editor); wake()
            return
        }
        if let text = store.text(Foreman.chatID), cd_load(page, text) != 0 {
            recallDue = true
        } else {
            _ = cd_new(page)
            recallDue = false
            var out = [CChar](repeating: 0, count: 256)
            let n = cd_forget_line(&out, Int32(out.count)); run.worker.write(out, count: Int(n))
        }
        chat = Foreman.chatID; cd_current(page, Foreman.chatID)
        var isFolder: ObjCBool = false
        if let kept = store.folder(Foreman.chatID), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue {
            setFolder(URL(fileURLWithPath: kept), keep: false)
        }
        configureWorkerEnv(run, isForeman: true)
        syncSkillsNote(for: page, isForeman: true)
        cd_chats(page, chatLines); syncForemanUI()
        window.makeFirstResponder(editor); wake()
    }
    func stopAllChats() {
        if cd_running(page) != 0 { stopRun(run) }
        let ids = Array(runs.keys)
        for id in ids {
            if let back = runs.removeValue(forKey: id) {
                stopRun(back)
                keep(back)
                back.stop()
                cd_close(back.page)
            }
        }
        cd_chats(page, chatLines)
        syncForemanUI()
        wake()
    }
    /// The shown chat's reply goes on out of view: its run moves to a page with no window and the
    /// window gets a new run. True when nothing runs in the window now.
    @discardableResult private func detach() -> Bool {
        guard cd_running(page) != 0 else { return true }
        keep()
        guard let id = chat, let back = cd_headless() else { return false }
        coderApplyOpenRouter(back)
        guard cd_take(back, page) != 0 else { cd_close(back); return false }
        let moved = run
        moved.page = back; moved.folder = folder; moved.chat = id
        runs[id] = moved
        run = CoderRun(page: page, folder: folder); wire(run)
        chat = nil; recallDue = false; cd_current(page, "")
        configureWorkerEnv(run, isForeman: false)
        syncSkillsNote(for: page, isForeman: false)
        cd_chats(page, chatLines); syncForemanUI()
        wake()
        return true
    }
    /// A reply out of view ended: its chat is kept, its workers stop and its page goes.
    private func finish(_ id: String) {
        guard let back = runs.removeValue(forKey: id) else { return }
        let cost = Int(cd_cost_usd_micros(back.page))
        back.stop(); cd_close(back.page)
        _ = foremanEvents.emit(type: "chat.finished", chatID: id, data: [
            "title": store.title(id) ?? "Chat",
            "cost_usd_micros": cost
        ])
        if id != chat { unread.insert(id) }
        cd_chats(page, chatLines); syncForemanUI(); wake()
    }
    var canFolder: Bool { cd_running(page) == 0 }
    /// View > Bigger Text, Smaller Text, Actual Size: by is a row up or down, nil the usual size.
    func zoom(_ by: Int32?) {
        let row = by.map { cd_zoom(page, $0) } ?? cd_zoom_set(page, 2)
        UserDefaults.standard.set(Int(row), forKey: "zoom")
        var style = [Float](repeating: 0, count: 14); _ = cd_style(page, &style)
        line = CGFloat(style[1])
        if let font = editor.font { editor.font = NSFont(descriptor: font.fontDescriptor, size: CGFloat(style[0])) }
        let paragraph = NSMutableParagraphStyle(); paragraph.minimumLineHeight = line; paragraph.maximumLineHeight = line
        editor.defaultParagraphStyle = paragraph
        editor.textStorage?.addAttribute(.paragraphStyle, value: paragraph, range: NSRange(location: 0, length: editor.textStorage?.length ?? 0))
        editor.typingAttributes[.paragraphStyle] = paragraph
        wake()
    }
    /// File > New Chat (Command-N): what the New chat button does.
    /// The working folder: commands start there. The corner shows it with ~ for the home folder.
    private(set) var folder = FileManager.default.homeDirectoryForCurrentUser
    func setFolder(_ url: URL, keep: Bool) {
        folder = url.standardizedFileURL
        if keep { UserDefaults.standard.set(folder.path, forKey: "folder") }
        let home = FileManager.default.homeDirectoryForCurrentUser.path, path = folder.path
        cd_folder(page, path == home ? "~" : path.hasPrefix(home + "/") ? "~" + path.dropFirst(home.count) : path)
        wake()
    }
    private struct BinaryIdentity: Equatable {
        let path: String
        let dev: dev_t
        let ino: ino_t
    }
    /// The running executable's path and inode on disk, or nil if the bundle was moved or deleted.
    /// macOS's out-of-process NSOpenPanel (openAndSavePanelService) validates the host binary on disk via
    /// SecCodeCopyGuestWithAttributes; if the bundle was replaced or removed while running, ViewBridge faults
    /// and blocks the main thread for ~10s (spinning beachball) before failing to display the panel.
    private static func currentBinaryIdentity() -> BinaryIdentity? {
        var buf = [CChar](repeating: 0, count: Int(MAXPATHLEN) * 4)
        let n = proc_pidpath(getpid(), &buf, UInt32(buf.count))
        guard n > 0 else { return nil }
        let path = String(cString: buf)
        var st = stat()
        guard stat(path, &st) == 0 else { return nil }
        return BinaryIdentity(path: path, dev: st.st_dev, ino: st.st_ino)
    }
    private func canPresentOpenPanel() -> Bool {
        guard let initial = launchBinary, let current = CoderWindow.currentBinaryIdentity() else { return false }
        return initial == current
    }
    private func chooseFolderFallback() {
        let alert = NSAlert()
        alert.messageText = "Choose Working Folder"
        alert.informativeText = "Enter the path to the working folder (commands start there). Restart Coder to restore the Finder folder picker after an app update."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 360, height: 24))
        field.stringValue = folder.path
        field.placeholderString = FileManager.default.homeDirectoryForCurrentUser.path
        alert.accessoryView = field
        alert.addButton(withTitle: "Choose")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            self.choosingFolder = false
            defer { self.window.makeFirstResponder(self.editor) }
            guard answer == .alertFirstButtonReturn else { return }
            let raw = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !raw.isEmpty else { return }
            let expanded = (raw as NSString).expandingTildeInPath
            var isDir: ObjCBool = false
            if FileManager.default.fileExists(atPath: expanded, isDirectory: &isDir), isDir.boolValue {
                self.setFolder(URL(fileURLWithPath: expanded, isDirectory: true), keep: true)
            }
        }
    }
    func chooseFolder() {
        guard cd_running(page) == 0, !choosingFolder, window.attachedSheet == nil else { return }
        choosingFolder = true
        // Defer presentation out of the 60 Hz .common RunLoop timer tick.
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            guard cd_running(self.page) == 0, self.window.attachedSheet == nil else { self.choosingFolder = false; return }
            guard self.canPresentOpenPanel() else { self.chooseFolderFallback(); return }
            let panel = NSOpenPanel()
            panel.canChooseDirectories = true; panel.canChooseFiles = false; panel.allowsMultipleSelection = false; panel.canCreateDirectories = true
            var isDir: ObjCBool = false
            panel.directoryURL = (FileManager.default.fileExists(atPath: self.folder.path, isDirectory: &isDir) && isDir.boolValue)
                ? self.folder : FileManager.default.homeDirectoryForCurrentUser
            panel.prompt = "Choose"; panel.message = "Choose the working folder. Commands start there."
            panel.beginSheetModal(for: self.window) { [weak self] answer in
                guard let self else { return }
                self.choosingFolder = false
                if answer == .OK, let url = panel.url { self.setFolder(url, keep: true) }
                self.window.makeFirstResponder(self.editor)
            }
        }
    }
    func newChat() { if cd_new(page) != 0 { window.makeFirstResponder(editor) }; wake() }

    private func act(_ what: Int32) {
        var out = [CChar](repeating: 0, count: 262144)
        switch what {
        case 1:
            editor.synchronize { editor.string = "" }
            configureWorkerEnv(run, isForeman: isForemanShown)
            syncSkillsNote(for: page, isForeman: isForemanShown)
            guard run.worker.start() else { cd_failed(page, "Microcoder could not start."); return }
            if recallDue {
                // an opened chat's turns, or nothing to remember when it has none that ended
                recallDue = false
                var n = cd_recall_line(page, &out, Int32(out.count))
                if n == 0 { n = cd_forget_line(&out, Int32(out.count)) }
                run.worker.write(out, count: Int(n))
            }
            let n = cd_task(page, run.session, folder.path, &out, Int32(out.count))
            run.worker.write(out, count: Int(n))
        case 2: window.makeFirstResponder(editor)
        // The line is written into out first: Swift copies an argument before a later one's call fills it.
        case 3: stopRun(run)
        case 4:
            // a new chat: one whose reply is running goes on out of view
            guard detach() else { return }
            chat = nil; recallDue = false; cd_current(page, "")
            configureWorkerEnv(run, isForeman: false)
            syncSkillsNote(for: page, isForeman: false)
            syncForemanUI()
            let n = cd_forget_line(&out, Int32(out.count)); run.worker.write(out, count: Int(n))
            window.makeFirstResponder(editor)
        case 5:
            out = [CChar](repeating: 0, count: 2_000_001)   // a whole chat fits: 24 messages and replies
            guard cd_copied(page, &out, 2_000_000) > 0 else { return }
            let board = NSPasteboard.general; board.clearContents(); board.setString(String(cString: out), forType: .string)
        case 6: chooseFolder()
        case 7:
            // a press on a chat in the sidebar: its kept text becomes the chat that is shown
            guard cd_open_id(page, &out, Int32(out.count)) > 0 else { return }
            open(String(cString: out))
        case 8: editBudget()
        case 9: editGoal()
        case 10:
            var to: Int32 = -1
            guard cd_pin_move(page, &out, Int32(out.count), &to) > 0, to >= 0 else { return }
            store.movePinned(String(cString: out), to: Int(to))
            cd_chats(page, chatLines)
        case 11: openForeman()
        case 12: handleUpdatePillClick()
        default: break
        }
    }

    /// Opens the Nth chat from the top of the sidebar list (1 is the first).
    func openChat(at index: Int) {
        let ordered = store.orderedList
        guard index >= 1, index <= ordered.count else { return }
        open(ordered[index - 1].id)
    }
    /// Opens a kept chat: it becomes the chat that is shown, with the working folder it had. A reply
    /// running in the shown chat goes on out of view; a chat whose reply runs out of view comes back
    /// with its run.
    private func open(_ id: String) {
        if id == Foreman.chatID { openForeman(); return }
        guard id != chat, detach() else { return }
        if let back = runs[id] {
            guard cd_take(page, back.page) != 0 else { return }
            runs[id] = nil; cd_close(back.page); back.page = page
            run.stop()   // the window's idle worker: a message in a chat opened later starts another
            run = back
            unread.remove(id)
            chat = id; recallDue = false; cd_current(page, id)
            setFolder(back.folder, keep: false)
            cd_chats(page, chatLines)
            window.makeFirstResponder(editor); wake()
            return
        }
        guard let text = store.text(id), cd_load(page, text) != 0 else {
            let failed = NSAlert()
            failed.messageText = "This chat could not be opened."
            failed.informativeText = "“" + (store.title(id) ?? "Chat") + "” could not be read."
            failed.beginSheetModal(for: window)
            return
        }
        let clearedUnread = unread.remove(id) != nil
        chat = id; recallDue = true; cd_current(page, id)
        configureWorkerEnv(run, isForeman: false)
        syncSkillsNote(for: page, isForeman: false)
        syncForemanUI()
        if clearedUnread { cd_chats(page, chatLines) }
        var isFolder: ObjCBool = false
        if let kept = store.folder(id), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue { setFolder(URL(fileURLWithPath: kept), keep: false) }
        window.makeFirstResponder(editor)
        wake()
    }
    /// The chat that is shown, when it is kept: File > Rename Chat acts on it.
    var canRename: Bool { chat != nil && chat != Foreman.chatID }
    func renameShown() { if let id = chat, id != Foreman.chatID { rename(id) } }
    /// The chat that is shown, when it is kept and no reply is running: File > Delete Chat acts on it.
    var canDelete: Bool { chat != nil && chat != Foreman.chatID && cd_running(page) == 0 }
    func deleteShown() { if let id = chat { delete(id) } }
    /// Asks for a new max budget (USD) for the shown chat thread.
    func editBudget() {
        let currentCost = Double(cd_cost_usd_micros(page)) / 1_000_000.0
        let currentBudget = Double(cd_budget_usd_micros(page)) / 1_000_000.0
        let alert = NSAlert()
        alert.messageText = "Chat Max Budget"
        alert.informativeText = String(format: "Set the maximum spend (USD) for this chat thread. Current spend: $%.2f.", currentCost)
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 220, height: 24))
        field.stringValue = String(format: "%.2f", currentBudget)
        field.placeholderString = "50.00"
        alert.accessoryView = field
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.window.makeFirstResponder(self.editor) }
            guard answer == .alertFirstButtonReturn else { return }
            let cleaned = field.stringValue
                .trimmingCharacters(in: .whitespacesAndNewlines)
                .replacingOccurrences(of: "$", with: "")
                .replacingOccurrences(of: ",", with: "")
            guard let dollars = Double(cleaned), dollars.isFinite, dollars > 0, dollars <= 1_000_000 else { return }
            let micros = Int64((dollars * 1_000_000.0).rounded())
            guard micros > 0 else { return }
            _ = cd_set_budget_usd_micros(self.page, micros)
            self.wake()
        }
    }
    /// Opens the thread goal editor (/goal or /goal edit) to create, edit, pause/resume, or clear a thread goal.
    func editGoal() {
        let status = cd_goal_status(page)
        let elapsed = cd_goal_time(page)
        var buf = [CChar](repeating: 0, count: 4097)
        let n = cd_goal_objective(page, &buf, Int32(buf.count))
        let current = n > 0 ? String(cString: buf) : ""
        let alert = NSAlert()
        alert.messageText = status == 0 ? "Set Thread Goal" : "Edit Thread Goal"
        if status == 0 {
            alert.informativeText = "Describe a long-running objective. Coder will work autonomously across turns until it is complete, paused, or hits the chat budget."
        } else {
            let label: String
            switch status {
            case 1: label = "Active"
            case 2: label = "Paused"
            case 3: label = "Complete"
            case 4: label = "Blocked"
            case 5: label = "Budget limit"
            default: label = "None"
            }
            let timeStr = elapsed < 60 ? "\(elapsed)s" : "\(elapsed / 60)m"
            alert.informativeText = "Status: \(label) · Elapsed: \(timeStr). Update the objective below or change the goal state."
        }
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 420, height: 110))
        scroll.hasVerticalScroller = true
        scroll.borderType = .bezelBorder
        let tv = NSTextView(frame: NSRect(x: 0, y: 0, width: 420, height: 110))
        tv.isRichText = false
        tv.isAutomaticQuoteSubstitutionEnabled = false
        tv.font = NSFont.systemFont(ofSize: 13)
        tv.string = current
        scroll.documentView = tv
        alert.accessoryView = scroll
        alert.addButton(withTitle: status == 0 ? "Start Goal" : "Save Goal")
        if status != 0 {
            alert.addButton(withTitle: status == 1 ? "Pause" : "Resume")
            alert.addButton(withTitle: "Clear")
        }
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = tv
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.window.makeFirstResponder(self.editor) }
            if answer == .alertFirstButtonReturn {
                let text = tv.string.trimmingCharacters(in: .whitespacesAndNewlines)
                if !text.isEmpty { _ = cd_goal_set(self.page, text) }
            } else if status != 0 && answer == .alertSecondButtonReturn {
                _ = cd_goal_set_status(self.page, status == 1 ? 2 : 1)
            } else if status != 0 && answer == .alertThirdButtonReturn {
                _ = cd_goal_clear(self.page)
            }
            self.wake()
        }
    }
    /// Asks for a new title, then renames a kept chat.
    func rename(_ id: String) {
        guard let current = store.title(id) else { return }
        let alert = NSAlert()
        alert.messageText = "Rename Chat"
        alert.informativeText = "Enter a new title for this chat."
        let field = NSTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        field.stringValue = current
        field.placeholderString = "Chat title"
        alert.accessoryView = field
        alert.addButton(withTitle: "Rename")
        alert.addButton(withTitle: "Cancel")
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.window.makeFirstResponder(self.editor) }
            guard answer == .alertFirstButtonReturn else { return }
            let title = CoderStore.cleanTitle(field.stringValue)
            guard !title.isEmpty else { return }
            self.store.rename(id, to: title)
            self.daemonClient.updateSessionMeta(sessionID: id, title: title)
            cd_chats(self.page, self.chatLines)
            self.wake()
        }
    }
    /// Whether a reply is running in a chat, shown or out of view: it can't be deleted then.
    private func busy(_ id: String) -> Bool { runs[id] != nil || (id == chat && cd_running(page) != 0) }
    /// Asks, then deletes a kept chat. Deleting the one that is shown starts a new chat. Not while a
    /// reply is running in it.
    func delete(_ id: String) {
        if busy(id) { return }
        let alert = NSAlert()
        alert.messageText = "Delete this chat?"
        alert.informativeText = "“" + (store.title(id) ?? "New chat") + "” will be deleted from this Mac. You can't undo this."
        alert.addButton(withTitle: "Delete").hasDestructiveAction = true
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self, answer == .alertFirstButtonReturn else { return }
            if id == self.chat { guard cd_running(self.page) == 0 else { return }; self.chat = nil; _ = cd_new(self.page) }
            guard !self.busy(id) else { return }
            self.unread.remove(id)
            self.store.remove(id)
            self.daemonClient.updateSessionMeta(sessionID: id, archived: true)
            cd_chats(self.page, self.chatLines)
            self.window.makeFirstResponder(self.editor); self.wake()
        }
    }
    /// Coder > Claude Key: sets, replaces or removes the key Claude sub-agents use. It applies to the
    /// next Claude sub-agent; one already running keeps what it started with.
    func chooseKey() {
        let alert = NSAlert()
        alert.messageText = "Claude Key"
        var about = "Claude sub-agents use this key instead of the Claude Code login on this Mac. Paste an Anthropic API key (sk-ant-api…) or a token from claude setup-token (sk-ant-oat…). Coder keeps it in a file only you can read."
        if let launch = CoderKey.fromLaunch { about += "\n\nThis launch uses the key in CODER_CLAUDE_KEY (" + CoderKey.tail(launch) + "). It wins over a key set here until you open Coder without it." }
        else if let kept = CoderKey.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + "." }
        else { about += "\n\nNo key is set now: Claude sub-agents use the Claude Code login." }
        alert.informativeText = about
        let field = NSSecureTextField(frame: NSRect(x: 0, y: 0, width: 320, height: 24))
        field.placeholderString = "sk-ant-…"
        alert.accessoryView = field
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        let hadKey = CoderKey.saved != nil
        if hadKey { alert.addButton(withTitle: "Remove Key").hasDestructiveAction = true }
        alert.window.initialFirstResponder = field
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            defer { self.window.makeFirstResponder(self.editor) }
            let key: String
            switch answer {
            case .alertFirstButtonReturn:
                key = field.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
                if key.isEmpty { return }
            case .alertThirdButtonReturn where hadKey: key = ""
            default: return
            }
            if !CoderKey.save(key) {
                let failed = NSAlert(); failed.messageText = "The key could not be saved."
                failed.informativeText = "Coder could not write " + CoderKey.file.path + "."
                failed.beginSheetModal(for: self.window)
            }
        }
    }
    /// Coder > OpenRouter Models: the key, the model for Microcoder (a composer picker entry) and the
    /// model for sub-agents (level openrouter). An empty model removes it. Each model is checked against
    /// OpenRouter's list first: it must take a strict JSON schema.
    func chooseOpenRouter() {
        let alert = NSAlert()
        alert.messageText = "OpenRouter Models"
        var about = "Run Microcoder or its sub-agents on any OpenRouter model that takes a strict JSON schema. Use OpenRouter ids such as deepseek/deepseek-v4.1-flash-1017. The main model joins the composer's model picker. The sub-agent model is one more level Microcoder can hand tasks to: ask for OpenRouter by name."
        if let kept = CoderOpenRouter.saved { about += "\n\nThe key set now ends " + CoderKey.tail(kept) + ". Leave the key empty to keep it." }
        else if CoderOpenRouter.hasKey { about += "\n\nNo key is set here: Coder uses ~/work/.secrets/openrouter.env." }
        else { about += "\n\nNo key is set. Paste one from openrouter.ai/settings/keys." }
        alert.informativeText = about
        // Three labeled rows at fixed frames: an alert's accessory view gets no Auto Layout width,
        // so a stack view there shrinks its fields to slivers.
        let width: CGFloat = 340, rowH: CGFloat = 46
        let box = NSView(frame: NSRect(x: 0, y: 0, width: width, height: rowH * 3))
        func field(_ row: Int, _ title: String, _ secure: Bool, _ value: String, _ hint: String) -> NSTextField {
            let top = rowH * CGFloat(2 - row)
            let label = NSTextField(labelWithString: title)
            label.font = NSFont.systemFont(ofSize: NSFont.smallSystemFontSize)
            label.frame = NSRect(x: 0, y: top + 26, width: width, height: 16)
            let f = secure ? NSSecureTextField(frame: NSRect(x: 0, y: top + 2, width: width, height: 22)) : NSTextField(frame: NSRect(x: 0, y: top + 2, width: width, height: 22))
            f.stringValue = value; f.placeholderString = hint
            box.addSubview(label); box.addSubview(f)
            return f
        }
        let key = field(0, "Key", true, "", CoderOpenRouter.saved != nil ? "Leave empty to keep the key set now" : "sk-or-…")
        let main = field(1, "Model for Microcoder", false, CoderOpenRouter.main, "deepseek/deepseek-v4.1-flash-1017 (empty: none)")
        let sub = field(2, "Model for sub-agents", false, CoderOpenRouter.sub, "deepseek/deepseek-v4.1-flash-1017 (empty: none)")
        alert.accessoryView = box
        alert.addButton(withTitle: "Save")
        alert.addButton(withTitle: "Cancel")
        let hadKey = CoderOpenRouter.saved != nil
        if hadKey { alert.addButton(withTitle: "Remove Key").hasDestructiveAction = true }
        alert.window.initialFirstResponder = hadKey ? main : key
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            if answer == .alertThirdButtonReturn && hadKey { _ = CoderOpenRouter.save(""); self.window.makeFirstResponder(self.editor); return }
            guard answer == .alertFirstButtonReturn else { self.window.makeFirstResponder(self.editor); return }
            let k = key.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            let m = main.stringValue.trimmingCharacters(in: .whitespacesAndNewlines), s = sub.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            if !k.isEmpty && !CoderOpenRouter.save(k) {
                let failed = NSAlert(); failed.messageText = "The key could not be saved."
                failed.informativeText = "Coder could not write " + CoderOpenRouter.file.path + "."
                failed.beginSheetModal(for: self.window); return
            }
            CoderOpenRouter.unfit([m, s]) { bad in
                if let bad, !bad.isEmpty {
                    let failed = NSAlert(); failed.messageText = "OpenRouter has no model " + bad.joined(separator: " or ") + " that takes a strict JSON schema."
                    failed.informativeText = "Microcoder sends one on every step. Check the id on openrouter.ai/models. The models were not changed."
                    failed.beginSheetModal(for: self.window); return
                }
                CoderOpenRouter.setModels(main: m, sub: s)
                self.applyOpenRouter()
                self.window.makeFirstResponder(self.editor)
            }
        }
    }
    /// The OpenRouter models to the window's page and every page running out of view.
    func applyOpenRouter() {
        coderApplyOpenRouter(page)
        for (_, r) in runs { coderApplyOpenRouter(r.page) }
        wake()
    }
    @objc private func togglePinFromMenu(_ item: NSMenuItem) {
        guard let id = item.representedObject as? String else { return }
        store.togglePin(id)
        daemonClient.updateSessionMeta(sessionID: id, pinned: store.isPinned(id))
        cd_chats(page, chatLines)
        wake()
    }
    @objc private func renameFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { rename(id) } }
    @objc private func deleteFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { delete(id) } }
    @objc private func openForemanFromMenu(_ item: NSMenuItem) { openForeman() }
    @objc private func setForemanTierFromMenu(_ item: NSMenuItem) {
        if let tier = ForemanTier(rawValue: item.tag) { setForemanTier(tier) }
    }
    @objc private func stopAllFromMenu(_ item: NSMenuItem) { stopAllChats() }
    @objc private func viewAuditFromMenu(_ item: NSMenuItem) { openForemanAuditLog() }
    @objc private func clearForemanFromMenu(_ item: NSMenuItem) { clearForemanHistory() }
    /// A right-click on the Foreman row or a chat in the sidebar.
    private func rightClick(_ event: NSEvent) -> Bool {
        guard event.window === window else { return false }
        let at = canvas.convert(event.locationInWindow, from: nil), scale = window.backingScaleFactor
        let y = canvas.isFlipped ? at.y : canvas.bounds.height - at.y
        let px = Float(at.x * scale), py = Float(y * scale)
        if cd_foreman_at(page, px, py) != 0 {
            let menu = NSMenu()
            let openItem = menu.addItem(withTitle: "Open Foreman", action: #selector(openForemanFromMenu(_:)), keyEquivalent: "")
            openItem.target = self
            menu.addItem(.separator())
            for (tag, label) in [(0, "Tier 0: Observe (Read-Only)"), (1, "Tier 1: Managed (Default)"), (2, "Tier 2: Autonomous")] {
                let ti = menu.addItem(withTitle: label, action: #selector(setForemanTierFromMenu(_:)), keyEquivalent: "")
                ti.target = self; ti.tag = tag
                ti.state = foremanTier.rawValue == tag ? .on : .off
            }
            menu.addItem(.separator())
            let auditItem = menu.addItem(withTitle: "View Audit Log…", action: #selector(viewAuditFromMenu(_:)), keyEquivalent: "")
            auditItem.target = self
            let stopAll = menu.addItem(withTitle: "Stop All Running Chats", action: #selector(stopAllFromMenu(_:)), keyEquivalent: "")
            stopAll.target = self; stopAll.isEnabled = runningCount > 0
            menu.addItem(.separator())
            let clearItem = menu.addItem(withTitle: "Clear Foreman History…", action: #selector(clearForemanFromMenu(_:)), keyEquivalent: "")
            clearItem.target = self
            menu.autoenablesItems = false
            NSMenu.popUpContextMenu(menu, with: event, for: canvas)
            return true
        }
        var out = [CChar](repeating: 0, count: 128)
        guard cd_chat_at(page, px, py, &out, Int32(out.count)) > 0 else { return false }
        let id = String(cString: out), menu = NSMenu()
        let pinItem = menu.addItem(withTitle: store.isPinned(id) ? "Unpin Chat" : "Pin Chat", action: #selector(togglePinFromMenu(_:)), keyEquivalent: "")
        pinItem.target = self; pinItem.representedObject = id
        let renameItem = menu.addItem(withTitle: "Rename Chat…", action: #selector(renameFromMenu(_:)), keyEquivalent: "")
        renameItem.target = self; renameItem.representedObject = id
        menu.addItem(.separator())
        let deleteItem = menu.addItem(withTitle: "Delete Chat…", action: #selector(deleteFromMenu(_:)), keyEquivalent: "")
        deleteItem.target = self; deleteItem.representedObject = id
        deleteItem.isEnabled = !busy(id)
        menu.autoenablesItems = false
        NSMenu.popUpContextMenu(menu, with: event, for: canvas)
        return true
    }
    private var monitor: Any?

    /// Keeps a chat: its file, and its entry in the list (at the top when new, in place when updated).
    /// The shown one, or one whose reply runs out of view.
    private func keep(_ which: CoderRun? = nil) {
        let r = which ?? run, shown = r === run
        let id = (shown ? chat : r.chat) ?? UUID().uuidString.lowercased()
        var out = [CChar](repeating: 0, count: 32_000_000)   // 24 messages with every row and every sub-agent's rows full fit
        guard cd_save(r.page, &out, Int32(out.count)) > 0 else { return }
        let text = String(cString: out)
        var name = [CChar](repeating: 0, count: 256)
        let title = cd_title(r.page, &name, Int32(name.count)) > 0 ? String(cString: name) : "New chat"
        store.keep(id: id, title: title, folder: (shown ? folder : r.folder).path, text: text)
        if shown { chat = id; cd_current(page, id) } else { r.chat = id; if id != chat { unread.insert(id) } }
        cd_chats(page, chatLines); syncForemanUI()
    }

    private func tick() {
        // replies out of view: their seconds pass, they are kept as they change, and they end
        let now = CACurrentMediaTime(), dt = Float(now - last); last = now
        for (id, back) in runs {
            cd_tick(back.page, dt)
            let bgAct = cd_action(back.page)
            if bgAct == 1 {
                var out = [CChar](repeating: 0, count: 262144)
                if back.worker.start() {
                    let n = cd_task(back.page, back.session, back.folder.path, &out, Int32(out.count))
                    back.worker.write(out, count: Int(n))
                } else {
                    cd_failed(back.page, "Microcoder could not start.")
                }
            } else if bgAct == 3 {
                stopRun(back)
            }
            if cd_save_due(back.page) != 0 { keep(back) }
            if cd_running(back.page) == 0 { finish(id) }
        }
        let what = cd_action(page)
        if what != 0 { act(what); wake() }
        let fgRunning = cd_running(page) != 0
        if fgRunning != lastForegroundRunning {
            lastForegroundRunning = fgRunning
            if let cid = chat {
                _ = foremanEvents.emit(type: fgRunning ? "chat.started" : "chat.finished", chatID: cid, data: [
                    "title": store.title(cid) ?? (cid == Foreman.chatID ? Foreman.title : "Chat"),
                    "cost_usd_micros": Int(cd_cost_usd_micros(page))
                ])
            }
            syncForemanUI()
        }
        if let text = also, chat != nil, cd_running(page) != 0 { also = nil; newChat(); act(cd_action(page)); if cd_set_text(page, text) != 0 { _ = cd_submit(page) } }
        if let n = showAgent, cd_pane(page, Int32(n)) != 0 { showAgent = nil; wake() }
        if cd_save_due(page) != 0 { keep(); wake() }
        var sides = [Float](repeating: 0, count: 2)
        if cd_sides(page, &sides) != 0 { UserDefaults.standard.set(sides[0], forKey: "sideWidth"); UserDefaults.standard.set(sides[1], forKey: "paneWidth") }
        else if let text = then, cd_running(page) == 0 { then = nil; newChat(); act(cd_action(page)); if cd_set_text(page, text) != 0 { _ = cd_submit(page) } }
        // An empty draft leaves the text to the bar, so its placeholder shows under the caret.
        let focused = window.firstResponder === editor
        var used = line
        if let layout = editor.layoutManager, let container = editor.textContainer {
            layout.ensureLayout(for: container); used = max(line, layout.usedRect(for: container).height)
        }
        cd_env(page, focused && !editor.string.isEmpty ? 1 : 0, Float(used), NSWorkspace.shared.accessibilityDisplayShouldReduceMotion ? 1 : 0)
        if cd_moving(page) != 0 || owed > 0 { cd_frame(page); if owed > 0 { owed -= 1 } }
        var box = [Float](repeating: 0, count: 6)
        guard cd_editor(page, &box) != 0 else {
            if focused { restoreEditorFocus = true }
            editor.isHidden = true
            return
        }
        let wasHidden = editor.isHidden
        editor.isHidden = false
        if wasHidden && restoreEditorFocus {
            restoreEditorFocus = false
            window.makeFirstResponder(editor)
        }
        // The bar draws its first line box[4] under the text box's top: the editor's text starts there too.
        let inset = NSSize(width: 0, height: CGFloat(box[4]))
        if editor.textContainerInset != inset { editor.textContainerInset = inset; wake() }
        let frame = NSRect(x: CGFloat(box[0]), y: canvas.bounds.height - CGFloat(box[1] + box[3]), width: CGFloat(box[2]), height: CGFloat(box[3]))
        if editor.frame != frame { editor.frame = frame; wake() }
    }

    var runningCount: Int {
        (cd_running(page) != 0 ? 1 : 0) + runs.values.filter { cd_running($0.page) != 0 }.count
    }
    func show() { window.makeKeyAndOrderFront(nil); wake() }
    func windowShouldClose(_ sender: NSWindow) -> Bool { window.orderOut(nil); return false }

    // MARK: - Foreman Control Plane

    private func handleForemanAsyncRequest(_ request: ForemanRequest, done: @escaping (ForemanResponse) -> Void) {
        if request.method == "POST" && (request.path == "/v1/wait" || request.path == "/wait") {
            foremanWait(request.payload, done: done)
            return
        }
        let response = handleForemanRequest(request)
        done(response)
    }

    private func handleForemanRequest(_ request: ForemanRequest) -> ForemanResponse {
        let method = request.method
        let path = request.path
        let payload = request.payload
        let query = request.query
        switch (method, path) {
        case ("GET", "/v1/status"), ("GET", "/health"):
            let managedRunning = store.orderedList.filter { $0.managedBy == Foreman.managedTag && busy($0.id) }.count
            return ForemanResponse(status: 200, body: [
                "ok": true,
                "tier": foremanTier.wireName,
                "tier_level": foremanTier.rawValue,
                "tier_label": foremanTier.label,
                "workspace": folder.path,
                "chat_count": store.orderedList.count,
                "running_count": runningCount,
                "managed_running_count": managedRunning,
                "shown_chat": chat ?? "",
                "event_cursor": foremanEvents.latestID,
                "audit_log": foremanAudit.fileURL.path
            ])
        case ("GET", "/v1/tier"), ("GET", "/tier"):
            return ForemanResponse(status: 200, body: [
                "tier": foremanTier.wireName,
                "tier_level": foremanTier.rawValue,
                "label": foremanTier.label,
                "short_label": foremanTier.shortLabel
            ])
        case ("GET", "/v1/events"), ("POST", "/v1/events"):
            let since = Int(query["since"] ?? query["since_id"] ?? "") ?? (payload["since"] as? Int) ?? (payload["since_id"] as? Int) ?? 0
            let limit = max(1, min(200, Int(query["limit"] ?? "") ?? (payload["limit"] as? Int) ?? 50))
            let chatFilter = query["chat_id"] ?? (payload["chat_id"] as? String) ?? ""
            let chatIds: Set<String> = chatFilter.isEmpty ? [] : [chatFilter]
            let events = foremanEvents.recent(sinceId: since, chatIds: chatIds, limit: limit).map { $0.toDictionary() }
            return ForemanResponse(status: 200, body: [
                "cursor": foremanEvents.latestID,
                "events": events
            ])
        case ("GET", "/v1/audit"), ("POST", "/v1/audit"):
            let limit = max(1, min(200, Int(query["limit"] ?? "") ?? (payload["limit"] as? Int) ?? 50))
            return ForemanResponse(status: 200, body: [
                "path": foremanAudit.fileURL.path,
                "entries": foremanAudit.tail(limit: limit)
            ])
        case ("POST", "/v1/chats/list"), ("GET", "/v1/chats"), ("GET", "/chats"):
            var merged = payload
            if merged["filter"] == nil, let f = query["filter"] { merged["filter"] = f }
            if merged["limit"] == nil, let l = Int(query["limit"] ?? "") { merged["limit"] = l }
            return foremanListChats(merged)
        case ("POST", "/v1/chats/read"), ("POST", "/chats/read"):
            return foremanReadChat(payload)
        case ("POST", "/v1/chats/create"), ("POST", "/chats/create"):
            let resp = foremanCreateChat(payload)
            foremanAudit.record(action: "create_chat", tier: foremanTier, chatID: resp.body["chat_id"] as? String, status: resp.status, detail: [
                "title": resp.body["title"] ?? payload["title"] ?? "",
                "folder": resp.body["folder"] ?? payload["folder"] ?? ""
            ])
            return resp
        case ("POST", "/v1/chats/send"), ("POST", "/chats/send"):
            let resp = foremanSendChat(payload)
            foremanAudit.record(action: "send_chat", tier: foremanTier, chatID: payload["chat_id"] as? String, status: resp.status, detail: [
                "mode": payload["mode"] ?? "send"
            ])
            return resp
        case ("POST", "/v1/chats/control"), ("POST", "/chats/control"):
            let resp = foremanControlChat(payload)
            let act = (payload["action"] as? String) ?? ""
            if act != "open" {
                foremanAudit.record(action: "control:\(act)", tier: foremanTier, chatID: payload["chat_id"] as? String, status: resp.status, detail: payload)
            }
            return resp
        case ("POST", "/v1/chats/stop-all"), ("POST", "/chats/stop-all"):
            let resp = foremanStopAll(payload)
            foremanAudit.record(action: "stop_all", tier: foremanTier, chatID: nil, status: resp.status, detail: [
                "stopped": resp.body["stopped"] ?? []
            ])
            return resp
        case ("POST", "/v1/search/chats"), ("POST", "/search/chats"):
            return foremanSearchChats(payload)
        case ("POST", "/v1/search/repo"), ("POST", "/search/repo"):
            return foremanSearchRepo(payload)
        default:
            return ForemanResponse(status: 404, body: ["error": "unknown endpoint: \(method) \(path)"])
        }
    }

    private func foremanStopAll(_ payload: [String: Any]) -> ForemanResponse {
        if foremanTier == .observe {
            return ForemanResponse(status: 403, body: ["error": "Foreman is in Observe (read-only) tier."])
        }
        let stopAll = (payload["all"] as? Bool) ?? (payload["include_foreman"] as? Bool) ?? false
        var stopped: [String] = []
        if cd_running(page) != 0, let id = chat, id != Foreman.chatID {
            if foremanTier == .autonomous || store.isManagedByForeman(id) {
                stopRun(run)
                stopped.append(id)
                _ = foremanEvents.emit(type: "chat.stopped", chatID: id, data: ["title": store.title(id) ?? "Chat"])
            }
        }
        for id in Array(runs.keys) where id != Foreman.chatID {
            if !stopAll && foremanTier == .managed && !store.isManagedByForeman(id) { continue }
            if stopAll && foremanTier != .autonomous && !store.isManagedByForeman(id) { continue }
            if let back = runs.removeValue(forKey: id) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page)
                stopped.append(id)
                _ = foremanEvents.emit(type: "chat.stopped", chatID: id, data: ["title": store.title(id) ?? "Chat"])
            }
        }
        cd_chats(page, chatLines); syncForemanUI(); wake()
        return ForemanResponse(status: 200, body: ["ok": true, "stopped": stopped])
    }

    private func foremanWait(_ payload: [String: Any], done: @escaping (ForemanResponse) -> Void) {
        let mode = ((payload["mode"] as? String) ?? "any").lowercased()
        let timeoutSec = max(1.0, min(300.0, (payload["timeout_seconds"] as? Double) ?? Double((payload["timeout_seconds"] as? Int) ?? 60)))
        var targets: [String] = []
        if let ids = payload["chat_ids"] as? [String], !ids.isEmpty {
            targets = ids.filter { !$0.isEmpty && $0 != Foreman.chatID }
        } else if let single = payload["chat_id"] as? String, !single.isEmpty {
            targets = [single]
        } else {
            // Default to currently running managed chats, or any running non-Foreman chats.
            let runningManaged = store.orderedList.filter { $0.managedBy == Foreman.managedTag && busy($0.id) }.map { $0.id }
            targets = runningManaged.isEmpty ? store.orderedList.filter { busy($0.id) }.map { $0.id } : runningManaged
        }
        if targets.isEmpty {
            done(ForemanResponse(status: 200, body: [
                "ok": true,
                "timed_out": false,
                "completed": [],
                "still_running": [],
                "message": "no running chats to wait for"
            ]))
            return
        }
        let targetSet = Set(targets)
        let evaluate: () -> (satisfied: Bool, completed: [String], running: [String]) = { [weak self] in
            guard let self else { return (true, [], []) }
            let running = targets.filter { self.busy($0) }
            let completed = targets.filter { !self.busy($0) }
            let ok = mode == "all" ? running.isEmpty : !completed.isEmpty
            return (ok, completed, running)
        }
        let initial = evaluate()
        if initial.satisfied {
            done(ForemanResponse(status: 200, body: [
                "ok": true,
                "timed_out": false,
                "mode": mode,
                "completed": initial.completed,
                "still_running": initial.running
            ]))
            return
        }
        var finished = false
        var token: UUID?
        let finishWait: (Bool, ForemanEvent?) -> Void = { [weak self] timedOut, trigger in
            guard !finished else { return }
            finished = true
            if let t = token { self?.foremanEvents.unsubscribe(t); token = nil }
            let state = evaluate()
            var body: [String: Any] = [
                "ok": true,
                "timed_out": timedOut,
                "mode": mode,
                "completed": state.completed,
                "still_running": state.running
            ]
            if let trigger { body["event"] = trigger.toDictionary() }
            done(ForemanResponse(status: 200, body: body))
        }
        token = foremanEvents.subscribe { ev in
            guard ev.type == "chat.finished" || ev.type == "chat.stopped" || ev.type == "chat.deleted" else { return }
            if let cid = ev.chatID, !targetSet.contains(cid) { return }
            let state = evaluate()
            if state.satisfied {
                finishWait(false, ev)
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + timeoutSec) {
            finishWait(true, nil)
        }
    }

    private static func goalStatusName(_ status: Int32) -> String {
        switch status {
        case 1: return "active"
        case 2: return "paused"
        case 3: return "complete"
        case 4: return "blocked"
        case 5: return "budget_limited"
        default: return "none"
        }
    }

    private func foremanListChats(_ payload: [String: Any]) -> ForemanResponse {
        let limit = max(1, min(200, (payload["limit"] as? Int) ?? 50))
        let managedOnly = (payload["managed_only"] as? Bool) ?? false
        let filter = ((payload["filter"] as? String) ?? "all").lowercased()
        let folderFilter = (payload["folder"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let filtered = store.orderedList.filter { c in
            if managedOnly && c.managedBy != Foreman.managedTag { return false }
            if !folderFilter.isEmpty && !c.folder.localizedCaseInsensitiveContains(folderFilter) { return false }
            switch filter {
            case "running", "active": return busy(c.id)
            case "idle": return !busy(c.id)
            case "managed": return c.managedBy == Foreman.managedTag
            case "user": return c.managedBy != Foreman.managedTag
            case "pinned": return c.pinned != nil
            default: return true
            }
        }.prefix(limit)
        let items: [[String: Any]] = filtered.map { c in
            var item: [String: Any] = [
                "id": c.id,
                "title": c.title,
                "folder": c.folder,
                "updated": c.updated,
                "pinned": c.pinned != nil,
                "managed_by": c.managedBy ?? "",
                "status": busy(c.id) ? "running" : "idle",
                "shown": chat == c.id
            ]
            let livePage: UnsafeMutableRawPointer? = (c.id == chat) ? page : runs[c.id]?.page
            if let p = livePage {
                item["cost_usd_micros"] = Int(cd_cost_usd_micros(p))
                item["budget_usd_micros"] = Int(cd_budget_usd_micros(p))
                let gs = cd_goal_status(p)
                if gs != 0 {
                    item["goal_status"] = CoderWindow.goalStatusName(gs)
                    var buf = [CChar](repeating: 0, count: 512)
                    if cd_goal_objective(p, &buf, Int32(buf.count)) > 0 {
                        item["goal_objective"] = String(cString: buf)
                    }
                }
            }
            return item
        }
        return ForemanResponse(status: 200, body: [
            "tier": foremanTier.wireName,
            "workspace": folder.path,
            "running_count": runningCount,
            "chats": items
        ])
    }

    private func snapshotText(for id: String) -> String? {
        let targetPage: UnsafeMutableRawPointer?
        if id == chat { targetPage = page }
        else if let back = runs[id] { targetPage = back.page }
        else { targetPage = nil }
        if let p = targetPage {
            var out = [CChar](repeating: 0, count: 4_000_000)
            if cd_save(p, &out, Int32(out.count)) > 0 {
                return String(cString: out)
            }
        }
        return store.text(id)
    }

    private func foremanReadChat(_ payload: [String: Any]) -> ForemanResponse {
        guard let id = payload["chat_id"] as? String, !id.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "chat_id is required"])
        }
        let tail = max(1, min(50, (payload["tail"] as? Int) ?? 8))
        let includeTools = (payload["include_tools"] as? Bool) ?? false
        guard let raw = snapshotText(for: id), let data = raw.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
        }
        let meta = store.meta(id)
        let allTurns = (obj["turns"] as? [[String: Any]]) ?? []
        let slice = allTurns.suffix(tail).map { t -> [String: Any] in
            let user = (t["ask"] as? String) ?? (t["u"] as? String) ?? ""
            let reply = (t["reply"] as? String) ?? (t["r"] as? String) ?? ""
            let note = (t["note"] as? String) ?? (t["e"] as? String) ?? ""
            let state = (t["state"] as? Int) ?? 0
            let maxChars = 4000
            let clippedReply = reply.count > maxChars ? String(reply.prefix(maxChars)) + "…[truncated]" : reply
            var entry: [String: Any] = [
                "user": user,
                "reply": clippedReply,
                "note": note,
                "state": state
            ]
            if includeTools {
                if let cmds = t["cmds"] as? [[String: Any]] {
                    entry["tools"] = cmds.map { c -> [String: Any] in
                        let outStr = (c["out"] as? String) ?? ""
                        let clippedOut = outStr.count > 1000 ? String(outStr.prefix(1000)) + "…[truncated]" : outStr
                        return [
                            "text": (c["text"] as? String) ?? "",
                            "out": clippedOut,
                            "failed": (c["failed"] as? Bool) ?? false,
                            "took": (c["took"] as? Int) ?? 0
                        ]
                    }
                }
                if let agents = t["agents"] as? [[String: Any]], !agents.isEmpty {
                    entry["agents"] = agents.map { a -> [String: Any] in
                        [
                            "level": (a["level"] as? String) ?? "",
                            "task": (a["task"] as? String) ?? "",
                            "state": (a["state"] as? Int) ?? 0,
                            "calls": (a["calls"] as? Int) ?? 0
                        ]
                    }
                }
            }
            return entry
        }
        var body: [String: Any] = [
            "chat_id": id,
            "title": meta?.title ?? (id == Foreman.chatID ? Foreman.title : "Chat"),
            "folder": meta?.folder ?? folder.path,
            "managed_by": meta?.managedBy ?? "",
            "status": busy(id) ? "running" : "idle",
            "total_turns": allTurns.count,
            "turns": slice
        ]
        if let cost = obj["cost_usd_micros"] as? Int { body["cost_usd_micros"] = cost }
        if let budget = obj["budget_usd_micros"] as? Int { body["budget_usd_micros"] = budget }
        if let gs = obj["goal_status"] as? Int, gs != 0 {
            body["goal_status"] = CoderWindow.goalStatusName(Int32(gs))
            if let gobj = obj["goal_objective"] as? String { body["goal_objective"] = gobj }
        }
        return ForemanResponse(status: 200, body: body)
    }

    private func foremanCreateChat(_ payload: [String: Any]) -> ForemanResponse {
        if foremanTier == .observe {
            return ForemanResponse(status: 403, body: ["error": "Foreman is in Observe (read-only) tier. Switch to Managed or Autonomous tier to create chats."])
        }
        guard let prompt = payload["prompt"] as? String, !prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "prompt is required"])
        }
        let folderPath = (payload["folder"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? folder.path
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: folderPath, isDirectory: &isDir), isDir.boolValue else {
            return ForemanResponse(status: 400, body: ["error": "folder does not exist: \(folderPath)"])
        }
        guard let back = cd_headless() else {
            return ForemanResponse(status: 500, body: ["error": "could not allocate headless chat page"])
        }
        cd_skills(back, coderSkillsNote())
        cd_claude(back, CoderWorker.hasClaude ? 1 : 0)
        coderApplyOpenRouter(back)
        if let budgetUSD = payload["budget_usd"] as? Double, budgetUSD > 0 {
            _ = cd_set_budget_usd_micros(back, Int64((budgetUSD * 1_000_000.0).rounded()))
        }
        if let goalObj = (payload["goal"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines), !goalObj.isEmpty {
            _ = cd_goal_set(back, goalObj)
        }
        let id = UUID().uuidString.lowercased()
        let rawTitle = (payload["title"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let title = rawTitle.isEmpty ? String(prompt.prefix(60)) : rawTitle
        let folderURL = URL(fileURLWithPath: folderPath)
        let moved = CoderRun(page: back, folder: folderURL)
        moved.chat = id
        wire(moved)
        store.registerChat(id: id, title: title, folder: folderURL.path, managedBy: Foreman.managedTag)
        if (payload["pin"] as? Bool) == true && !store.isPinned(id) {
            store.togglePin(id)
        }
        let attributed = "[From Foreman]\n" + prompt
        guard cd_set_text(back, attributed) != 0, cd_submit(back) != 0 else {
            moved.stop(); cd_close(back)
            return ForemanResponse(status: 500, body: ["error": "failed to submit prompt to new chat"])
        }
        if cd_action(back) == 1 {
            var out = [CChar](repeating: 0, count: 262144)
            if moved.worker.start() {
                let n = cd_task(back, moved.session, moved.folder.path, &out, Int32(out.count))
                moved.worker.write(out, count: Int(n))
            } else {
                cd_failed(back, "Microcoder could not start.")
            }
        }
        runs[id] = moved
        keep(moved)
        _ = foremanEvents.emit(type: "chat.created", chatID: id, data: ["title": store.title(id) ?? title, "folder": folderURL.path])
        _ = foremanEvents.emit(type: "chat.started", chatID: id, data: ["title": store.title(id) ?? title])
        foremanAudit.record(action: "create_chat", tier: foremanTier, chatID: id, details: ["title": title, "folder": folderURL.path])
        if (payload["open"] as? Bool) == true { open(id) }
        cd_chats(page, chatLines); syncForemanUI(); wake()
        return ForemanResponse(status: 200, body: [
            "ok": true,
            "chat_id": id,
            "title": store.title(id) ?? title,
            "folder": folderURL.path,
            "managed_by": Foreman.managedTag,
            "pinned": store.isPinned(id),
            "status": "running"
        ])
    }

    private func checkMutationPermission(for id: String, action: String) -> ForemanResponse? {
        if id == Foreman.chatID {
            return ForemanResponse(status: 400, body: ["error": "cannot \(action) the Foreman chat itself"])
        }
        switch foremanTier {
        case .observe:
            return ForemanResponse(status: 403, body: ["error": "Foreman is in Observe (read-only) tier; \(action) is not allowed."])
        case .managed:
            if !store.isManagedByForeman(id) {
                return ForemanResponse(status: 403, body: ["error": "Chat \(id) is user-created. In Managed tier, Foreman can only \(action) Foreman-managed chats. Switch to Autonomous tier to control all chats."])
            }
            return nil
        case .autonomous:
            return nil
        }
    }

    private func foremanSendChat(_ payload: [String: Any]) -> ForemanResponse {
        guard let id = payload["chat_id"] as? String, !id.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "chat_id is required"])
        }
        let rawMsg = (payload["prompt"] as? String) ?? (payload["message"] as? String) ?? ""
        guard !rawMsg.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "prompt is required"])
        }
        let message = rawMsg
        if let denied = checkMutationPermission(for: id, action: "send to") { return denied }
        let mode = ((payload["mode"] as? String) ?? "follow_up").lowercased()
        let steer = ((payload["steer"] as? Bool) ?? false) || mode == "steer" || mode == "interrupt"
        if busy(id) {
            if !steer {
                return ForemanResponse(status: 400, body: ["error": "chat \(id) is currently running; wait, stop, or pass steer=true"])
            }
            if id == chat && cd_running(page) != 0 {
                stopRun(run)
            } else if let back = runs.removeValue(forKey: id) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page)
            }
        }
        let attributed = "[From Foreman]\n" + message
        if id == chat {
            guard cd_set_text(page, attributed) != 0, cd_submit(page) != 0 else {
                return ForemanResponse(status: 500, body: ["error": "failed to submit message to shown chat"])
            }
            foremanAudit.record(action: "send_chat", tier: foremanTier, chatID: id, details: ["steer": steer])
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "status": "running"])
        }
        guard let saved = store.text(id) else {
            return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
        }
        guard let back = cd_headless() else {
            return ForemanResponse(status: 500, body: ["error": "could not allocate headless chat page"])
        }
        cd_skills(back, coderSkillsNote())
        cd_claude(back, CoderWorker.hasClaude ? 1 : 0)
        coderApplyOpenRouter(back)
        guard cd_load(back, saved) != 0 else {
            cd_close(back)
            return ForemanResponse(status: 500, body: ["error": "could not load saved chat \(id)"])
        }
        let folderPath = store.folder(id) ?? folder.path
        let moved = CoderRun(page: back, folder: URL(fileURLWithPath: folderPath))
        moved.chat = id
        wire(moved)
        guard cd_set_text(back, attributed) != 0, cd_submit(back) != 0 else {
            moved.stop(); cd_close(back)
            return ForemanResponse(status: 500, body: ["error": "failed to submit follow-up message"])
        }
        if cd_action(back) == 1 {
            var out = [CChar](repeating: 0, count: 262144)
            if moved.worker.start() {
                var rn = cd_recall_line(back, &out, Int32(out.count))
                if rn == 0 { rn = cd_forget_line(&out, Int32(out.count)) }
                moved.worker.write(out, count: Int(rn))
                let tn = cd_task(back, moved.session, moved.folder.path, &out, Int32(out.count))
                moved.worker.write(out, count: Int(tn))
            } else {
                cd_failed(back, "Microcoder could not start.")
            }
        }
        runs[id] = moved
        keep(moved)
        _ = foremanEvents.emit(type: "chat.started", chatID: id, data: ["title": store.title(id) ?? "Chat"])
        foremanAudit.record(action: "send_chat", tier: foremanTier, chatID: id, details: ["steer": steer])
        cd_chats(page, chatLines); syncForemanUI(); wake()
        return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "status": "running"])
    }

    private func foremanControlChat(_ payload: [String: Any]) -> ForemanResponse {
        guard let id = payload["chat_id"] as? String, !id.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "chat_id is required"])
        }
        guard let action = payload["action"] as? String, !action.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "action is required"])
        }
        if action == "open" {
            open(id)
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "action": "open"])
        }
        if let denied = checkMutationPermission(for: id, action: action) { return denied }
        switch action {
        case "stop":
            if id == chat && cd_running(page) != 0 {
                stopRun(run)
            } else if let back = runs.removeValue(forKey: id) {
                stopRun(back); keep(back); back.stop(); cd_close(back.page)
            } else {
                return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "status": "already_idle"])
            }
            _ = foremanEvents.emit(type: "chat.stopped", chatID: id, data: ["title": store.title(id) ?? "Chat"])
            foremanAudit.record(action: "control_stop", tier: foremanTier, chatID: id)
            cd_chats(page, chatLines); syncForemanUI(); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "action": "stop", "status": "stopped"])
        case "pin":
            if !store.isPinned(id) { store.togglePin(id) }
            foremanAudit.record(action: "control_pin", tier: foremanTier, chatID: id)
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "pinned": true])
        case "unpin":
            if store.isPinned(id) { store.togglePin(id) }
            foremanAudit.record(action: "control_unpin", tier: foremanTier, chatID: id)
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "pinned": false])
        case "rename":
            guard let title = payload["title"] as? String, !CoderStore.cleanTitle(title).isEmpty else {
                return ForemanResponse(status: 400, body: ["error": "non-empty title is required for rename"])
            }
            let cleaned = CoderStore.cleanTitle(title)
            store.rename(id, to: cleaned)
            foremanAudit.record(action: "control_rename", tier: foremanTier, chatID: id, details: ["title": cleaned])
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "title": store.title(id) ?? cleaned])
        case "delete":
            if busy(id) {
                return ForemanResponse(status: 400, body: ["error": "cannot delete a running chat; stop it first"])
            }
            if id == chat { chat = nil; _ = cd_new(page); cd_current(page, "") }
            store.remove(id)
            _ = foremanEvents.emit(type: "chat.deleted", chatID: id)
            foremanAudit.record(action: "control_delete", tier: foremanTier, chatID: id)
            cd_chats(page, chatLines); wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "deleted": true])
        case "set_budget", "budget":
            let rawBudget = (payload["budget_usd"] as? Double) ?? (payload["budget_usd"] as? Int).map(Double.init)
            guard let budgetUSD = rawBudget, budgetUSD > 0 else {
                return ForemanResponse(status: 400, body: ["error": "positive budget_usd is required"])
            }
            let micros = Int64((budgetUSD * 1_000_000.0).rounded())
            if id == chat {
                _ = cd_set_budget_usd_micros(page, micros); keep()
            } else if let back = runs[id] {
                _ = cd_set_budget_usd_micros(back.page, micros); keep(back)
            } else if let saved = store.text(id), let tmp = cd_headless() {
                if cd_load(tmp, saved) != 0 {
                    _ = cd_set_budget_usd_micros(tmp, micros)
                    var buf = [CChar](repeating: 0, count: 4_000_000)
                    if cd_save(tmp, &buf, Int32(buf.count)) > 0 {
                        let dir = store.folder(id) ?? folder.path
                        store.keep(id: id, title: store.title(id) ?? "Chat", folder: dir, text: String(cString: buf))
                    }
                }
                cd_close(tmp)
            } else {
                return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
            }
            foremanAudit.record(action: "control_set_budget", tier: foremanTier, chatID: id, details: ["budget_usd": budgetUSD])
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "budget_usd": budgetUSD])
        case "set_goal", "goal", "pause_goal", "resume_goal", "resume", "clear_goal":
            let goalStatus = ((payload["goal_status"] as? String) ?? "").lowercased()
            let applyGoal: (UnsafeMutableRawPointer) -> Bool = { p in
                switch action {
                case "set_goal", "goal":
                    if goalStatus == "paused" || goalStatus == "pause" { return cd_goal_set_status(p, 2) != 0 }
                    if goalStatus == "active" || goalStatus == "resume" { return cd_goal_set_status(p, 1) != 0 }
                    if goalStatus == "clear" || goalStatus == "none" { return cd_goal_clear(p) != 0 }
                    let rawObj = (payload["objective"] as? String) ?? (payload["goal"] as? String) ?? ""
                    let obj = rawObj.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !obj.isEmpty else { return false }
                    return cd_goal_set(p, obj) != 0
                case "pause_goal": return cd_goal_set_status(p, 2) != 0
                case "resume_goal", "resume": return cd_goal_set_status(p, 1) != 0
                case "clear_goal": return cd_goal_clear(p) != 0
                default: return false
                }
            }
            var ok = false
            if id == chat {
                ok = applyGoal(page); if ok { keep() }
            } else if let back = runs[id] {
                ok = applyGoal(back.page); if ok { keep(back) }
            } else if let saved = store.text(id), let tmp = cd_headless() {
                if cd_load(tmp, saved) != 0 {
                    ok = applyGoal(tmp)
                    if ok {
                        var buf = [CChar](repeating: 0, count: 4_000_000)
                        if cd_save(tmp, &buf, Int32(buf.count)) > 0 {
                            let dir = store.folder(id) ?? folder.path
                            store.keep(id: id, title: store.title(id) ?? "Chat", folder: dir, text: String(cString: buf))
                        }
                    }
                }
                cd_close(tmp)
            } else {
                return ForemanResponse(status: 404, body: ["error": "chat not found: \(id)"])
            }
            if !ok { return ForemanResponse(status: 400, body: ["error": "failed to apply \(action)"]) }
            foremanAudit.record(action: "control_\(action)", tier: foremanTier, chatID: id)
            wake()
            return ForemanResponse(status: 200, body: ["ok": true, "chat_id": id, "action": action])
        default:
            return ForemanResponse(status: 400, body: ["error": "unknown action: \(action)"])
        }
    }

    private func foremanSearchChats(_ payload: [String: Any]) -> ForemanResponse {
        guard let query = payload["query"] as? String, !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "query is required"])
        }
        let limit = max(1, min(50, (payload["limit"] as? Int) ?? 20))
        let folderFilter = (payload["folder"] as? String)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let managedOnly = (payload["managed_only"] as? Bool) ?? false
        let df = DateFormatter()
        df.dateFormat = "yyyy-MM-dd"
        df.timeZone = TimeZone(secondsFromGMT: 0)
        let dateFromTs = (payload["date_from"] as? String).flatMap { df.date(from: $0)?.timeIntervalSince1970 } ?? 0
        let dateToTs = (payload["date_to"] as? String).flatMap { df.date(from: $0)?.timeIntervalSince1970 }.map { $0 + 86400 } ?? 0
        let since = max((payload["since"] as? Double) ?? (payload["since"] as? Int).map(Double.init) ?? 0, dateFromTs)
        let needle = query.lowercased()
        var results: [[String: Any]] = []
        for meta in store.orderedList {
            if results.count >= limit { break }
            if managedOnly && meta.managedBy != Foreman.managedTag { continue }
            if !folderFilter.isEmpty && !meta.folder.localizedCaseInsensitiveContains(folderFilter) { continue }
            if since > 0 && meta.updated < since { continue }
            if dateToTs > 0 && meta.updated > dateToTs { continue }
            var snippets: [String] = []
            if meta.title.lowercased().contains(needle) {
                snippets.append("title: " + meta.title)
            }
            if let raw = snapshotText(for: meta.id), let data = raw.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let turns = obj["turns"] as? [[String: Any]] {
                for (idx, t) in turns.enumerated() {
                    if snippets.count >= 3 { break }
                    let pairs: [(String, String?)] = [
                        ("user", (t["ask"] as? String) ?? (t["u"] as? String)),
                        ("reply", (t["reply"] as? String) ?? (t["r"] as? String))
                    ]
                    for (role, maybeText) in pairs {
                        guard let text = maybeText, let range = text.range(of: query, options: .caseInsensitive) else { continue }
                        let start = text.index(range.lowerBound, offsetBy: -60, limitedBy: text.startIndex) ?? text.startIndex
                        let end = text.index(range.upperBound, offsetBy: 120, limitedBy: text.endIndex) ?? text.endIndex
                        let excerpt = String(text[start..<end]).replacingOccurrences(of: "\n", with: " ")
                        snippets.append("turn \(idx + 1) (\(role)): " + excerpt)
                        if snippets.count >= 3 { break }
                    }
                }
            }
            if !snippets.isEmpty {
                results.append([
                    "chat_id": meta.id,
                    "title": meta.title,
                    "managed_by": meta.managedBy ?? "",
                    "folder": meta.folder,
                    "updated": meta.updated,
                    "snippets": snippets
                ])
            }
        }
        return ForemanResponse(status: 200, body: [
            "query": query,
            "total": results.count,
            "results": results
        ])
    }

    private func foremanSearchRepo(_ payload: [String: Any]) -> ForemanResponse {
        guard let query = payload["query"] as? String, !query.isEmpty else {
            return ForemanResponse(status: 400, body: ["error": "query is required"])
        }
        let limit = max(1, min(200, (payload["max"] as? Int) ?? (payload["limit"] as? Int) ?? 50))
        let rootPath: String
        if let sub = ((payload["folder"] as? String) ?? (payload["path"] as? String)), !sub.isEmpty {
            rootPath = sub.hasPrefix("/") ? sub : folder.appendingPathComponent(sub).path
        } else {
            rootPath = folder.path
        }
        let p = Process()
        let isGitRepo = FileManager.default.fileExists(atPath: URL(fileURLWithPath: rootPath).appendingPathComponent(".git").path) ||
                        FileManager.default.fileExists(atPath: folder.appendingPathComponent(".git").path)
        if isGitRepo {
            p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            var args = ["-C", rootPath, "grep", "-n", "-i", "-F", query]
            if let glob = payload["glob"] as? String, !glob.isEmpty { args.append(contentsOf: ["--", glob]) }
            p.arguments = args
        } else {
            p.executableURL = URL(fileURLWithPath: "/usr/bin/grep")
            p.arguments = ["-rnIiF", query, rootPath]
        }
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch {
            return ForemanResponse(status: 500, body: ["error": "search command failed: \(error.localizedDescription)"])
        }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        let text = String(data: data, encoding: .utf8) ?? ""
        var matches: [[String: Any]] = []
        for line in text.split(separator: "\n") {
            if matches.count >= limit { break }
            let parts = line.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            if parts.count >= 3 {
                matches.append([
                    "file": String(parts[0]),
                    "line": Int(parts[1]) ?? 0,
                    "text": String(parts[2]).trimmingCharacters(in: .whitespaces)
                ])
            }
        }
        return ForemanResponse(status: 200, body: [
            "workspace": rootPath,
            "query": query,
            "total": matches.count,
            "matches": matches
        ])
    }

    func retire() {
        updater.stop()
        foremanServer.stop()
        if chat != nil { keep() }
        for back in runs.values { keep(back); back.stop(); cd_close(back.page) }
        runs = [:]
        if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil; clock?.invalidate(); clock = nil
        run.stop(); cd_on_change(page, nil, nil); cd_close(page)
    }
}

final class CoderDelegate: NSObject, NSApplicationDelegate {
    var main: CoderWindow?
    private var statusItem: NSStatusItem?
    func applicationDidFinishLaunching(_ notification: Notification) {
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path, let main = CoderWindow(resources: resources) else {
            fputs("Coder: the GPU surface could not start.\n", stderr); exit(1)
        }
        self.main = main
        let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = statusItem.button {
            let image = NSImage(systemSymbolName: "terminal", accessibilityDescription: "Coder")
            image?.isTemplate = true
            button.image = image
        }
        let menu = NSMenu()
        let showItem = menu.addItem(withTitle: "Show Coder", action: #selector(showWindow), keyEquivalent: "")
        showItem.target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Coder", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        statusItem.menu = menu
        self.statusItem = statusItem
        NSApp.activate(ignoringOtherApps: true)
    }
    @objc private func showWindow() {
        NSApp.activate(ignoringOtherApps: true)
        main?.show()
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { main?.show() }
        return true
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let count = main?.runningCount, count > 0 else { return .terminateNow }
        NSApp.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = count == 1 ? "An agent task is still running" : "\(count) agent tasks are still running"
        alert.informativeText = "Quitting Coder will stop all running tasks. Close the window instead to keep them running in the background."
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Quit")
        return alert.runModal() == .alertSecondButtonReturn ? .terminateNow : .terminateCancel
    }
    func applicationWillTerminate(_ notification: Notification) { main?.retire() }
    @objc func newChat(_ sender: Any?) { main?.newChat() }
    @objc func openFolder(_ sender: Any?) { main?.chooseFolder() }
    @objc func renameChat(_ sender: Any?) { main?.renameShown() }
    @objc func deleteChat(_ sender: Any?) { main?.deleteShown() }
    @objc func biggerText(_ sender: Any?) { main?.zoom(1) }
    @objc func smallerText(_ sender: Any?) { main?.zoom(-1) }
    @objc func actualSize(_ sender: Any?) { main?.zoom(nil) }
    @objc func selectChat(_ sender: NSMenuItem) { main?.openChat(at: sender.tag) }
    @objc func openForeman(_ sender: Any?) { main?.openForeman() }
    @objc func setForemanTier(_ sender: NSMenuItem) {
        switch sender.tag {
        case 0: main?.setForemanTier(.observe)
        case 1: main?.setForemanTier(.managed)
        case 2: main?.setForemanTier(.autonomous)
        default: break
        }
    }
    @objc func stopAllChats(_ sender: Any?) { main?.stopAllChats() }
    @objc func viewForemanAuditLog(_ sender: Any?) { main?.openForemanAuditLog() }
    @objc func clearForemanHistory(_ sender: Any?) { main?.clearForemanHistory() }
    @objc func claudeKey(_ sender: Any?) { main?.chooseKey() }
    @objc func openRouterModels(_ sender: Any?) { main?.chooseOpenRouter() }
    @objc func checkForUpdates(_ sender: Any?) { main?.checkForUpdates() }
    // Open Folder is off while the shown chat's reply runs, as its button is. New Chat is always on:
    // a running reply goes on out of view.
    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(renameChat(_:)) { return main?.canRename == true }
        if item.action == #selector(deleteChat(_:)) { return main?.canDelete == true }
        if item.action == #selector(openFolder(_:)) { return main?.canFolder == true }
        if item.action == #selector(setForemanTier(_:)) {
            let current = main?.foremanTier ?? .managed
            let matches = (senderTag: item.tag, tier: current)
            switch matches.senderTag {
            case 0: item.state = matches.tier == .observe ? .on : .off
            case 1: item.state = matches.tier == .managed ? .on : .off
            case 2: item.state = matches.tier == .autonomous ? .on : .off
            default: item.state = .off
            }
            return true
        }
        if item.action == #selector(stopAllChats(_:)) { return (main?.runningCount ?? 0) > 0 }
        return true
    }
}

@main struct CoderApp {
    static func main() {
        let app = NSApplication.shared; app.setActivationPolicy(.regular)
        let delegate = CoderDelegate(); app.delegate = delegate
        let bar = NSMenu(), appItem = NSMenuItem(), editItem = NSMenuItem()
        let appMenu = NSMenu(), edit = NSMenu(title: "Edit")
        let fileItem = NSMenuItem(), file = NSMenu(title: "File")
        file.addItem(withTitle: "New Chat", action: #selector(CoderDelegate.newChat(_:)), keyEquivalent: "n").target = delegate
        file.addItem(withTitle: "Open Folder…", action: #selector(CoderDelegate.openFolder(_:)), keyEquivalent: "o").target = delegate
        file.addItem(.separator())
        file.addItem(withTitle: "Rename Chat…", action: #selector(CoderDelegate.renameChat(_:)), keyEquivalent: "").target = delegate
        let del = file.addItem(withTitle: "Delete Chat…", action: #selector(CoderDelegate.deleteChat(_:)), keyEquivalent: "\u{8}")
        del.target = delegate; del.keyEquivalentModifierMask = [.command]
        fileItem.submenu = file
        let viewItem = NSMenuItem(), view = NSMenu(title: "View")
        let actual = view.addItem(withTitle: "Actual Size", action: #selector(CoderDelegate.actualSize(_:)), keyEquivalent: "0")
        actual.target = delegate; actual.keyEquivalentModifierMask = [.command, .option]
        view.addItem(withTitle: "Bigger Text", action: #selector(CoderDelegate.biggerText(_:)), keyEquivalent: "+").target = delegate
        // Command-= is Command-plus with no Shift
        let plain = view.addItem(withTitle: "Bigger Text", action: #selector(CoderDelegate.biggerText(_:)), keyEquivalent: "=")
        plain.target = delegate; plain.isHidden = true; plain.allowsKeyEquivalentWhenHidden = true
        view.addItem(withTitle: "Smaller Text", action: #selector(CoderDelegate.smallerText(_:)), keyEquivalent: "-").target = delegate
        for k in 1...9 {
            let item = view.addItem(withTitle: "Chat \(k)", action: #selector(CoderDelegate.selectChat(_:)), keyEquivalent: "\(k)")
            item.target = delegate; item.tag = k; item.keyEquivalentModifierMask = [.command]
            item.isHidden = true; item.allowsKeyEquivalentWhenHidden = true
        }
        viewItem.submenu = view
        appMenu.addItem(withTitle: "Claude Key…", action: #selector(CoderDelegate.claudeKey(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(withTitle: "OpenRouter Models…", action: #selector(CoderDelegate.openRouterModels(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(withTitle: "Check for Updates…", action: #selector(CoderDelegate.checkForUpdates(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Hide Coder", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit Coder", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        let foremanItem = NSMenuItem(), foremanMenu = NSMenu(title: "Foreman")
        let openForemanItem = foremanMenu.addItem(withTitle: "Open Foreman", action: #selector(CoderDelegate.openForeman(_:)), keyEquivalent: "0")
        openForemanItem.target = delegate; openForemanItem.keyEquivalentModifierMask = [.command]
        foremanMenu.addItem(.separator())
        let t0 = foremanMenu.addItem(withTitle: "Tier 0: Observe (Read-Only)", action: #selector(CoderDelegate.setForemanTier(_:)), keyEquivalent: "")
        t0.target = delegate; t0.tag = 0
        let t1 = foremanMenu.addItem(withTitle: "Tier 1: Managed (Default)", action: #selector(CoderDelegate.setForemanTier(_:)), keyEquivalent: "")
        t1.target = delegate; t1.tag = 1
        let t2 = foremanMenu.addItem(withTitle: "Tier 2: Autonomous", action: #selector(CoderDelegate.setForemanTier(_:)), keyEquivalent: "")
        t2.target = delegate; t2.tag = 2
        foremanMenu.addItem(.separator())
        let stopAll = foremanMenu.addItem(withTitle: "Stop All Running Chats", action: #selector(CoderDelegate.stopAllChats(_:)), keyEquivalent: ".")
        stopAll.target = delegate; stopAll.keyEquivalentModifierMask = [.command, .shift]
        let auditLogItem = foremanMenu.addItem(withTitle: "View Audit Log…", action: #selector(CoderDelegate.viewForemanAuditLog(_:)), keyEquivalent: "")
        auditLogItem.target = delegate
        foremanMenu.addItem(.separator())
        let clearHistItem = foremanMenu.addItem(withTitle: "Clear Foreman History…", action: #selector(CoderDelegate.clearForemanHistory(_:)), keyEquivalent: "")
        clearHistItem.target = delegate
        foremanItem.submenu = foremanMenu
        appItem.submenu = appMenu; editItem.submenu = edit; bar.addItem(appItem); bar.addItem(fileItem); bar.addItem(editItem); bar.addItem(viewItem); bar.addItem(foremanItem)
        app.mainMenu = bar
        app.run()
    }
}
