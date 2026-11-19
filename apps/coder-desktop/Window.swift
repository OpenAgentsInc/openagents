// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The window: a chat's run, the page and editor, the clock, what the page asks for, and quitting.
import AppKit

/// The window: the compiled page fills it. The platform editor sits over the composer's text box
/// and hands Coax each whole draft; what the page asks for (send, stop, a new chat, copy) is done here.
/// One chat's run: the page that reads its lines, its worker and its sub-agents' workers. The chat
/// that is shown runs on the window's page. One whose reply goes on out of view, after New Chat or
/// a press on another chat, has a page with no window (cd_headless) until it ends or is opened again.
final class CoderRun {
    var page: UnsafeMutableRawPointer
    let worker = CoderWorker()
    var agents: [Int32: CoderWorker] = [:]
    var session = "coder-desktop-" + UUID().uuidString.lowercased()
    var folder: URL                    // its working folder, once it is out of view
    var chat: String?                  // its kept chat's id, once it is out of view
    var turn: CoderTurn?               // the running message, kept so a reopened Coder attaches to it
    var queued: [String: String] = [:] // the whole text of each message this Coder queued, by id (the page keeps previews)
    init(page: UnsafeMutableRawPointer, folder: URL) { self.page = page; self.folder = folder }
    func stop() {
        endTurn()
        worker.gone = nil; worker.line = nil; worker.stop()
        for one in agents.values { one.gone = nil; one.line = nil; one.stop() }
        agents = [:]
    }
    /// The message ended: nothing is left to attach to, and its sub-agents' runs go.
    func endTurn() {
        guard let t = turn else { return }
        turn = nil
        CoderTurn.remove(t.run)
        for id in t.agents { CoderWorker.daemon?.forgetRun(id) }
    }
    /// Coder quits: the daemon keeps its workers running, and the kept turn says where to attach.
    func detach() {
        worker.gone = nil; worker.line = nil; worker.detach()
        for one in agents.values { one.gone = nil; one.line = nil; one.detach() }
        agents = [:]
    }
}

