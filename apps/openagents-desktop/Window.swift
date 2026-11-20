// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The window: a chat's run, the page, the clock, what the page asks for, and quitting.
import AppKit

/// The window: the compiled page fills it and draws and edits its own text fields (page/editor.coax:
/// the view hands it every key and the input method); what the page asks for (send, stop, a new
/// chat, copy) is done here.
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
    var steered: [String: String] = [:] // the whole text of each steer this Coder sent, by id
    var bound: String?                 // the chat id keep() last bound its worker's queue to
    var worktree: ChatWorktree?        // the worktree made for its chat's first message (Window+Worktrees.swift)
    var pendingChat: String?           // chat id reserved while its worktree is being created
    var preparingWorktree = false, worktreeSkip = false, startingWorker = false
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
    var run: CoderRun                    // the shown chat's run, on the window's page
    var runs: [String: CoderRun] = [:]   // chats whose replies run out of view, by id
    var unread: Set<String> = []         // background chats with unread output or a completed turn
    private var last = CACurrentMediaTime()      // when the clock last ticked, for their pages' seconds
    private var link: CADisplayLink?     // the clock: the canvas's display's refresh (120 Hz on ProMotion)
    private var watchdog: Timer?         // ticks while the display link is paused (the window hidden or off every screen)
    private var ticked = 0.0             // when tick last ran
    private var then: String?   // --then TEXT: sent in a new chat once --send's reply ends
    private var also: String?   // --new TEXT: sent in a new chat while --send's reply runs
    private var showAgent: Int?   // --agent N: the Nth sub-agent's transcript opens as soon as there is one
    var owed = 3   // frames still to draw after a change
    private var blinkAt = 0.0           // when the caret next blinks (the page's own clock), 0 for never
    var draftBeforeEdit = ""          // the draft there before a queued message's text was edited in the composer, back after (GitHub #13717)
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
    /// The working folder: commands start there. The corner shows it with ~ for the home folder.
    var folder = FileManager.default.homeDirectoryForCurrentUser
    var fileURL: URL?                    // the file in the pane on the right (Window+Files.swift)
    var scenePanel: PleScenePanelView?   // the embedded Metal 3D PLE scene viewport (SceneView.swift)
    var gymView: GymWorkbenchView?       // the Gym / 3D PLE Workbench view (GymWorkbenchView.swift, #13792)
    private var monitor: Any?
    private var keyMonitor: Any?   // Esc in settings and the composer; the editing chords before the menu bar's
    private var flagsMonitor: Any? // Command-key hold (>1s) sidebar shortcut hints
    private var cmdHoldTimer: Timer?
    private var cmdHintsShown = false
    private var titleBar: CoderTitleBar?   // the transparent title bar and the traffic lights' places (Window+TitleBar.swift)

    init?(resources: String) {
        guard let page = cd_open(resources), let raw = cd_view(page) else { return nil }
        self.page = page
        run = CoderRun(page: page, folder: FileManager.default.homeDirectoryForCurrentUser)
        canvas = Unmanaged<NSView>.fromOpaque(raw).takeUnretainedValue()
        // the text size chosen last time (a row of the page's sizes; 2 is the usual)
        if UserDefaults.standard.object(forKey: "zoom") != nil { _ = cd_zoom_set(page, Int32(UserDefaults.standard.integer(forKey: "zoom"))) }
        // the sidebars' widths as dragged last time
        cd_sides_set(page, UserDefaults.standard.float(forKey: "sideWidth"), UserDefaults.standard.float(forKey: "paneWidth"))
        // Most of the screen, up to 1500 by 1000 points.
        let screen = NSScreen.main?.visibleFrame.size ?? NSSize(width: 1440, height: 900)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: min(1500, (screen.width * 0.86).rounded()), height: min(1000, (screen.height * 0.88).rounded())),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        super.init()
        window.title = "Coder"; window.titleVisibility = .hidden; window.minSize = NSSize(width: 420, height: 320); window.isReleasedWhenClosed = false; window.delegate = self
        // the page fills the window, under its transparent title bar; a file shows in the page's own pane (page/filepane.coax)
        canvas.frame = NSRect(origin: .zero, size: window.contentRect(forFrameRect: window.frame).size); canvas.autoresizingMask = [.width, .height]
        window.contentView = canvas
        titleBar = CoderTitleBar(window: window, canvas: canvas, page: page)
        cd_on_change(page, { raw in guard let raw else { return }; Unmanaged<CoderWindow>.fromOpaque(raw).takeUnretainedValue().wake() },
                     Unmanaged.passUnretained(self).toOpaque())
        cd_skills(page, coderSkillsNote())
        cd_claude(page, CoderWorker.hasClaude ? 1 : 0)
        cd_agy(page, CoderWorker.hasAntigravity ? 1 : 0)
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
        // Foreman, Gym, log in and the account are experimental features (Settings > General), off unless set
        cd_experimental_set(page, Experimental.on ? 1 : 0)
        if Experimental.on { startForemanServer() }
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
        syncSettingsInfo()
        flagsMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            self?.handleModifierFlags(event.modifierFlags)
            return event
        }
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            self?.cancelCmdHold()
            guard let self, event.window === self.window, self.window.attachedSheet == nil else { return event }
            // Command-` opens Foreman when enabled (heard before macOS window cycling).
            let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
            if flags.contains(.command) && !flags.contains(.shift) && !flags.contains(.option) && !flags.contains(.control) && event.charactersIgnoringModifiers == "`" && (!self.overlay.dialogOpen || self.overlay.paletteUp) {
                if self.isForemanEnabled {
                    if cd_palette_open(self.page) != 0 { _ = cd_palette_toggle(self.page, 0) }
                    self.openForeman()
                }
                return nil
            }
            // the editing chords go to the field with the keyboard before the menu bar sees them
            if self.window.firstResponder === self.canvas && cd_wants_text(self.page) != 0, let key = self.editingChord(event) {
                if cd_key(self.page, key.code, key.mods) != 0 { self.wake(); return nil }
            }
            // in the composer Esc gives up an edit or stops the reply (#13717); Shift-Return is the
            // File menu's (followUpOther)
            if self.composerFocused {
                if event.keyCode == 53 && cd_followup_escape(self.page) != 0 { self.wake(); return nil }
            }
            guard event.keyCode == 53, cd_settings_page(self.page) >= 0 else { return event }
            self.escape(); return nil
        }
        wire(run)
        reattach()
        canvas.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: canvas, queue: .main) { [weak self] _ in self?.wake() }
        monitor = NSEvent.addLocalMonitorForEvents(matching: .rightMouseDown) { [weak self] event in self?.rightClick(event) == true ? nil : event }
        // Frames come with the display's refresh, measured by the system (Zeron's per-display link):
        // 120 Hz on a ProMotion screen. Idle, the page draws nothing; its other work runs at most 60 times a second.
        let link = canvas.displayLink(target: self, selector: #selector(refresh(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        link.add(to: .main, forMode: .common); self.link = link
        let watchdog = Timer(timeInterval: 0.25, repeats: true) { [weak self] _ in
            guard let self, CACurrentMediaTime() - self.ticked > 0.25 else { return }
            self.tick()
        }
        RunLoop.main.add(watchdog, forMode: .common); self.watchdog = watchdog
        window.center(); window.makeKeyAndOrderFront(nil); focusComposer()
        // Coder --foreman opens the persistent Foreman chat at launch.
        if CommandLine.arguments.contains("--foreman") { openForeman() }
        if CommandLine.arguments.contains("--gym") { openGym() }
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
    /// The composer (or settings' search) takes the keyboard: the page's view is the first responder.
    func focusComposer() { cd_focus(page, 1); window.makeFirstResponder(canvas); wake() }
    func focusSearch() { cd_focus(page, 2); window.makeFirstResponder(canvas); wake() }
    var composerFocused: Bool { window.firstResponder === canvas && cd_settings_page(page) < 0 && cd_overlay_open(page) == 0 && cd_wants_text(page) != 0 }
    /// The composer's text.
    var draft: String {
        var out = [CChar](repeating: 0, count: 16385)
        return cd_draft(page, &out, Int32(out.count)) > 0 ? String(cString: out) : ""
    }
    /// Command-C, -X, -V, -A, -Z and Shift-Command-Z as the page's key and modifiers; nil for the rest.
    private func editingChord(_ event: NSEvent) -> (code: Int32, mods: Int32)? {
        let flags = event.modifierFlags.intersection([.shift, .control, .option, .command])
        guard flags.contains(.command), !flags.contains(.control), !flags.contains(.option),
              let c = event.charactersIgnoringModifiers?.lowercased().unicodeScalars.first, "cxvaz".unicodeScalars.contains(c) else { return nil }
        if flags.contains(.shift) && c != "z" { return nil }
        return (Int32(c.value), flags.contains(.shift) ? 9 : 8)
    }

    func act(_ what: Int32) {
        var out = [CChar](repeating: 0, count: 262144)
        switch what {
        case 1:
            if holdForWorktree() { return }   // a new chat's own worktree first (#13758)
            sendTask(run)
        case 2: focusComposer()
        // The line is written into out first: Swift copies an argument before a later one's call fills it.
        case 3: stopRun(run)
        case 4:
            // a new chat: one whose reply is running goes on out of view
            closeGym()
            guard detach() else { return }
            chat = nil; recallDue = false; cd_current(page, "")
            leaveWorktreeFolder()
            configureWorkerEnv(run, isForeman: false)
            syncSkillsNote(for: page, isForeman: false)
            syncForemanUI()
            let n = cd_forget_line(&out, Int32(out.count)); run.worker.write(out, count: Int(n))
            focusComposer()
        case 5:
            // the buffer grows until the copy fits: a chat has no message cap (#13745); a full buffer may have cut it
            var cap = 2_000_000, n: Int32 = 0
            repeat {
                out = [CChar](repeating: 0, count: cap + 1)
                n = cd_copied(page, &out, Int32(cap + 1))
                if n < Int32(cap) { break }
                cap *= 2
            } while cap <= 1_536_000_000
            guard n > 0 else { return }
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
        case 21: if Experimental.on, let url = URL(string: "https://openagents.com/login") { NSWorkspace.shared.open(url) }
        case 22: checkForUpdates()
        case 23: focusSearch()
        case 24:
            // a file a row or a reply names: opened in the right-side file pane (from the working folder unless whole)
            guard cd_open_path(page, &out, Int32(out.count)) > 0 else { return }
            let named = (String(cString: out) as NSString).expandingTildeInPath
            let url = named.hasPrefix("/") ? URL(fileURLWithPath: named) : folder.appendingPathComponent(named)
            openNamedFile(url)
        case 25: followUp()
        case 26: paletteAsked()
        case 27: attachMenu()
        case 28: listFiles()
        case 29: zoom(nil)
        case 30: let j = Int(cd_stop_agent(run.page)); if j >= 0 { cancelAgent(run, j) }
        case 31:
            // an http(s) link in a reply or sub-agent message: opened in the system browser
            guard cd_open_url(page, &out, Int32(out.count)) > 0 else { return }
            if let url = URL(string: String(cString: out)),
               let scheme = url.scheme?.lowercased(),
               scheme == "http" || scheme == "https" {
                NSWorkspace.shared.open(url)
            }
        case 32: openGym()
        case 33: chooseAntigravity()
        case 34: setAntigravityMode(Int(cd_settings_agy_mode(page)))
        case 35: chooseAntigravityAuth()
        case 36: experimentalChanged()
        case 37: forkShown()
        case 40...43: fileAsked(what)
        default: break
        }
    }
    @objc private func refresh(_ link: CADisplayLink) {
        let blinkDue = blinkAt > 0 && CACurrentMediaTime() >= blinkAt
        if owed == 0 && !blinkDue && cd_moving(page) == 0 && CACurrentMediaTime() - ticked < 1.0 / 61.0 { return }
        if blinkDue { owed = max(owed, 1) }
        tick()
    }
    private func tick() {
        ticked = CACurrentMediaTime()
        // replies out of view: their seconds pass, they are kept as they change, and they end
        let now = CACurrentMediaTime(), dt = Float(now - last); last = now
        for (id, back) in runs {
            cd_tick(back.page, dt)
            let bgAct = cd_action(back.page)
            if bgAct == 1 {
                sendTask(back)
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
            if !fgRunning { run.endTurn(); refreshBranch() }
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
        // not while a new chat's first message waits on its worktree: kept now, the chat would take an id
        // of its own and the worktree made for it would be thrown away with the message left running (#13771)
        if !run.preparingWorktree, cd_save_due(page) != 0 { keep(); wake() }
        var sides = [Float](repeating: 0, count: 2)
        if cd_sides(page, &sides) != 0 { UserDefaults.standard.set(sides[0], forKey: "sideWidth"); UserDefaults.standard.set(sides[1], forKey: "paneWidth") }
        else if let text = then, cd_running(page) == 0 { then = nil; newChat(); act(cd_action(page)); if cd_set_text(page, text) != 0 { _ = cd_submit(page) } }
        // the page's fields show a caret while the view has the keyboard
        let active = window.isKeyWindow && window.firstResponder === canvas
        let ws = NSWorkspace.shared
        cd_env(page, active ? 1 : 0, (ws.accessibilityDisplayShouldReduceMotion ? 1 : 0) | (ws.accessibilityDisplayShouldReduceTransparency ? 2 : 0))
        if cd_moving(page) != 0 || owed > 0 {
            cd_frame(page); if owed > 0 { owed -= 1 }
            // the caret's next phase: one frame then (the page's clock is the system's uptime, as CACurrentMediaTime)
            let b = cd_blink(page); blinkAt = b >= 0 ? CACurrentMediaTime() + Double(b) + 0.002 : 0
        }
        syncScenePanelFrame()
        syncGymPanelFrame()
    }

    /// Whether quitting leaves the running messages going: the daemon runs their workers.
    var runsOutliveQuit: Bool { CoderWorker.daemon?.ownsRuns == true }
    var runningCount: Int {
        (cd_running(page) != 0 ? 1 : 0) + runs.values.filter { cd_running($0.page) != 0 }.count
    }
    func show() { window.makeKeyAndOrderFront(nil); wake() }
    func windowShouldClose(_ sender: NSWindow) -> Bool { cancelCmdHold(); window.orderOut(nil); return false }
    func windowDidResignKey(_ notification: Notification) { cancelCmdHold() }
    private func setCmdHints(_ shown: Bool) {
        guard cmdHintsShown != shown else { return }
        cmdHintsShown = shown
        cd_cmd_hints(page, shown ? 1 : 0)
        wake()
    }
    private func cancelCmdHold() {
        cmdHoldTimer?.invalidate()
        cmdHoldTimer = nil
        setCmdHints(false)
    }
    private func handleModifierFlags(_ flags: NSEvent.ModifierFlags) {
        let mods = flags.intersection(.deviceIndependentFlagsMask)
        let cmdOnly = mods.contains(.command) && !mods.contains(.shift) && !mods.contains(.option) && !mods.contains(.control)
        guard cmdOnly, window.isKeyWindow, window.attachedSheet == nil else {
            cancelCmdHold()
            return
        }
        if !cmdHintsShown && cmdHoldTimer == nil {
            cmdHoldTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: false) { [weak self] _ in
                guard let self else { return }
                self.cmdHoldTimer = nil
                let live = NSEvent.modifierFlags.intersection(.deviceIndependentFlagsMask)
                let stillCmd = live.contains(.command) && !live.contains(.shift) && !live.contains(.option) && !live.contains(.control)
                if stillCmd && self.window.isKeyWindow && self.window.attachedSheet == nil {
                    self.setCmdHints(true)
                } else {
                    self.cancelCmdHold()
                }
            }
        }
    }
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
        if let monitor { NSEvent.removeMonitor(monitor) }; monitor = nil; link?.invalidate(); link = nil; watchdog?.invalidate(); watchdog = nil
        cmdHoldTimer?.invalidate(); cmdHoldTimer = nil
        if let flagsMonitor { NSEvent.removeMonitor(flagsMonitor) }; flagsMonitor = nil
        if let keyMonitor { NSEvent.removeMonitor(keyMonitor) }; keyMonitor = nil
        titleBar?.stop(); titleBar = nil
        cd_on_change(page, nil, nil); cd_close(page)
    }
}
