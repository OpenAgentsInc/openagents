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

    func start() -> Bool {
        if alive { return true }
        guard let path = Bundle.main.path(forResource: "mcworker", ofType: nil) else { return false }
        let p = Process(), into = Pipe(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.currentDirectoryURL = FileManager.default.homeDirectoryForCurrentUser
        // An app opened from the Finder has the system's short PATH: the commands Microcoder runs also
        // find what is installed for this person (gamebuilder is in ~/.local/bin).
        var env = ProcessInfo.processInfo.environment
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let had = (env["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin").split(separator: ":").map(String.init)
        let more = [home + "/.local/bin", "/opt/homebrew/bin", "/usr/local/bin"].filter { !had.contains($0) }
        env["PATH"] = (more + had).joined(separator: ":")
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
    func stop() { let p = process; process = nil; input = nil; p?.terminate() }
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
    private var owed = 3   // frames still to draw after a change

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
            // a line that asked about a command is answered at once (the count first: see act)
            var out = [CChar](repeating: 0, count: 256)
            let n = cd_answer_line(self.page, &out, Int32(out.count)); self.worker.write(out, count: Int(n))
            self.wake()
        }
        worker.gone = { [weak self] in guard let self else { return }; cd_failed(self.page, "Microcoder stopped running. Send the message again."); self.wake() }
        canvas.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: canvas, queue: .main) { [weak self] _ in self?.wake() }
        let clock = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(clock, forMode: .common); self.clock = clock
        window.center(); window.makeKeyAndOrderFront(nil); window.makeFirstResponder(editor)
        // Coder --send TEXT sends TEXT at launch, for a check of the whole path with no typing.
        if let at = CommandLine.arguments.firstIndex(of: "--send"), at + 1 < CommandLine.arguments.count, cd_set_text(page, CommandLine.arguments[at + 1]) != 0, cd_submit(page) != 0,
           let next = CommandLine.arguments.firstIndex(of: "--then"), next + 1 < CommandLine.arguments.count { then = CommandLine.arguments[next + 1] }
        tick()
    }

    func wake() { owed = 3 }
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
            let n = cd_task(page, session, folder.path, &out, Int32(out.count))
            worker.write(out, count: Int(n))
        case 2: window.makeFirstResponder(editor)
        // The line is written into out first: Swift copies an argument before a later one's call fills it.
        case 3: let n = cd_stop_line(&out, Int32(out.count)); worker.write(out, count: Int(n))
        case 4: let n = cd_forget_line(&out, Int32(out.count)); worker.write(out, count: Int(n))
        case 5:
            out = [CChar](repeating: 0, count: 2_000_001)   // a whole chat fits: 24 messages and replies
            guard cd_copied(page, &out, 2_000_000) > 0 else { return }
            let board = NSPasteboard.general; board.clearContents(); board.setString(String(cString: out), forType: .string)
        case 6: chooseFolder()
        default: break
        }
    }

    private func tick() {
        let what = cd_action(page)
        if what != 0 { act(what); wake() }
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

    func retire() { clock?.invalidate(); clock = nil; worker.gone = nil; worker.stop(); cd_on_change(page, nil, nil); cd_close(page) }
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
    @objc func biggerText(_ sender: Any?) { main?.zoom(1) }
    @objc func smallerText(_ sender: Any?) { main?.zoom(-1) }
    @objc func actualSize(_ sender: Any?) { main?.zoom(nil) }
    // New Chat is off while a run is going, as the button is.
    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool { (item.action != #selector(newChat(_:)) && item.action != #selector(openFolder(_:))) || main?.canNew == true }
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