/// A message whose workers the daemon runs (GitHub #13694), kept in Coder's Application Support
/// folder as turns/RUN.json with the chat as it was when the message was sent (RUN.chat). A Coder
/// that opens again loads that chat, sends the message on its page without a worker, and reads
/// the run's lines from `offset` on, its sub-agents' runs in the order they started.
struct CoderTurn: Codable {
    var run: String
    var chat: String
    var offset: UInt64
    var folder: String
    var session: String
    var program: String
    var agents: [String]
    static var dir: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        return base.appendingPathComponent("Coder/turns", isDirectory: true)
    }
    func save(chatText: String? = nil) {
        let fm = FileManager.default
        try? fm.createDirectory(at: CoderTurn.dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        if let chatText { try? Data(chatText.utf8).write(to: CoderTurn.dir.appendingPathComponent(run + ".chat"), options: .atomic) }
        if let data = try? JSONEncoder().encode(self) { try? data.write(to: CoderTurn.dir.appendingPathComponent(run + ".json"), options: .atomic) }
    }
    var chatText: String? { try? String(contentsOf: CoderTurn.dir.appendingPathComponent(run + ".chat"), encoding: .utf8) }
    static func remove(_ run: String) {
        for ext in [".json", ".chat"] { try? FileManager.default.removeItem(at: dir.appendingPathComponent(run + ext)) }
    }
    static func all() -> [CoderTurn] {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
        return names.filter { $0.hasSuffix(".json") }.compactMap { name in
            guard let data = try? Data(contentsOf: dir.appendingPathComponent(name)) else { return nil }
            return try? JSONDecoder().decode(CoderTurn.self, from: data)
        }
    }
    /// The kept chat without its last message (the one this turn sent), for the page to send again.
    static func withoutLast(_ text: String) -> (chat: String, ask: String)? {
        guard let data = text.data(using: .utf8), var obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              var turns = obj["turns"] as? [Any], let last = turns.popLast() as? [String: Any], let ask = last["ask"] as? String, !ask.isEmpty else { return nil }
        obj["turns"] = turns
        guard let out = try? JSONSerialization.data(withJSONObject: obj), let chat = String(data: out, encoding: .utf8) else { return nil }
        return (chat, ask)
    }
}
final class CoderWindow: NSObject, NSWindowDelegate {
    let window: NSWindow
    let page: UnsafeMutableRawPointer
    let canvas: NSView
    let editor: KitMacEditor
    var run: CoderRun                    // the shown chat's run, on the window's page
    var runs: [String: CoderRun] = [:]   // chats whose replies run out of view, by id
    var unread: Set<String> = []         // background chats with unread output or a completed turn
    private var last = CACurrentMediaTime()      // when the clock last ticked, for their pages' seconds
    var line: CGFloat
    private var clock: Timer?
    private var then: String?   // --then TEXT: sent in a new chat once --send's reply ends
    private var also: String?   // --new TEXT: sent in a new chat while --send's reply runs
    private var showAgent: Int?   // --agent N: the Nth sub-agent's transcript opens as soon as there is one
    var owed = 3   // frames still to draw after a change
    private var restoreEditorFocus = false
    let store = CoderStore()
    var chat: String?          // the id of the chat that is shown, once it is kept
    var recallDue = false      // a chat was opened: the worker is told its turns ahead of the next message
    var choosingFolder = false // prevents stacking folder sheets while one is opening or open
    let launchBinary = CoderWindow.currentBinaryIdentity()
    let foremanServer = ForemanControlServer()
    let foremanEvents = ForemanEventBus()
    let foremanAudit = ForemanAuditLog()
    let daemonClient = CoderDaemonClient()
    private var lastForegroundRunning = false
    var foremanTier: ForemanTier = ForemanTier.load()
    let updater = CoderUpdater()
    /// Settings' search field (settings.coax draws its box and placeholder; this holds the typing).
    let search: KitMacEditor
    /// The working folder: commands start there. The corner shows it with ~ for the home folder.
    var folder = FileManager.default.homeDirectoryForCurrentUser
    private var monitor: Any?
    private var keyMonitor: Any?   // Esc in settings

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
        var searchStyle = style; searchStyle[0] = 12 * style[0] / 14.4; searchStyle[1] = 16 * style[0] / 14.4
        search = KitMacEditor(style: searchStyle)
        // Most of the screen, up to 1500 by 1000 points.
        let screen = NSScreen.main?.visibleFrame.size ?? NSSize(width: 1440, height: 900)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: min(1500, (screen.width * 0.86).rounded()), height: min(1000, (screen.height * 0.88).rounded())),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        super.init()
        window.title = "Coder"; window.titleVisibility = .hidden; window.minSize = NSSize(width: 420, height: 320); window.isReleasedWhenClosed = false; window.delegate = self
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
        if foremanServer.start() {
            ForemanControlDescriptor.write(url: foremanServer.baseURL, token: foremanServer.operatorToken, auditLog: foremanAudit.fileURL.path)
        }
        // the unified daemon runs the workers when it can, so runs outlive Coder (GitHub #13694)
        CoderWorker.daemon = daemonClient
        daemonClient.ensureDaemon()
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
        // Settings' search: each edit filters the list; Return opens the best match.
        search.committed = { [weak self] text in guard let self else { return }; cd_search_set(self.page, text) }
        search.command = { [weak self] selector in
            guard let self else { return false }
            if selector == #selector(NSResponder.insertNewline(_:)) { _ = cd_search_submit(self.page); return true }
            if selector == #selector(NSResponder.cancelOperation(_:)) { self.escape(); return true }
            return selector == #selector(NSResponder.insertTab(_:))
        }
        search.textContainer?.widthTracksTextView = false; search.textContainer?.size = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        search.isHorizontallyResizable = false; search.isHidden = true
        search.setAccessibilityIdentifier("set.search.field"); search.setAccessibilityLabel("Search settings")
        canvas.addSubview(search)
        syncSettingsInfo()
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self, event.keyCode == 53, event.window === self.window, self.window.attachedSheet == nil, cd_settings_page(self.page) >= 0 else { return event }
            self.escape(); return nil
        }
        wire(run)
        reattach()
        canvas.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: canvas, queue: .main) { [weak self] _ in self?.wake() }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { [weak self] event in self?.rightClick(event) == true ? nil : event }
        let clock = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(clock, forMode: .common); self.clock = clock
        window.center(); window.makeKeyAndOrderFront(nil); window.makeFirstResponder(editor)
        // Coder --foreman opens the persistent Foreman chat at launch.
        if CommandLine.arguments.contains("--foreman") { openForeman() }
        // Coder --settings PAGE opens settings at launch on a page (general, models, foreman, account, about).
        if let at = CommandLine.arguments.firstIndex(of: "--settings"), at + 1 < CommandLine.arguments.count,
           let which = ["general", "models", "foreman", "account", "about"].firstIndex(of: CommandLine.arguments[at + 1]) { openSettings(Int32(which)) }
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

    func act(_ what: Int32) {
        var out = [CChar](repeating: 0, count: 262144)
        switch what {
        case 1:
            editor.synchronize { editor.string = "" }
            configureWorkerEnv(run, isForeman: isForemanShown)
            syncSkillsNote(for: page, isForeman: isForemanShown)
            guard run.worker.start() else { cd_failed(page, run.worker.failure ?? "Microcoder could not start."); return }
            let from = run.worker.offset
            if recallDue {
                // an opened chat's turns, or nothing to remember when it has none that ended
                recallDue = false
                var n = cd_recall_line(page, &out, Int32(out.count))
                if n == 0 { n = cd_forget_line(&out, Int32(out.count)) }
                run.worker.write(out, count: Int(n))
            }
            let n = cd_task(page, run.session, folder.path, &out, Int32(out.count))
            run.worker.write(out, count: Int(n))
            beginTurn(run, from: from)
            bindQueue(run)
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
        case 13: settingsMoved()
        case 14: zoom(1)
        case 15: zoom(-1)
        case 16: chooseKey()
        case 17: chooseOpenRouter()
        case 18: if let tier = ForemanTier(rawValue: Int(cd_settings_tier(page))) { setForemanTier(tier) }
        case 19: openForemanAuditLog()
        case 20: clearForemanHistory()
        case 21: if let url = URL(string: "https://openagents.com/login") { NSWorkspace.shared.open(url) }
        case 22: checkForUpdates()
        case 23: window.makeFirstResponder(search)
        case 24:
            // a file a row or a reply names: opened from the working folder unless its path is whole
            guard cd_open_path(page, &out, Int32(out.count)) > 0 else { return }
            let named = (String(cString: out) as NSString).expandingTildeInPath
            let url = named.hasPrefix("/") ? URL(fileURLWithPath: named) : folder.appendingPathComponent(named)
            if FileManager.default.fileExists(atPath: url.path) { NSWorkspace.shared.open(url) }
        default: break
        }
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
                    let from = back.worker.offset
                    let n = cd_task(back.page, back.session, back.folder.path, &out, Int32(out.count))
                    back.worker.write(out, count: Int(n))
                    beginTurn(back, from: from)
                    bindQueue(back)
                } else {
                    cd_failed(back.page, back.worker.failure ?? "Microcoder could not start.")
                }
            } else if bgAct == 3 {
                stopRun(back)
            }
            if cd_save_due(back.page) != 0 { keep(back) }
            if cd_running(back.page) == 0 && cd_queue_waiting(back.page) == 0 { finish(id) }   // a waiting queue starts its next message
        }
        let what = cd_action(page)
        if what != 0 { act(what); wake() }
        let fgRunning = cd_running(page) != 0
        if fgRunning != lastForegroundRunning {
            lastForegroundRunning = fgRunning
            if !fgRunning { run.endTurn() }
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
        placeSearch()
        var box = [Float](repeating: 0, count: 6)
        guard cd_editor(page, &box) != 0 else {
            if focused { restoreEditorFocus = true }
            // in settings the hidden composer gives the keyboard up
            if focused && cd_settings_page(page) >= 0 { window.makeFirstResponder(nil) }
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

    /// Whether quitting leaves the running messages going: the daemon runs their workers.
    var runsOutliveQuit: Bool { CoderWorker.daemon?.ownsRuns == true }
    var runningCount: Int {
        (cd_running(page) != 0 ? 1 : 0) + runs.values.filter { cd_running($0.page) != 0 }.count
    }
    func show() { window.makeKeyAndOrderFront(nil); wake() }
    func windowShouldClose(_ sender: NSWindow) -> Bool { window.orderOut(nil); return false }
    var retired = false

    func retire() {
        guard !retired else { return }
        retired = true
        updater.stop()
        foremanServer.stop()
        ForemanControlDescriptor.remove()
        if chat != nil { keep(closing: true) }
        // the daemon's runs go on after Coder quits; a reopened Coder attaches to them (GitHub #13694)
        for back in runs.values { keep(back, closing: true); if back.turn != nil { back.detach() } else { back.stop() }; cd_close(back.page) }
        runs = [:]
        if run.turn != nil && cd_running(page) != 0 { run.detach() } else { run.stop() }
        if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil; clock?.invalidate(); clock = nil
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }; keyMonitor = nil
        cd_on_change(page, nil, nil); cd_close(page)
    }
}
