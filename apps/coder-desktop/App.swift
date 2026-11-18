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

/// One kept chat in the list: its id (its file's name), its title, when it last changed and its working folder.
struct CoderChat: Codable { var id: String; var title: String; var updated: Double; var folder: String }
/// The chats kept on this Mac: ~/Library/Application Support/Coder/chats, one file a chat (ID.json,
/// the page's text) and index.json, the list, most recent first.
final class CoderStore {
    private let dir: URL
    private(set) var list: [CoderChat] = []
    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        dir = base.appendingPathComponent("Coder/chats")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: dir.appendingPathComponent("index.json")), let kept = try? JSONDecoder().decode([CoderChat].self, from: data) { list = kept }
    }
    /// The list as the page reads it: one chat a line, its id, a tab, its title.
    var lines: String { list.map { $0.id + "\t" + $0.title }.joined(separator: "\n") }
    func text(_ id: String) -> String? { try? String(contentsOf: dir.appendingPathComponent(id + ".json"), encoding: .utf8) }
    func folder(_ id: String) -> String? { list.first { $0.id == id }?.folder }
    func title(_ id: String) -> String? { list.first { $0.id == id }?.title }
    /// Deletes a chat: its file and its place in the list. It can't be brought back.
    func remove(_ id: String) {
        try? FileManager.default.removeItem(at: dir.appendingPathComponent(id + ".json"))
        list.removeAll { $0.id == id }
        if let data = try? JSONEncoder().encode(list) { try? data.write(to: dir.appendingPathComponent("index.json"), options: .atomic) }
    }
    /// Keeps a chat's text and moves it to the top of the list.
    func keep(id: String, title: String, folder: String, text: String) {
        guard (try? text.write(to: dir.appendingPathComponent(id + ".json"), atomically: true, encoding: .utf8)) != nil else { return }
        list.removeAll { $0.id == id }
        list.insert(CoderChat(id: id, title: title, updated: Date().timeIntervalSince1970, folder: folder), at: 0)
        if let data = try? JSONEncoder().encode(list) { try? data.write(to: dir.appendingPathComponent("index.json"), options: .atomic) }
    }
}

final class CoderWindow: NSObject {
    let window: NSWindow
    private let page: UnsafeMutableRawPointer
    private let canvas: NSView
    private let editor: KitMacEditor
    private let worker = CoderWorker()
    private let session = "coder-desktop-" + UUID().uuidString.lowercased()
    private var line: CGFloat
    private var clock: Timer?
    private var then: String?   // --then TEXT: sent in a new chat once --send's reply ends
    private var showAgent: Int?   // --agent N: the Nth sub-agent's transcript opens as soon as there is one
    private var owed = 3   // frames still to draw after a change
    private let store = CoderStore()
    private var chat: String?          // the id of the chat that is shown, once it is kept
    private var recallDue = false      // a chat was opened: the worker is told its turns ahead of the next message

