// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The app delegate, the status item, the main menu and main.
import AppKit

final class CoderDelegate: NSObject, NSApplicationDelegate {
    var main: CoderWindow?
    private var statusItem: NSStatusItem?
    private var opened: [URL] = []   // files the Mac asked Coder to open before its window was up
    var accountStatusItem: NSMenuItem?
    var accountItems: [NSMenuItem] = []   // OpenAgents > Log In… and the rest: an experimental feature
    var foremanMenuItem: NSMenuItem?      // the Foreman menu: an experimental feature
    func applicationDidFinishLaunching(_ notification: Notification) {
        DefaultsMigration.migrateIfNeeded()
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path, let main = CoderWindow(resources: resources) else {
            fputs("OpenAgents: the GPU surface could not start.\n", stderr); exit(1)
        }
        self.main = main
        main.installPaletteKeys()
        TestLaunch.start()
        CoderTailnet.shared.start()   // a phone on your tailnet pairs with no code (Tailnet.swift, #13778)
        MacPush.delegateForwards = true
        MacAccount.shared.changed = { [weak self] in
            DispatchQueue.main.async { self?.refreshAccountMenu() }
        }
        applyExperimental(Experimental.on)
        let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = statusItem.button {
            let image = NSImage(systemSymbolName: "terminal", accessibilityDescription: "OpenAgents")
            image?.isTemplate = true
            button.image = image
        }
        let menu = NSMenu()
        let showItem = menu.addItem(withTitle: "Show OpenAgents", action: #selector(showWindow), keyEquivalent: "")
        showItem.target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit OpenAgents", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        statusItem.menu = menu
        self.statusItem = statusItem
        NSApp.activate(ignoringOtherApps: true)
        let early = opened; opened = []
        for url in early { main.openFileInPane(url) }
    }
    // A .coax file (or another text file) opened with Coder from the Finder or `open` (#13759): it
    // opens in the pane on the window's right. Several go one after another; the last one shows.
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls where !url.isFileURL && Experimental.on {
            _ = MacAccount.shared.handleUniversalLink(url)
        }
        let files = urls.filter { $0.isFileURL }
        guard !files.isEmpty else { return }
        guard let main else { opened += files; return }
        NSApp.activate(ignoringOtherApps: true)
        main.show()
        for url in files { main.openFileInPane(url) }
    }
    func application(_ application: NSApplication, continue userActivity: NSUserActivity, restorationHandler: @escaping ([any NSUserActivityRestoring]) -> Void) -> Bool {
        guard Experimental.on, userActivity.activityType == NSUserActivityTypeBrowsingWeb, let url = userActivity.webpageURL else { return false }
        return MacAccount.shared.handleUniversalLink(url)
    }
    func application(_ application: NSApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        MacPush.shared.application(application, didRegisterForRemoteNotificationsWithDeviceToken: deviceToken)
    }
    func application(_ application: NSApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        MacPush.shared.application(application, didFailToRegisterForRemoteNotificationsWithError: error)
    }
    /// Experimental features (Settings > General) on or off: the account's and Foreman's menus show or
    /// hide, and the account is asked about only while they are on. Off, nothing logs in or asks to.
    func applyExperimental(_ on: Bool) {
        for item in accountItems { item.isHidden = !on }
        foremanMenuItem?.isHidden = !on
        if on { Task.detached { await MacAccount.shared.look() } }
    }
    @MainActor func refreshAccountMenu() {
        let title = MacAccount.shared.title
        accountStatusItem?.title = title.isEmpty ? "Log In to OpenAgents…" : title
    }
    @objc func accountLogin(_ sender: Any?) { guard Experimental.on else { return }; Task { @MainActor in await MacAccount.shared.showLogin() } }
    @objc func accountSecurity(_ sender: Any?) { Task { @MainActor in await MacAccount.shared.showSecurity() } }
    @objc func accountLinkDevice(_ sender: Any?) { Task { @MainActor in await MacAccount.shared.linkDeviceKey() } }
    @objc func accountLogout(_ sender: Any?) { Task.detached { await MacAccount.shared.logout() } }
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
        if main?.openFileDirty == true, let main {
            // the file in the pane has changes: Save, Don't Save or Cancel first
            NSApp.activate(ignoringOtherApps: true)
            main.show()
            // the answer closes the file (or Cancel keeps Coder open); the quit then starts over
            main.askToSave(then: {
                main.dropFile()
                NSApp.reply(toApplicationShouldTerminate: false)
                DispatchQueue.main.async { NSApp.terminate(nil) }
            }, cancelled: { NSApp.reply(toApplicationShouldTerminate: false) })
            return .terminateLater
        }
        guard let count = main?.runningCount, count > 0 else { return .terminateNow }
        // the daemon keeps them running; the next Coder attaches to them (GitHub #13694)
        if main?.runsOutliveQuit == true { return .terminateNow }
        NSApp.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.alertStyle = .warning
        alert.messageText = count == 1 ? "An agent task is still running" : "\(count) agent tasks are still running"
        alert.informativeText = "Quitting OpenAgents will stop all running tasks. Close the window instead to keep them running in the background."
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Quit")
        return alert.runModal() == .alertSecondButtonReturn ? .terminateNow : .terminateCancel
    }
    func applicationWillTerminate(_ notification: Notification) { main?.retire(); TestLaunch.stopHost() }
    @objc func newChat(_ sender: Any?) { main?.newChat() }
    @objc func search(_ sender: Any?) { main?.togglePalette() }
    @objc func toggleSidebar(_ sender: Any?) { main?.toggleSidebar() }
    @objc func chooseModel(_ sender: Any?) { main?.toggleModels() }
    @objc func nextChat(_ sender: Any?) { main?.stepChat(1) }
    @objc func previousChat(_ sender: Any?) { main?.stepChat(-1) }
    @objc func openFolder(_ sender: Any?) { main?.chooseFolder() }
    @objc func openScene(_ sender: Any?) { main?.chooseSceneFile() }
    @objc func saveFile(_ sender: Any?) { main?.saveOpenFile() }
    @objc func renameChat(_ sender: Any?) { main?.renameShown() }
    @objc func deleteChat(_ sender: Any?) { main?.deleteShown() }
    @objc func followUpOther(_ sender: Any?) { main?.followUpOther() }
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
    // The old dialogs' items open their settings pages, at their rows.
    @objc func claudeKey(_ sender: Any?) { main?.openSettings(1, row: "set.claude") }
    @objc func openRouterModels(_ sender: Any?) { main?.openSettings(1, row: "set.or") }
    @objc func checkForUpdates(_ sender: Any?) { main?.openSettings(4, row: "set.update"); main?.checkForUpdates() }
    @objc func openSettings(_ sender: Any?) { main?.openSettings() }
    @objc func backToChats(_ sender: Any?) { main?.closeSettings() }
    @objc func findInSettings(_ sender: Any?) { main?.findInSettings() }
    // Open Folder is off while the shown chat's reply runs, as its button is. New Chat is always on:
    // a running reply goes on out of view.
    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(accountLogin(_:)) { return Experimental.on }
        if item.action == #selector(accountSecurity(_:)) || item.action == #selector(accountLinkDevice(_:)) || item.action == #selector(accountLogout(_:)) {
            return Experimental.on && MacAccount.shared.loggedIn
        }
        if item.action == #selector(renameChat(_:)) { return main?.canRename == true }
        if item.action == #selector(deleteChat(_:)) { return main?.canDelete == true }
        if item.action == #selector(openFolder(_:)) { return main?.canFolder == true }
        if item.action == #selector(saveFile(_:)) { return main?.canSaveOpenFile == true }
        if item.action == #selector(followUpOther(_:)) {
            guard let w = main else { return false }
            item.title = "Queue a Follow-up"
            return cd_running(w.page) != 0
        }
        if item.action == #selector(openForeman(_:)) { return main?.isForemanEnabled == true }
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
        if item.action == #selector(backToChats(_:)) || item.action == #selector(findInSettings(_:)) { return main?.settingsOpen == true }
        return true
    }
}

