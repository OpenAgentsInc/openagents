// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
import AppKit

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
struct CoderChat: Codable { var id: String; var title: String; var created: Double? = nil; var updated: Double; var folder: String; var renamed: Bool? = nil }
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
    /// The list as the page reads it: one chat a line, its id, a tab, its title.
    var lines: String { list.map { $0.id + "\t" + $0.title }.joined(separator: "\n") }
    func text(_ id: String) -> String? { try? String(contentsOf: dir.appendingPathComponent(id + ".json"), encoding: .utf8) }
    func folder(_ id: String) -> String? { list.first { $0.id == id }?.folder }
    func title(_ id: String) -> String? { list.first { $0.id == id }?.title }
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
        saveIndex()
    }
    /// Keeps a chat's text. A new chat joins the top of the list; an existing chat updates in place
    /// so chats stay ordered chronologically by creation time (newest created first).
    func keep(id: String, title: String, folder: String, text: String) {
        guard (try? text.write(to: dir.appendingPathComponent(id + ".json"), atomically: true, encoding: .utf8)) != nil else { return }
        let now = Date().timeIntervalSince1970
        if let i = list.firstIndex(where: { $0.id == id }) {
            if list[i].created == nil { list[i].created = list[i].updated }
            if list[i].renamed != true { list[i].title = title }
            list[i].updated = now
            list[i].folder = folder
        } else {
            list.insert(CoderChat(id: id, title: title, created: now, updated: now, folder: folder), at: 0)
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
    private var last = CACurrentMediaTime()      // when the clock last ticked, for their pages' seconds
    private var line: CGFloat
    private var clock: Timer?
    private var then: String?   // --then TEXT: sent in a new chat once --send's reply ends
    private var also: String?   // --new TEXT: sent in a new chat while --send's reply runs
    private var showAgent: Int?   // --agent N: the Nth sub-agent's transcript opens as soon as there is one
    private var owed = 3   // frames still to draw after a change
    private let store = CoderStore()
    private var chat: String?          // the id of the chat that is shown, once it is kept
    private var recallDue = false      // a chat was opened: the worker is told its turns ahead of the next message

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
        cd_chats(page, chatLines)
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
        // Coder --chat N opens the Nth most recent chat at launch (1 is the newest).
        if let at = CommandLine.arguments.firstIndex(of: "--chat"), at + 1 < CommandLine.arguments.count, let k = Int(CommandLine.arguments[at + 1]) { openChat(at: k) }
        // Coder --send TEXT sends TEXT at launch, for a check of the whole path with no typing.
        if let at = CommandLine.arguments.firstIndex(of: "--send"), at + 1 < CommandLine.arguments.count, cd_set_text(page, CommandLine.arguments[at + 1]) != 0, cd_submit(page) != 0,
           let next = CommandLine.arguments.firstIndex(of: "--then"), next + 1 < CommandLine.arguments.count { then = CommandLine.arguments[next + 1] }
        // Coder --send A --new B sends B in a new chat while A's reply runs, which goes on out of view.
        if let at = CommandLine.arguments.firstIndex(of: "--new"), at + 1 < CommandLine.arguments.count { also = CommandLine.arguments[at + 1] }
        // Coder --agent N opens the transcript of the Nth sub-agent of the newest message that has one.
        if let at = CommandLine.arguments.firstIndex(of: "--agent"), at + 1 < CommandLine.arguments.count { showAgent = Int(CommandLine.arguments[at + 1]) }
        tick()
    }

    func wake() { owed = 3 }

    /// A run's worker lines go to its page, wherever that page is now.
    private func wire(_ r: CoderRun) {
        r.worker.line = { [weak self, weak r] text in guard let self, let r else { return }; cd_line(r.page, text); self.pump(r) }
        r.worker.gone = { [weak self, weak r] in guard let self, let r else { return }; cd_failed(r.page, "Microcoder stopped running. Send the message again."); self.pump(r) }
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
                one.line = { [weak self, weak r] text in guard let self, let r, r.agents[j] === one else { return }; cd_agent_line(r.page, j, text); self.pump(r) }
                one.gone = { [weak self, weak r] in guard let self, let r, r.agents[j] === one else { return }; r.agents[j] = nil; cd_agent_failed(r.page, j, "The sub-agent's worker stopped running."); self.pump(r) }
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
    /// The list as the page reads it: one chat a line, its id, a tab, its title, and a tab and 1
    /// when its reply runs out of view.
    private var chatLines: String { store.list.map { $0.id + "\t" + $0.title + (runs[$0.id] != nil ? "\t1" : "") }.joined(separator: "\n") }
    /// The shown chat's reply goes on out of view: its run moves to a page with no window and the
    /// window gets a new run. True when nothing runs in the window now.
    @discardableResult private func detach() -> Bool {
        guard cd_running(page) != 0 else { return true }
        keep()
        guard let id = chat, let back = cd_headless() else { return false }
        guard cd_take(back, page) != 0 else { cd_close(back); return false }
        let moved = run
        moved.page = back; moved.folder = folder; moved.chat = id
        runs[id] = moved
        run = CoderRun(page: page, folder: folder); wire(run)
        chat = nil; recallDue = false; cd_current(page, "")
        cd_chats(page, chatLines)
        wake()
        return true
    }
    /// A reply out of view ended: its chat is kept, its workers stop and its page goes.
    private func finish(_ id: String) {
        guard let back = runs.removeValue(forKey: id) else { return }
        back.stop(); cd_close(back.page)
        cd_chats(page, chatLines); wake()
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
    func chooseFolder() {
        guard cd_running(page) == 0 else { return }
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true; panel.canChooseFiles = false; panel.allowsMultipleSelection = false; panel.canCreateDirectories = true
        panel.directoryURL = folder; panel.prompt = "Choose"; panel.message = "Choose the working folder. Commands start there."
        panel.beginSheetModal(for: window) { [weak self] answer in
            guard let self else { return }
            if answer == .OK, let url = panel.url { self.setFolder(url, keep: true) }
            self.window.makeFirstResponder(self.editor)
        }
    }
    func newChat() { if cd_new(page) != 0 { window.makeFirstResponder(editor) }; wake() }

    private func act(_ what: Int32) {
        var out = [CChar](repeating: 0, count: 262144)
        switch what {
        case 1:
            editor.synchronize { editor.string = "" }
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
        default: break
        }
    }

    /// Opens the Nth chat from the top of the list (1 is the newest).
    func openChat(at index: Int) {
        guard index >= 1, index <= store.list.count else { return }
        open(store.list[index - 1].id)
    }
    /// Opens a kept chat: it becomes the chat that is shown, with the working folder it had. A reply
    /// running in the shown chat goes on out of view; a chat whose reply runs out of view comes back
    /// with its run.
    private func open(_ id: String) {
        guard id != chat, detach() else { return }
        if let back = runs[id] {
            guard cd_take(page, back.page) != 0 else { return }
            runs[id] = nil; cd_close(back.page); back.page = page
            run.stop()   // the window's idle worker: a message in a chat opened later starts another
            run = back
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
        chat = id; recallDue = true; cd_current(page, id)
        var isFolder: ObjCBool = false
        if let kept = store.folder(id), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue { setFolder(URL(fileURLWithPath: kept), keep: false) }
        window.makeFirstResponder(editor)
        wake()
    }
    /// The chat that is shown, when it is kept: File > Rename Chat acts on it.
    var canRename: Bool { chat != nil }
    func renameShown() { if let id = chat { rename(id) } }
    /// The chat that is shown, when it is kept and no reply is running: File > Delete Chat acts on it.
    var canDelete: Bool { chat != nil && cd_running(page) == 0 }
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
            self.store.remove(id)
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
    @objc private func renameFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { rename(id) } }
    @objc private func deleteFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { delete(id) } }
    /// A right-click on a chat in the sidebar: a context menu with Rename Chat and Delete Chat.
    private func rightClick(_ event: NSEvent) -> Bool {
        guard event.window === window else { return false }
        let at = canvas.convert(event.locationInWindow, from: nil), scale = window.backingScaleFactor
        var out = [CChar](repeating: 0, count: 128)
        let y = canvas.isFlipped ? at.y : canvas.bounds.height - at.y
        guard cd_chat_at(page, Float(at.x * scale), Float(y * scale), &out, Int32(out.count)) > 0 else { return false }
        let id = String(cString: out), menu = NSMenu()
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
        if shown { chat = id; cd_current(page, id) } else { r.chat = id }
        cd_chats(page, chatLines)
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
        guard cd_editor(page, &box) != 0 else { editor.isHidden = true; return }
        editor.isHidden = false
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

    func retire() {
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
    @objc func claudeKey(_ sender: Any?) { main?.chooseKey() }
    // Open Folder is off while the shown chat's reply runs, as its button is. New Chat is always on:
    // a running reply goes on out of view.
    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(renameChat(_:)) { return main?.canRename == true }
        if item.action == #selector(deleteChat(_:)) { return main?.canDelete == true }
        if item.action == #selector(openFolder(_:)) { return main?.canFolder == true }
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
        view.addItem(withTitle: "Actual Size", action: #selector(CoderDelegate.actualSize(_:)), keyEquivalent: "0").target = delegate
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
        appItem.submenu = appMenu; editItem.submenu = edit; bar.addItem(appItem); bar.addItem(fileItem); bar.addItem(editItem); bar.addItem(viewItem)
        app.mainMenu = bar
        app.run()
    }
}