    init?(resources: String) {
        guard let page = cd_open(resources), let raw = cd_view(page) else { return nil }
        self.page = page
        canvas = Unmanaged<NSView>.fromOpaque(raw).takeUnretainedValue()
        // the text size chosen last time (a row of the page's sizes; 2 is the usual)
        if UserDefaults.standard.object(forKey: "zoom") != nil { _ = cd_zoom_set(page, Int32(UserDefaults.standard.integer(forKey: "zoom"))) }
        var style = [Float](repeating: 0, count: 14); _ = cd_style(page, &style)
        editor = KitMacEditor(style: style); line = CGFloat(style[1])
        // Most of the screen, up to 1500 by 1000 points.
        let screen = NSScreen.main?.visibleFrame.size ?? NSSize(width: 1440, height: 900)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: min(1500, (screen.width * 0.86).rounded()), height: min(1000, (screen.height * 0.88).rounded())),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        super.init()
        window.title = "Coder"; window.minSize = NSSize(width: 420, height: 320); window.isReleasedWhenClosed = false
        canvas.frame = window.contentLayoutRect; canvas.autoresizingMask = [.width, .height]
        window.contentView = canvas
        cd_on_change(page, { raw in guard let raw else { return }; Unmanaged<CoderWindow>.fromOpaque(raw).takeUnretainedValue().wake() },
                     Unmanaged.passUnretained(self).toOpaque())
        cd_skills(page, coderSkillsNote())
        cd_claude(page, CoderWorker.hasClaude ? 1 : 0)
        cd_chats(page, store.lines)
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
        worker.line = { [weak self] text in
            guard let self else { return }
            cd_line(self.page, text)
            self.pump()
        }
        worker.gone = { [weak self] in guard let self else { return }; cd_failed(self.page, "Microcoder stopped running. Send the message again."); self.pump() }
        canvas.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: canvas, queue: .main) { [weak self] _ in self?.wake() }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { [weak self] event in self?.rightClick(event) == true ? nil : event }
        let clock = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(clock, forMode: .common); self.clock = clock
        window.center(); window.makeKeyAndOrderFront(nil); window.makeFirstResponder(editor)
        // Coder --chat N opens the Nth most recent chat at launch (1 is the newest).
        if let at = CommandLine.arguments.firstIndex(of: "--chat"), at + 1 < CommandLine.arguments.count, let k = Int(CommandLine.arguments[at + 1]), k >= 1, k <= store.list.count { open(store.list[k - 1].id) }
        // Coder --send TEXT sends TEXT at launch, for a check of the whole path with no typing.
        if let at = CommandLine.arguments.firstIndex(of: "--send"), at + 1 < CommandLine.arguments.count, cd_set_text(page, CommandLine.arguments[at + 1]) != 0, cd_submit(page) != 0,
           let next = CommandLine.arguments.firstIndex(of: "--then"), next + 1 < CommandLine.arguments.count { then = CommandLine.arguments[next + 1] }
        // Coder --agent N opens the transcript of the Nth sub-agent of the newest message that has one.
        if let at = CommandLine.arguments.firstIndex(of: "--agent"), at + 1 < CommandLine.arguments.count { showAgent = Int(CommandLine.arguments[at + 1]) }
        tick()
    }

    func wake() { owed = 3 }

    /// The sub-agents' workers, by the page's number for each. Microcoder hands tasks over in one
    /// line; each runs on a worker of its own, all at once, and the page reads every line.
    private var agents: [Int32: CoderWorker] = [:]
    /// After any worker's line: answer what is owed, start the sub-agents the page asks for, and
    /// stop the workers of those that have ended. (Each count is taken before its write: see act.)
    private func pump() {
        var out = [CChar](repeating: 0, count: 300_000)
        while true {
            let j = cd_agent_next(page)
            if j < 0 { break }
            let one = CoderWorker()
            if cd_agent_claude(page, j) != 0 { one.program = "claudeworker" }
            one.line = { [weak self] text in guard let self, self.agents[j] === one else { return }; cd_agent_line(self.page, j, text); self.pump() }
            one.gone = { [weak self] in guard let self, self.agents[j] === one else { return }; self.agents[j] = nil; cd_agent_failed(self.page, j, "The sub-agent's worker stopped running."); self.pump() }
            guard one.start() else { cd_agent_failed(page, j, "The sub-agent's worker could not start."); continue }
            agents[j] = one
            let n = cd_agent_task(page, j, session + "-agent-\(j)-" + UUID().uuidString.lowercased(), folder.path, &out, Int32(out.count))
            one.write(out, count: Int(n))
        }
        for (j, one) in agents {
            let n = cd_agent_answer_line(page, j, &out, Int32(out.count))
            one.write(out, count: Int(n))
            if cd_agent_running(page, j) == 0 { agents[j] = nil; one.gone = nil; one.line = nil; one.stop() }
        }
        let n = cd_answer_line(page, &out, Int32(out.count)); worker.write(out, count: Int(n))
        wake()
    }
    var canNew: Bool { cd_running(page) == 0 }
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
            guard worker.start() else { cd_failed(page, "Microcoder could not start."); return }
            if recallDue {
                // an opened chat's turns, or nothing to remember when it has none that ended
                recallDue = false
                var n = cd_recall_line(page, &out, Int32(out.count))
                if n == 0 { n = cd_forget_line(&out, Int32(out.count)) }
                worker.write(out, count: Int(n))
            }
            let n = cd_task(page, session, folder.path, &out, Int32(out.count))
            worker.write(out, count: Int(n))
        case 2: window.makeFirstResponder(editor)
        // The line is written into out first: Swift copies an argument before a later one's call fills it.
        case 3: let n = cd_stop_line(&out, Int32(out.count)); worker.write(out, count: Int(n))
        case 4:
            chat = nil; recallDue = false; cd_current(page, "")
            let n = cd_forget_line(&out, Int32(out.count)); worker.write(out, count: Int(n))
        case 5:
            out = [CChar](repeating: 0, count: 2_000_001)   // a whole chat fits: 24 messages and replies
            guard cd_copied(page, &out, 2_000_000) > 0 else { return }
            let board = NSPasteboard.general; board.clearContents(); board.setString(String(cString: out), forType: .string)
        case 6: chooseFolder()
        case 7:
            // a press on a chat in the sidebar: its kept text becomes the chat that is shown
            guard cd_open_id(page, &out, Int32(out.count)) > 0 else { return }
            open(String(cString: out))
        default: break
        }
    }

    /// Opens a kept chat: it becomes the chat that is shown, with the working folder it had.
    private func open(_ id: String) {
        guard let text = store.text(id), cd_load(page, text) != 0 else { return }
        chat = id; recallDue = true; cd_current(page, id)
        var isFolder: ObjCBool = false
        if let kept = store.folder(id), FileManager.default.fileExists(atPath: kept, isDirectory: &isFolder), isFolder.boolValue { setFolder(URL(fileURLWithPath: kept), keep: false) }
        window.makeFirstResponder(editor)
        wake()
    }
    /// The chat that is shown, when it is kept and no reply is running: File > Delete Chat acts on it.
    var canDelete: Bool { chat != nil && cd_running(page) == 0 }
    func deleteShown() { if let id = chat { delete(id) } }
    /// Asks, then deletes a kept chat. Deleting the one that is shown starts a new chat. Not while a
    /// reply is running in it.
    func delete(_ id: String) {
        if id == chat && cd_running(page) != 0 { return }
        let alert = NSAlert()
        alert.messageText = "Delete this chat?"
        alert.informativeText = "“" + (store.title(id) ?? "New chat") + "” will be deleted from this Mac. You can't undo this."
        alert.addButton(withTitle: "Delete").hasDestructiveAction = true
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { [weak self] answer in
            guard let self, answer == .alertFirstButtonReturn else { return }
            if id == self.chat { guard cd_running(self.page) == 0 else { return }; self.chat = nil; _ = cd_new(self.page) }
            self.store.remove(id)
            cd_chats(self.page, self.store.lines)
            self.window.makeFirstResponder(self.editor); self.wake()
        }
    }
    @objc private func deleteFromMenu(_ item: NSMenuItem) { if let id = item.representedObject as? String { delete(id) } }
    /// A right-click on a chat in the sidebar: a menu with Delete Chat.
    private func rightClick(_ event: NSEvent) -> Bool {
        guard event.window === window else { return false }
        let at = canvas.convert(event.locationInWindow, from: nil), scale = window.backingScaleFactor
        var out = [CChar](repeating: 0, count: 128)
        let y = canvas.isFlipped ? at.y : canvas.bounds.height - at.y
        guard cd_chat_at(page, Float(at.x * scale), Float(y * scale), &out, Int32(out.count)) > 0 else { return false }
        let id = String(cString: out), menu = NSMenu()
        let item = menu.addItem(withTitle: "Delete Chat…", action: #selector(deleteFromMenu(_:)), keyEquivalent: "")
        item.target = self; item.representedObject = id
        item.isEnabled = !(id == chat && cd_running(page) != 0)
        menu.autoenablesItems = false
        NSMenu.popUpContextMenu(menu, with: event, for: canvas)
        return true
    }
    private var monitor: Any?

    /// Keeps the chat that is shown: its file, and its place at the top of the list.
    private func keep() {
        let id = chat ?? UUID().uuidString.lowercased()
        var out = [CChar](repeating: 0, count: 32_000_000)   // 24 messages with every row and every sub-agent's rows full fit
        guard cd_save(page, &out, Int32(out.count)) > 0 else { return }
        let text = String(cString: out)
        var name = [CChar](repeating: 0, count: 256)
        let title = cd_title(page, &name, Int32(name.count)) > 0 ? String(cString: name) : "New chat"
        store.keep(id: id, title: title, folder: folder.path, text: text)
        chat = id; cd_current(page, id); cd_chats(page, store.lines)
    }

    private func tick() {
        let what = cd_action(page)
        if what != 0 { act(what); wake() }
        if let n = showAgent, cd_pane(page, Int32(n)) != 0 { showAgent = nil; wake() }
        if cd_save_due(page) != 0 { keep(); wake() }
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

    func retire() { if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil; clock?.invalidate(); clock = nil; worker.gone = nil; worker.stop(); for one in agents.values { one.gone = nil; one.line = nil; one.stop() }; agents = [:]; cd_on_change(page, nil, nil); cd_close(page) }
}

final class CoderDelegate: NSObject, NSApplicationDelegate {
    var main: CoderWindow?
    func applicationDidFinishLaunching(_ notification: Notification) {
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path, let main = CoderWindow(resources: resources) else {
            fputs("Coder: the GPU surface could not start.\n", stderr); exit(1)
        }
        self.main = main
        NSApp.activate(ignoringOtherApps: true)
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationWillTerminate(_ notification: Notification) { main?.retire() }
    @objc func newChat(_ sender: Any?) { main?.newChat() }
    @objc func openFolder(_ sender: Any?) { main?.chooseFolder() }
    @objc func deleteChat(_ sender: Any?) { main?.deleteShown() }
    @objc func biggerText(_ sender: Any?) { main?.zoom(1) }
    @objc func smallerText(_ sender: Any?) { main?.zoom(-1) }
    @objc func actualSize(_ sender: Any?) { main?.zoom(nil) }
    // New Chat is off while a run is going, as the button is.
    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool { item.action == #selector(deleteChat(_:)) ? main?.canDelete == true : (item.action != #selector(newChat(_:)) && item.action != #selector(openFolder(_:))) || main?.canNew == true }
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
        viewItem.submenu = view
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