/// A scratch test launch (GitHub #13752): a copy a test driver started with CODER_TEST_LAUNCH set (the
/// Tier 3 probe sets it), never a Coder in /Applications. It cannot outlive its driver: it stops the
/// `coder-host serve` it started (by that PID) when it quits or gets SIGTERM, and it quits itself when no
/// driver has spoken for CODER_TEST_IDLE_S seconds (default 600). A driver speaks by posting
/// CoderTestDriverActivity; a launch with no driver quits that long after it starts.
enum TestLaunch {
    static let active: Bool = {
        guard let v = ProcessInfo.processInfo.environment["CODER_TEST_LAUNCH"], !v.isEmpty else { return false }
        return !Bundle.main.bundlePath.hasPrefix("/Applications/")
    }()
    private static var lastDriver = Date()
    private static var timer: Timer?
    private static var sigterm: DispatchSourceSignal?

    static func start() {
        guard active, timer == nil else { return }
        let idle = max(5, Double(ProcessInfo.processInfo.environment["CODER_TEST_IDLE_S"] ?? "") ?? 600)
        NotificationCenter.default.addObserver(forName: Notification.Name("CoderTestDriverActivity"), object: nil, queue: .main) { _ in
            lastDriver = Date()
        }
        timer = Timer.scheduledTimer(withTimeInterval: min(30, idle / 4), repeats: true) { _ in
            guard Date().timeIntervalSince(lastDriver) > idle else { return }
            fputs("Coder: test launch idle for \(Int(idle)) s with no driver; quitting (CODER_TEST_IDLE_S).\n", stderr)
            stopHost()
            exit(0)   // not terminate(): a running reply would ask first in a modal alert no one answers
        }
        // SIGTERM (a driver killing its launch): stop the daemon, then die of SIGTERM as before
        signal(SIGTERM, SIG_IGN)
        let src = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .global(qos: .userInitiated))
        src.setEventHandler {
            stopHost()
            signal(SIGTERM, SIG_DFL)
            kill(getpid(), SIGTERM)
        }
        src.resume()
        sigterm = src
    }

    static func stopHost() {
        guard active else { return }
        let pid = CoderDaemonClient.spawnedPID
        guard pid > 0 else { return }
        CoderDaemonClient.spawnedPID = 0
        kill(pid, SIGTERM)
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
        file.addItem(withTitle: "Open 3D Scene…", action: #selector(CoderDelegate.openScene(_:)), keyEquivalent: "").target = delegate
        file.addItem(withTitle: "Search…", action: #selector(CoderDelegate.search(_:)), keyEquivalent: "k").target = delegate
        file.addItem(withTitle: "Save", action: #selector(CoderDelegate.saveFile(_:)), keyEquivalent: "s").target = delegate
        file.addItem(.separator())
        file.addItem(withTitle: "Rename Chat…", action: #selector(CoderDelegate.renameChat(_:)), keyEquivalent: "").target = delegate
        let del = file.addItem(withTitle: "Delete Chat…", action: #selector(CoderDelegate.deleteChat(_:)), keyEquivalent: "\u{8}")
        del.target = delegate; del.keyEquivalentModifierMask = [.command]
        // while a reply runs, Return steers and Shift-Return queues (#13717)
        file.addItem(.separator())
        let other = file.addItem(withTitle: "Queue a Follow-up", action: #selector(CoderDelegate.followUpOther(_:)), keyEquivalent: "\r")
        other.target = delegate; other.keyEquivalentModifierMask = [.shift]
        fileItem.submenu = file
        let viewItem = NSMenuItem(), view = NSMenu(title: "View")
        let actual = view.addItem(withTitle: "Actual Size", action: #selector(CoderDelegate.actualSize(_:)), keyEquivalent: "0")
        actual.target = delegate; actual.keyEquivalentModifierMask = [.command]
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
        view.addItem(.separator())
        view.addItem(withTitle: "Toggle Sidebar", action: #selector(CoderDelegate.toggleSidebar(_:)), keyEquivalent: "b").target = delegate
        view.addItem(withTitle: "Choose Model", action: #selector(CoderDelegate.chooseModel(_:)), keyEquivalent: "/").target = delegate
        // Control-Tab is heard before the composer takes it (Window+Palette.swift); these name the keys
        let next = view.addItem(withTitle: "Next Chat", action: #selector(CoderDelegate.nextChat(_:)), keyEquivalent: "\t")
        next.target = delegate; next.keyEquivalentModifierMask = [.control]
        let previous = view.addItem(withTitle: "Previous Chat", action: #selector(CoderDelegate.previousChat(_:)), keyEquivalent: "\t")
        previous.target = delegate; previous.keyEquivalentModifierMask = [.control, .shift]
        view.addItem(.separator())
        view.addItem(withTitle: "Back to Chats", action: #selector(CoderDelegate.backToChats(_:)), keyEquivalent: "[").target = delegate
        viewItem.submenu = view
        appMenu.addItem(withTitle: "Settings…", action: #selector(CoderDelegate.openSettings(_:)), keyEquivalent: ",").target = delegate
        // the account's items: an experimental feature (CoderDelegate.applyExperimental shows or hides them)
        let acctSeparator = NSMenuItem.separator(); appMenu.addItem(acctSeparator)
        let acctItem = appMenu.addItem(withTitle: "Log In to OpenAgents…", action: #selector(CoderDelegate.accountLogin(_:)), keyEquivalent: "")
        acctItem.target = delegate
        delegate.accountStatusItem = acctItem
        let security = appMenu.addItem(withTitle: "Account Security…", action: #selector(CoderDelegate.accountSecurity(_:)), keyEquivalent: "")
        security.target = delegate
        let link = appMenu.addItem(withTitle: "Link This Mac's Key…", action: #selector(CoderDelegate.accountLinkDevice(_:)), keyEquivalent: "")
        link.target = delegate
        let logout = appMenu.addItem(withTitle: "Log Out", action: #selector(CoderDelegate.accountLogout(_:)), keyEquivalent: "")
        logout.target = delegate
        delegate.accountItems = [acctSeparator, acctItem, security, link, logout]
        for item in delegate.accountItems { item.isHidden = !Experimental.on }
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Claude Key…", action: #selector(CoderDelegate.claudeKey(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(withTitle: "OpenRouter Models…", action: #selector(CoderDelegate.openRouterModels(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(withTitle: "Check for Updates…", action: #selector(CoderDelegate.checkForUpdates(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Hide OpenAgents", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit OpenAgents", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "Z")
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        edit.addItem(.separator())
        edit.addItem(withTitle: "Find in Settings", action: #selector(CoderDelegate.findInSettings(_:)), keyEquivalent: "f").target = delegate
        let foremanItem = NSMenuItem(), foremanMenu = NSMenu(title: "Foreman")
        // Command-` is also heard in Window.swift before macOS window cycling.
        let openForemanItem = foremanMenu.addItem(withTitle: "Open Foreman", action: #selector(CoderDelegate.openForeman(_:)), keyEquivalent: "`")
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
        delegate.foremanMenuItem = foremanItem; foremanItem.isHidden = !Experimental.on
        appItem.submenu = appMenu; editItem.submenu = edit; bar.addItem(appItem); bar.addItem(fileItem); bar.addItem(editItem); bar.addItem(viewItem); bar.addItem(foremanItem)
        app.mainMenu = bar
        app.run()
    }
}
