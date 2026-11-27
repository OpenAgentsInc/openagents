// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The app delegate, the status item, the main menu and main.
import AppKit

final class CoderDelegate: NSObject, NSApplicationDelegate {
    var main: CoderWindow?
    var extraWindows: [CoderWindow] = []
    private var statusItem: NSStatusItem?
    private var opened: [URL] = []   // files & openagents:// URLs the Mac asked Coder to open before its window was up
    var accountStatusItem: NSMenuItem?
    var accountItems: [NSMenuItem] = []   // OpenAgents > Log In… and the rest: gated on .account (default ON; #14087)
    var foremanMenuItem: NSMenuItem?      // the Foreman menu: gated on .foreman
    var gatedMenuItems: [(NSMenuItem, () -> Bool)] = []
    func applicationDidFinishLaunching(_ notification: Notification) {
        DefaultsMigration.migrateIfNeeded()
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path, let main = CoderWindow(resources: resources) else {
            fputs("OpenAgents: the GPU surface could not start.\n", stderr); exit(1)
        }
        self.main = main
        main.installPaletteKeys()
        DesktopActivation.shared.start(delegate: self)
        if FeatureFlags.isOn(.scheduled) {
            ScheduledTaskManager.shared.start(delegate: self)
        }
        TestLaunch.start()
        MacPush.delegateForwards = true
        MacAccount.shared.changed = { [weak self] in
            DispatchQueue.main.async {
                self?.refreshAccountMenu()
                self?.main?.refreshAccountCredits()
                self?.main?.refreshCloudChatSync()
            }
        }
        if FeatureFlags.isOn(.pairing) {
            CoderTailnet.shared.start()   // a phone on your tailnet pairs with no code (Tailnet.swift, #13778)
        }
        applyFlags()
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
        for url in early {
            if url.scheme?.lowercased() == "openagents" {
                _ = DesktopActivation.shared.handleURL(url)
            } else if url.isFileURL {
                main.openNamedFile(url)
            }
        }
    }
    // A .coax file (or another text file) opened with Coder from the Finder or `open` (#13759),
    // or a custom `openagents://` protocol deep link (#13889).
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls where url.scheme?.lowercased() == "openagents" {
            if main == nil { opened.append(url) } else { _ = DesktopActivation.shared.handleURL(url) }
        }
        for url in urls where !url.isFileURL && url.scheme?.lowercased() != "openagents" && FeatureFlags.isOn(.account) {
            _ = MacAccount.shared.handleUniversalLink(url)
        }
        let files = urls.filter { $0.isFileURL }
        guard !files.isEmpty else { return }
        guard let main else { opened += files; return }
        NSApp.activate(ignoringOtherApps: true)
        main.show()
        for url in files { main.openNamedFile(url) }
    }
    func openNewWindow(folderPath: String? = nil) {
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path,
              let win = CoderWindow(resources: resources) else { return }
        win.installPaletteKeys()
        if let folderPath, !folderPath.isEmpty {
            win.openFolderURL(URL(fileURLWithPath: folderPath))
        }
        extraWindows.append(win)
        NSApp.activate(ignoringOtherApps: true)
        win.show()
    }
    @objc func newWindow(_ sender: Any?) { openNewWindow() }
    @objc func toggleQuitHold(_ sender: Any?) {
        DesktopActivation.quitHoldEnabled.toggle()
    }
    @objc func toggleCliShim(_ sender: Any?) {
        let res = DesktopActivation.isShimInstalled() ? DesktopActivation.uninstallShim() : DesktopActivation.installShim()
        let alert = NSAlert()
        alert.alertStyle = res.ok ? .informational : .warning
        alert.messageText = res.ok ? "CLI Shim Updated" : "CLI Shim Error"
        alert.informativeText = res.message
        alert.runModal()
    }
    func application(_ application: NSApplication, continue userActivity: NSUserActivity, restorationHandler: @escaping ([any NSUserActivityRestoring]) -> Void) -> Bool {
        guard FeatureFlags.isOn(.account), userActivity.activityType == NSUserActivityTypeBrowsingWeb, let url = userActivity.webpageURL else { return false }
        return MacAccount.shared.handleUniversalLink(url)
    }
    func application(_ application: NSApplication, didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data) {
        MacPush.shared.application(application, didRegisterForRemoteNotificationsWithDeviceToken: deviceToken)
    }
    func application(_ application: NSApplication, didFailToRegisterForRemoteNotificationsWithError error: Error) {
        MacPush.shared.application(application, didFailToRegisterForRemoteNotificationsWithError: error)
    }
    @MainActor func refreshAccountMenu() {
        let title = MacAccount.shared.title
        accountStatusItem?.title = title.isEmpty ? "Log In to OpenAgents…" : title
        main?.syncCloudAuth()
    }
    @objc func accountLogin(_ sender: Any?) { guard FeatureFlags.isOn(.account) else { return }; Task { @MainActor in await MacAccount.shared.showLogin() } }
    @objc func accountSecurity(_ sender: Any?) { guard FeatureFlags.isOn(.account) else { return }; Task { @MainActor in await MacAccount.shared.showSecurity() } }
    @objc func accountLinkDevice(_ sender: Any?) { guard FeatureFlags.isOn(.account) else { return }; Task { @MainActor in await MacAccount.shared.linkDeviceKey() } }
    @objc func accountLogout(_ sender: Any?) { guard FeatureFlags.isOn(.account) else { return }; Task.detached { await MacAccount.shared.logout() } }
    /// Applies the resolved 30-flag mask to all menu items and delegate-owned background services.
    func applyFlags() {
        let acctOn = FeatureFlags.isOn(.account)
        for item in accountItems { item.isHidden = !acctOn }
        foremanMenuItem?.isHidden = !FeatureFlags.isOn(.foreman)
        for (item, check) in gatedMenuItems { item.isHidden = !check() }
        if acctOn { Task.detached { await MacAccount.shared.look() } }
        if FeatureFlags.isOn(.scheduled) {
            ScheduledTaskManager.shared.start(delegate: self)
        } else {
            ScheduledTaskManager.shared.stop()
        }
    }
    func applyExperimental(_ on: Bool) {
        applyFlags()
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
        // Optional Hold-to-Quit / double-press Cmd+Q protection (#13889)
        if !DesktopActivation.shared.shouldAllowQuit(runningAgents: main?.runningCount ?? 0) {
            return .terminateCancel
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
    func applicationWillTerminate(_ notification: Notification) {
        KeepAwakeManager.shared.release()
        let keepLocalServer = (main?.runsOutliveQuit == true) && ((main?.runningCount ?? 0) > 0)
        DesktopActivation.shared.stop()
        ScheduledTaskManager.shared.stop()
        main?.retire()
        for w in extraWindows { w.retire() }
        LocalModelServer.shared.stopIfIdle(runsOutlive: keepLocalServer)
        TestLaunch.stopHost()
    }
    @objc func newChat(_ sender: Any?) { main?.newChat() }
    @objc func openDashboard(_ sender: Any?) { main?.openDashboard() }
    @objc func search(_ sender: Any?) { main?.togglePalette() }
    @objc func toggleSidebar(_ sender: Any?) { main?.toggleSidebar() }
    @objc func editUndo(_ sender: Any?) { if main?.performEditChord(Int32(UnicodeScalar("z").value), mods: 8) != true { NSApp.sendAction(Selector(("undo:")), to: nil, from: sender) } }
    @objc func editRedo(_ sender: Any?) { if main?.performEditChord(Int32(UnicodeScalar("z").value), mods: 9) != true { NSApp.sendAction(Selector(("redo:")), to: nil, from: sender) } }
    @objc func editCut(_ sender: Any?) { if main?.performEditChord(Int32(UnicodeScalar("x").value), mods: 8) != true { NSApp.sendAction(#selector(NSText.cut(_:)), to: nil, from: sender) } }
    @objc func editCopy(_ sender: Any?) { if main?.performEditChord(Int32(UnicodeScalar("c").value), mods: 8) != true { NSApp.sendAction(#selector(NSText.copy(_:)), to: nil, from: sender) } }
    @objc func editPaste(_ sender: Any?) { if main?.performEditChord(Int32(UnicodeScalar("v").value), mods: 8) != true { NSApp.sendAction(#selector(NSText.paste(_:)), to: nil, from: sender) } }
    @objc func editSelectAll(_ sender: Any?) { if main?.performEditChord(Int32(UnicodeScalar("a").value), mods: 8) != true { NSApp.sendAction(#selector(NSText.selectAll(_:)), to: nil, from: sender) } }
    @objc func toggleSimulator(_ sender: Any?) { guard FeatureFlags.isOn(.simulator) else { return }; main?.toggleSimulator() }
    @objc func toggleTerminal(_ sender: Any?) { guard FeatureFlags.isOn(.terminal) else { return }; main?.toggleTerminal() }
    @objc func toggleWebPreview(_ sender: Any?) { guard FeatureFlags.isOn(.preview) else { return }; main?.toggleWebPreview() }
    @objc func toggleMetaverse(_ sender: Any?) { guard FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) else { return }; main?.toggleMetaversePane() }
    @objc func toggleMetaverseExpand(_ sender: Any?) { guard FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) else { return }; main?.toggleMetaverseExpanded() }
    @objc func openGameBuilder(_ sender: Any?) { guard FeatureFlags.isOn(.gamebuilder) else { return }; main?.openGameBuilderStudio() }
    @objc func openGtaWorkbench(_ sender: Any?) { guard FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) else { return }; main?.openGtaWorkbench() }
    @objc func triggerSnapShot(_ sender: Any?) { guard FeatureFlags.isOn(.preview) else { return }; main?.triggerRegionSnapshot() }
    @objc func openThreadProof(_ sender: Any?) { guard FeatureFlags.isOn(.proof) else { return }; ThreadProofDrawer.shared.present(window: main) }
    @objc func openAgentsPage(_ sender: Any?) { guard FeatureFlags.isOn(.agents) else { return }; AgentsPageController.shared.present(window: main) }
    @objc func openCapabilityStudio(_ sender: Any?) { guard FeatureFlags.isOn(.capabilities) else { return }; CapabilityStudio.shared.present(window: main) }
    @objc func openScene(_ sender: Any?) { guard FeatureFlags.isOn(.scenes) else { return }; main?.chooseSceneFile() }
    @objc func chooseModel(_ sender: Any?) { main?.toggleModels() }
    @objc func nextChat(_ sender: Any?) { main?.stepChat(1) }
    @objc func previousChat(_ sender: Any?) { main?.stepChat(-1) }
    @objc func openFolder(_ sender: Any?) { main?.chooseFolder() }
    @objc func saveFile(_ sender: Any?) { main?.saveOpenFile() }
    @objc func renameChat(_ sender: Any?) { main?.renameShown() }
    @objc func forkChat(_ sender: Any?) { main?.forkShown() }
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
    @objc func viewForemanAuditLog(_ sender: Any?) { main?.openForemanAuditLog() }
    @objc func clearForemanHistory(_ sender: Any?) { main?.clearForemanHistory() }
    // The old dialogs' items open their settings pages, at their rows.
    @objc func claudeKey(_ sender: Any?) { guard FeatureFlags.isOn(.modelsClaude) else { return }; main?.openSettings(1, row: "set.claude") }
    @objc func openRouterModels(_ sender: Any?) { guard FeatureFlags.isOn(.modelsOpenrouter) else { return }; main?.openSettings(1, row: "set.or") }
    @objc func openScheduledTasks(_ sender: Any?) { guard FeatureFlags.isOn(.scheduled) else { return }; ScheduledTaskManager.shared.showScheduledTasksDialog() }
    @objc func openUpdateEverything(_ sender: Any?) { guard FeatureFlags.isOn(.scheduled) else { return }; ScheduledTaskManager.shared.showUpdateEverythingDialog() }
    @objc func stopAllChats(_ sender: Any?) { main?.stopAllChats() }
    @objc func checkForUpdates(_ sender: Any?) { main?.openSettings(4, row: "set.update"); main?.checkForUpdates() }
    @objc func openSettings(_ sender: Any?) { main?.openSettings() }
    @objc func backToChats(_ sender: Any?) { main?.closeSettings() }
    @objc func findInSettings(_ sender: Any?) { main?.openFind() }
    @objc func findNext(_ sender: Any?) { main?.findNext() }
    @objc func findPrev(_ sender: Any?) { main?.findPrev() }
    // Open Folder is off while the shown chat's reply runs, as its button is. New Chat is always on:
    // a running reply goes on out of view.
    @objc func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(accountLogin(_:)) { return FeatureFlags.isOn(.account) }
        if item.action == #selector(accountSecurity(_:)) || item.action == #selector(accountLinkDevice(_:)) || item.action == #selector(accountLogout(_:)) {
            return FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn
        }
        if item.action == #selector(openScene(_:)) { return FeatureFlags.isOn(.scenes) }
        if item.action == #selector(toggleSimulator(_:)) { return FeatureFlags.isOn(.simulator) }
        if item.action == #selector(toggleTerminal(_:)) { return FeatureFlags.isOn(.terminal) }
        if item.action == #selector(toggleWebPreview(_:)) { return FeatureFlags.isOn(.preview) }
        if item.action == #selector(toggleMetaverse(_:)) || item.action == #selector(toggleMetaverseExpand(_:)) || item.action == #selector(openGtaWorkbench(_:)) { return FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) }
        if item.action == #selector(openGameBuilder(_:)) { return FeatureFlags.isOn(.gamebuilder) }
        if item.action == #selector(triggerSnapShot(_:)) { return FeatureFlags.isOn(.preview) }
        if item.action == #selector(openThreadProof(_:)) { return FeatureFlags.isOn(.proof) }
        if item.action == #selector(openAgentsPage(_:)) { return FeatureFlags.isOn(.agents) }
        if item.action == #selector(openCapabilityStudio(_:)) { return FeatureFlags.isOn(.capabilities) }
        if item.action == #selector(claudeKey(_:)) { return FeatureFlags.isOn(.modelsClaude) }
        if item.action == #selector(openRouterModels(_:)) { return FeatureFlags.isOn(.modelsOpenrouter) }
        if item.action == #selector(openScheduledTasks(_:)) || item.action == #selector(openUpdateEverything(_:)) { return FeatureFlags.isOn(.scheduled) }
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
        if item.action == #selector(renameChat(_:)) { return main?.canRename == true }
        if item.action == #selector(forkChat(_:)) { return main?.canFork == true }
        if item.action == #selector(deleteChat(_:)) { return main?.canDelete == true }
        if item.action == #selector(openFolder(_:)) { return main?.canFolder == true }
        if item.action == #selector(saveFile(_:)) { return main?.canSaveOpenFile == true }
        if item.action == #selector(followUpOther(_:)) {
            guard let w = main else { return false }
            item.title = "Queue a Follow-up"
            return cd_running(w.page) != 0
        }
        if item.action == #selector(stopAllChats(_:)) { return (main?.runningCount ?? 0) > 0 }
        if item.action == #selector(backToChats(_:)) { return main?.settingsOpen == true }
        if item.action == #selector(findInSettings(_:)) { return main != nil }
        if item.action == #selector(findNext(_:)) || item.action == #selector(findPrev(_:)) { return main?.findOpen == true }
        if item.action == #selector(toggleQuitHold(_:)) {
            item.state = DesktopActivation.quitHoldEnabled ? .on : .off
            return true
        }
        if item.action == #selector(toggleCliShim(_:)) {
            item.title = DesktopActivation.isShimInstalled() ? "Uninstall CLI Shim…" : "Install CLI Shim…"
            return true
        }
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
    /// Offscreen Metal capture of the Lean 4 & Riemann Hypothesis Proof Workbench (#13946).
    /// Renders directly into an offscreen MTLTexture with ZERO windows and ZERO focus stealing.
    static func runLeanRhShots(outDir: String) -> Int32 {
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path else {
            fputs("OpenAgents --shot-lean-rh: missing Components resources\n", stderr); return 1
        }
        let fm = FileManager.default
        try? fm.createDirectory(atPath: outDir, withIntermediateDirectories: true)
        guard let page = cd_open_ex(resources, 1, 1440, 900) else {
            fputs("OpenAgents --shot-lean-rh: cd_open_ex(offscreen=1) failed\n", stderr); return 1
        }
        cd_experimental_set(page, 1)
        cd_gym_set(page, 1)
        cd_gym_mode_set(page, 2)
        if let atlasData = try? Data(contentsOf: CoderWindow.rhAtlasFile),
           let atlasStr = String(data: atlasData, encoding: .utf8), !atlasStr.isEmpty {
            _ = cd_lean_atlas_set(page, atlasStr)
        }
        func runToolSync(_ args: [String]) -> String? {
            let bundled = Bundle.main.resourceURL?.appendingPathComponent("lean-run").path
            let shared = fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/lean-run").path
            let tool = (bundled != nil && fm.isExecutableFile(atPath: bundled!)) ? bundled! : (fm.isExecutableFile(atPath: shared) ? shared : nil)
            guard let exe = tool else { return nil }
            let p = Process()
            p.executableURL = URL(fileURLWithPath: exe)
            p.arguments = args
            let pipe = Pipe(); p.standardOutput = pipe; p.standardError = Pipe()
            guard (try? p.run()) != nil else { return nil }
            let d = pipe.fileHandleForReading.readDataToEndOfFile()
            p.waitUntilExit()
            let s = String(decoding: d, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            return s.isEmpty ? nil : s
        }
        if let doc = runToolSync(["doctor", "--json"]) { _ = cd_lean_doctor_set(page, doc) }
        if let aud = runToolSync(["rh", "audit", "--json"]) { _ = cd_lean_audit_set(page, aud) }
        if let mainMod = runToolSync(["rh", "module", "Main", "--json"]) { _ = cd_lean_module_set(page, mainMod) }
        if let hits = runToolSync(["rh", "search", "Behrend", "--limit", "16", "--json"]) { _ = cd_lean_search_set(page, hits) }

        func snap(_ name: String) -> Bool {
            for _ in 0..<4 { _ = cd_shot_frame(page, 1440, 900, 0.016) }
            let path = (outDir as NSString).appendingPathComponent(name)
            return cd_shot_write(page, path) != 0
        }
        let shots: [(String, String, Int32?)] = [
            ("01-tour.png", "{\"ok\":true,\"id\":\"s1\",\"open\":true,\"view\":\"tour\",\"tour_step\":1}", nil),
            ("02a-atlas-lod0.png", "{\"ok\":true,\"id\":\"s2a\",\"open\":true,\"view\":\"atlas\"}", 0),
            ("02b-atlas-lod1.png", "{\"ok\":true,\"id\":\"s2b\",\"open\":true,\"view\":\"atlas\"}", 1),
            ("03-math-strip.png", "{\"ok\":true,\"id\":\"s3\",\"open\":true,\"view\":\"math\",\"math_tab\":\"strip\"}", nil),
            ("04-math-robin.png", "{\"ok\":true,\"id\":\"s4\",\"open\":true,\"view\":\"math\",\"math_tab\":\"robin\"}", nil),
            ("05-math-primes.png", "{\"ok\":true,\"id\":\"s5\",\"open\":true,\"view\":\"math\",\"math_tab\":\"primes\"}", nil),
            ("06-inspect-main.png", "{\"ok\":true,\"id\":\"s6\",\"open\":true,\"view\":\"inspect\",\"module\":\"Main\"}", nil),
            ("07-trust.png", "{\"ok\":true,\"id\":\"s7\",\"open\":true,\"view\":\"trust\"}", nil),
            ("08-prover.png", "{\"ok\":true,\"id\":\"s8\",\"open\":true,\"view\":\"prover\"}", nil),
        ]
        var okCount = 0
        for (file, plan, lod) in shots {
            _ = cd_lean_apply_plan(page, plan)
            if let l = lod { cd_lean_atlas_lod_set(page, l) }
            if snap(file) { okCount += 1; print("wrote \(file)") }
        }
        if let behMod = runToolSync(["rh", "module", "Formalism.BehrendSets.FinalE", "--json"]) {
            _ = cd_lean_module_set(page, behMod)
            _ = cd_lean_apply_plan(page, "{\"ok\":true,\"id\":\"s2c\",\"open\":true,\"view\":\"atlas\",\"module\":\"Formalism.BehrendSets.FinalE\"}")
            cd_lean_atlas_lod_set(page, 2)
            if snap("02c-atlas-lod2.png") { okCount += 1; print("wrote 02c-atlas-lod2.png") }
            _ = cd_lean_apply_plan(page, "{\"ok\":true,\"id\":\"s9\",\"open\":true,\"view\":\"inspect\",\"module\":\"Formalism.BehrendSets.FinalE\"}")
            if snap("09-inspect-behrend.png") { okCount += 1; print("wrote 09-inspect-behrend.png") }
        }
        var stateBuf = [CChar](repeating: 0, count: 8192)
        if cd_lean_state_json(page, "shot", &stateBuf, Int32(stateBuf.count)) > 0 {
            print("state: \(String(cString: stateBuf))")
        }
        return okCount >= 8 ? 0 : 1
    }

    /// Headless offscreen Metal PNG capture of the Metaverse Engine 3D RH Flythrough (ZERO windows).
    static func runRh3dShots(outDir: String) -> Int32 {
        let fm = FileManager.default
        try? fm.createDirectory(atPath: outDir, withIntermediateDirectories: true)
        let view = RhFlythroughView(frame: NSRect(x: 0, y: 0, width: 1440, height: 900))
        let shots: [(String, Float?, Bool)] = [
            ("00-overview.png", nil, true),
            ("01-zeta-terrain.png", 0.0, false),
            ("02-primes.png", 7.0, false),
            ("03-robin-5040.png", 14.0, false),
            ("04-idea.png", 21.0, false),
            ("05-last-step.png", 28.0, false),
            ("06-proof-city.png", 35.0, false),
            ("07-checkers.png", 42.0, false),
            ("08-theorem.png", 49.0, false),
        ]
        var ok = 0
        for (name, t, ov) in shots {
            let url = URL(fileURLWithPath: outDir).appendingPathComponent(name)
            if view.renderOffscreenPNG(to: url, width: 1440, height: 900, timeSeconds: t, overview: ov) {
                print("wrote \(name)")
                ok += 1
            } else {
                fputs("failed \(name)\n", stderr)
            }
        }
        return ok == shots.count ? 0 : 1
    }

    private static func runRh3dRecord(outPath: String) -> Int32 {
        let app = NSApplication.shared
        app.setActivationPolicy(.prohibited)
        let view = RhFlythroughView(frame: NSRect(x: 0, y: 0, width: 1536, height: 864))
        let url = URL(fileURLWithPath: outPath)
        try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        return view.recordWalkthroughMP4(to: url, width: 1920, height: 1080, fps: 30) ? 0 : 1
    }

    /// Offscreen Metal PNG capture of Settings -> Providers and Add provider 3-step modal wizard (ZERO windows).
    /// Renders with flags == 0 (core basic flow, no feature flags enabled).
    static func runProvidersShots(outDir: String) -> Int32 {
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path else {
            fputs("OpenAgents --shot-providers: missing Components resources\n", stderr); return 1
        }
        let fm = FileManager.default
        try? fm.createDirectory(atPath: outDir, withIntermediateDirectories: true)
        guard let page = cd_open_ex(resources, 1, 1340, 880) else {
            fputs("OpenAgents --shot-providers: cd_open_ex(offscreen=1) failed\n", stderr); return 1
        }
        cd_flags_set(page, 0)
        cd_settings_open(page, 14, "")
        cd_ps_set_meta(page, "Christopher's MacBook Pro (2)", "All projects", "Checked 1h ago", 0)
        cd_ps_clear(page)
        _ = cd_ps_add_instance(page, "codex", 2, "Codex", "", 0, 1, 0, "Disabled", "", 0)
        _ = cd_ps_add_instance(page, "claude", 1, "Claude", "", 0, 1, 0, "Disabled", "", 0)
        _ = cd_ps_add_instance(page, "antigravity", 0, "Antigravity", "", 1, 1, 2, "Needs attention", "Antigravity is installed. Connect with Agent Platform to load models.", 0)
        cd_ps_select(page, 2)
        cd_ps_set_detail_config(page, "Antigravity", 0, 3, 0, "openagents-gemini", "us-central1", "", "")
        cd_ps_set_detail_runtime(page, 1, "Installed.", "Install managed runtime", -1, 0, "")
        cd_ps_set_detail_auth(page, 5, "Connect on Christopher's MacBook Pro (2) to load models.", "Antigravity could not authenticate with the configured credentials.", "Retry connection", 0, "")
        cd_ps_env_clear(page)
        cd_ps_models_clear(page)
        _ = cd_ps_models_add(page, "gemini-4-neon", "Gemini 4 Neon", 1, 1, 1, 0)
        _ = cd_ps_models_add(page, "gemini-4-carbon", "Gemini 4 Carbon", 1, 0, 1, 0)
        _ = cd_ps_models_add(page, "gemini-4-hydrogen", "Gemini 4 Hydrogen", 0, 0, 0, 0)

        func snap(_ name: String) -> Bool {
            cd_env(page, 1, 0)
            for _ in 0..<5 { _ = cd_shot_frame(page, 1340, 880, 0.016) }
            let path = (outDir as NSString).appendingPathComponent(name)
            return cd_shot_write(page, path) != 0
        }

        var ok = 0
        // 1. Upper view (matching Screenshot 3)
        cd_ps_ui_set(page, 0, 0, 0, 0)
        if snap("01-providers-top.png") { ok += 1; print("wrote 01-providers-top.png") }

        // 2. Scrolled down with GCP project focused (blue focus ring, matching Screenshot 2)
        cd_ps_ui_set(page, 0, 0, 448, 3)
        if snap("02-providers-scrolled.png") { ok += 1; print("wrote 02-providers-scrolled.png") }

        // 3. Inline OAuth waiting card + env var + Models list
        cd_ps_set_detail_auth(page, 3, "Finish sign-in in your browser on Christopher's MacBook Pro (2).", "", "Cancel", 0, "https://accounts.google.com/o/oauth2/v2/auth?client_id=antigravity")
        _ = cd_ps_env_add(page, "VERTEX_CUSTOM_ENDPOINT", "https://us-central1-aiplatform.googleapis.com", 0, 0)
        cd_ps_ui_set(page, 0, 0, 210, 0)
        if snap("03a-providers-oauth.png") { ok += 1; print("wrote 03a-providers-oauth.png") }
        cd_ps_ui_set(page, 0, 0, 760, 0)
        if snap("03-providers-models.png") { ok += 1; print("wrote 03-providers-models.png") }

        // 4. Add provider wizard Steps 1, 2, 3 (reset auth card so background matches Screenshot 1)
        cd_ps_set_detail_auth(page, 5, "Connect on Christopher's MacBook Pro (2) to load models.", "Antigravity could not authenticate with the configured credentials.", "Retry connection", 0, "")
        cd_ps_ui_set(page, 1, 0, 0, 0)
        if snap("04-wizard-step1.png") { ok += 1; print("wrote 04-wizard-step1.png") }
        cd_ps_ui_set(page, 1, 1, 0, 0)
        if snap("04b-wizard-step2.png") { ok += 1; print("wrote 04b-wizard-step2.png") }
        cd_ps_ui_set(page, 1, 2, 0, 0)
        if snap("05-wizard-step3.png") { ok += 1; print("wrote 05-wizard-step3.png") }

        cd_close(page)
        return ok == 7 ? 0 : 1
    }

    /// Headless offscreen end-to-end Claude Code sub-agent test on the real `--minimal` macOS app (#14104).
    /// Modes: "" (default read-only Sonnet), "off", "two", "stop", "file", "missing", "unauthed", "plan".
    /// Never opens a visible window or mutates UserDefaults; cleans up all workers & sub-agents on exit.
    static func runDelegateClaudeTest(mode: String, cwdArg: String?, modelArg: String?) -> Int32 {
        func fail(_ step: String, _ why: String) -> Int32 {
            fputs("FAIL [\(step)] \(why)\n", stderr)
            return 1
        }
        func ok(_ step: String, _ msg: String) {
            print("ok   [\(step)] \(msg)")
            fflush(stdout)
        }
        guard let resources = Bundle.main.resourceURL?.appendingPathComponent("Components").path else {
            return fail("bundle", "missing Components resources")
        }
        guard let claudeWorkerPath = Bundle.main.path(forResource: "claudeworker", ofType: nil),
              FileManager.default.isExecutableFile(atPath: claudeWorkerPath) else {
            return fail("bundle", "claudeworker is missing from Contents/Resources")
        }
        guard let oaWorkerPath = Bundle.main.path(forResource: "oa-worker", ofType: nil),
              FileManager.default.isExecutableFile(atPath: oaWorkerPath) else {
            return fail("bundle", "oa-worker is missing from Contents/Resources")
        }
        guard let gitguardPath = Bundle.main.path(forResource: "gitguard", ofType: nil),
              FileManager.default.isExecutableFile(atPath: gitguardPath) else {
            return fail("bundle", "gitguard is missing from Contents/Resources")
        }
        ok("bundle", "oa-worker, claudeworker, and gitguard bundled in --minimal app")

        func runClaudeWorkerOnce(env extra: [String: String], taskLine: String, timeout: TimeInterval) -> [String] {
            let p = Process(), into = Pipe(), out = Pipe()
            p.executableURL = URL(fileURLWithPath: claudeWorkerPath)
            var env = ProcessInfo.processInfo.environment
            env["SSH_CONNECTION"] = "1"
            env["BROWSER"] = "/usr/bin/true"
            env["NO_OPEN_BROWSER"] = "1"
            for (k, v) in extra { env[k] = v }
            p.environment = env
            p.standardInput = into
            p.standardOutput = out
            p.standardError = FileHandle.nullDevice
            do { try p.run() } catch { return [] }
            if let d = (taskLine + "\n").data(using: .utf8) {
                try? into.fileHandleForWriting.write(contentsOf: d)
            }
            let deadline = Date().addingTimeInterval(timeout)
            var buf = Data()
            var lines: [String] = []
            let fh = out.fileHandleForReading
            let fd = fh.fileDescriptor
            let flags = fcntl(fd, F_GETFL)
            _ = fcntl(fd, F_SETFL, flags | O_NONBLOCK)
            var tmp = [UInt8](repeating: 0, count: 8192)
            while Date() < deadline {
                let n = Darwin.read(fd, &tmp, tmp.count)
                if n > 0 {
                    buf.append(contentsOf: tmp[0..<n])
                    while let nl = buf.firstIndex(of: 0x0a) {
                        let one = buf.subdata(in: buf.startIndex..<nl)
                        buf.removeSubrange(buf.startIndex...nl)
                        if let s = String(data: one, encoding: .utf8), !s.isEmpty {
                            lines.append(s)
                            if s.contains("\"kind\":\"ended\"") {
                                try? into.fileHandleForWriting.close()
                                p.waitUntilExit()
                                return lines
                            }
                        }
                    }
                } else if !p.isRunning {
                    break
                } else {
                    Thread.sleep(forTimeInterval: 0.02)
                }
            }
            try? into.fileHandleForWriting.close()
            if p.isRunning { p.terminate(); Thread.sleep(forTimeInterval: 0.2) }
            return lines
        }

        // Direct live probes for missing CLI & not-signed-in error cases (never falls back to another model).
        if mode == "missing" {
            let fakeHome = FileManager.default.temporaryDirectory.appendingPathComponent("oa-claude-missing-\(UUID().uuidString)")
            try? FileManager.default.createDirectory(at: fakeHome, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: fakeHome) }
            let missingEnv = ["PATH": "/usr/bin:/bin", "HOME": fakeHome.path, "SSH_CONNECTION": "1", "BROWSER": "/usr/bin/true", "NO_OPEN_BROWSER": "1"]
            let authLine = CoderProbes.ask("claudeworker", ["--auth"], env: missingEnv, timeout: 6)
            guard authLine.contains("\"installed\":false") && authLine.contains("\"ready\":false") else {
                return fail("missing-probe", "unexpected --auth output: \(authLine)")
            }
            ok("missing-probe", authLine)
            let lines = runClaudeWorkerOnce(
                env: missingEnv,
                taskLine: "{\"task\":\"say hi\",\"session\":\"s1\",\"cwd\":\"\(fakeHome.path)\",\"sandbox\":\"danger-full-access\",\"policy\":{\"enabled\":false,\"allow\":0,\"ceiling\":3,\"roots\":\"\"},\"no_ask\":true,\"model\":\"haiku\",\"steerable\":false}",
                timeout: 15
            )
            guard let ended = lines.last(where: { $0.contains("\"kind\":\"ended\"") }),
                  ended.contains("\"ending\":\"failed\""),
                  ended.lowercased().contains("not installed") || ended.lowercased().contains("not found") else {
                return fail("missing-worker", "expected specific not-installed failure, got: \(lines)")
            }
            ok("missing-worker", ended)
            print("openagents-desktop-macos delegate-claude (missing): ok")
            return 0
        }

        if mode == "unauthed" {
            let emptyCfg = FileManager.default.temporaryDirectory.appendingPathComponent("oa-claude-unauthed-\(UUID().uuidString)")
            try? FileManager.default.createDirectory(at: emptyCfg, withIntermediateDirectories: true)
            defer { try? FileManager.default.removeItem(at: emptyCfg) }
            let unauthedEnv = [
                "PATH": CoderWorker.path,
                "CLAUDE_CONFIG_DIR": emptyCfg.path,
                "SSH_CONNECTION": "1",
                "BROWSER": "/usr/bin/true",
                "NO_OPEN_BROWSER": "1",
            ]
            let authLine = CoderProbes.ask("claudeworker", ["--auth"], env: unauthedEnv, timeout: 8)
            guard authLine.contains("\"installed\":true") && authLine.contains("\"logged_in\":false") && authLine.contains("\"ready\":false") else {
                return fail("unauthed-probe", "unexpected --auth output: \(authLine)")
            }
            ok("unauthed-probe", authLine)
            let lines = runClaudeWorkerOnce(
                env: unauthedEnv,
                taskLine: "{\"task\":\"Reply with HI\",\"session\":\"s2\",\"cwd\":\"\(emptyCfg.path)\",\"sandbox\":\"danger-full-access\",\"policy\":{\"enabled\":false,\"allow\":0,\"ceiling\":3,\"roots\":\"\"},\"no_ask\":true,\"model\":\"haiku\",\"steerable\":false}",
                timeout: 30
            )
            guard let ended = lines.last(where: { $0.contains("\"kind\":\"ended\"") }),
                  ended.contains("\"ending\":\"failed\""),
                  ended.lowercased().contains("not signed in") || ended.lowercased().contains("not logged in") || ended.lowercased().contains("login") || ended.lowercased().contains("auth") else {
                return fail("unauthed-worker", "expected specific not-signed-in failure, got: \(lines)")
            }
            ok("unauthed-worker", ended)
            print("openagents-desktop-macos delegate-claude (unauthed): ok")
            return 0
        }

        // Verify live `claudeworker --auth` probe on this Mac first.
        let authLine = CoderProbes.ask("claudeworker", ["--auth"], env: ["SSH_CONNECTION": "1", "BROWSER": "/usr/bin/true", "NO_OPEN_BROWSER": "1"], timeout: 8)
        guard let authData = authLine.data(using: .utf8),
              let authObj = try? JSONSerialization.jsonObject(with: authData) as? [String: Any],
              (authObj["ready"] as? Bool) == true else {
            return fail("claude-auth", "claudeworker --auth not ready: \(authLine)")
        }
        let claudeLabel = (authObj["label"] as? String) ?? "Signed in"
        ok("claude-auth", claudeLabel)

        // Prepare clean temp git repo cwd if none supplied.
        let ownedTmp: URL?
        let workDir: String
        if let c = cwdArg, !c.isEmpty {
            ownedTmp = nil
            workDir = c
        } else {
            let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("oa-delegate-mac-\(UUID().uuidString)")
            try? FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
            let gi = Process()
            gi.executableURL = URL(fileURLWithPath: "/usr/bin/git")
            gi.arguments = ["init", "-q", tmp.path]
            try? gi.run(); gi.waitUntilExit()
            try? "OpenAgents Claude sub-agent test fixture\nSECRET_TOKEN=CEDAR_4291\n".write(
                to: tmp.appendingPathComponent("README.md"), atomically: true, encoding: .utf8)
            ownedTmp = tmp
            workDir = tmp.path
        }
        defer { if let u = ownedTmp { try? FileManager.default.removeItem(at: u) } }

        guard let page = cd_open_ex(resources, 1, 1340, 880) else {
            return fail("page", "cd_open_ex(offscreen) failed")
        }
        defer { cd_close(page) }

        // Attach resident daemon if already running (exercises Worker.swift gitguard materialization too!).
        let daemon = CoderDaemonClient()
        if daemon.isDaemonAvailable {
            CoderWorker.daemon = daemon
            ok("daemon", "using resident coder-host daemon (verifies versioned gitguard materialization)")
        } else {
            CoderWorker.daemon = nil
            ok("daemon", "running workers directly (no resident daemon)")
        }

        // Determine parent model: explicit --model > cloud (if signed in / env key) > local Inkling.
        var workerEnv = AgentSettings.workerEnv()
        workerEnv["OA_ALLOW_LOCAL_HARNESSES"] = (mode == "off") ? "0" : "1"
        workerEnv["OA_AGENT_PERMISSION_MODE"] = (mode == "plan") ? "plan" : "allow_all"
        workerEnv["OA_AGENT_BYPASS_PERMISSIONS"] = "0"
        workerEnv["OA_FULL_AUTO"] = (mode == "plan") ? "0" : "1"
        let pickedModel: String
        if let m = modelArg, !m.isEmpty {
            pickedModel = m
        } else {
            let oaAuth = CoderProbes.ask("oa-worker", ["--auth"], env: workerEnv, timeout: 8)
            if oaAuth.contains("\"ready\":true") && (oaAuth.contains("Vertex") || oaAuth.contains("Gemini") || oaAuth.contains("OpenAgents Cloud") || oaAuth.contains("OpenRouter") || !oaAuth.contains("Local Psionic")) {
                pickedModel = "oa:cloud"
            } else {
                pickedModel = "oa:local"
            }
        }
        if pickedModel == "oa:local" {
            guard LocalModelStore.isSelectable else {
                return fail("local-model", "no cloud auth and Inkling 2 Mini weights are not installed")
            }
            var srvDone = false
            var srvErr: String? = nil
            LocalModelServer.shared.ensureRunning { err in
                srvErr = err
                srvDone = true
            }
            let srvDeadline = Date().addingTimeInterval(90)
            while !srvDone && Date() < srvDeadline {
                RunLoop.current.run(until: Date().addingTimeInterval(0.05))
            }
            if !srvDone {
                return fail("local-model", "psionic-inkling-serve timed out starting")
            }
            if let srvErr { return fail("local-model", srvErr) }
            workerEnv["PSIONIC_BASE_URL"] = LocalModelServer.baseURL
            workerEnv["PSIONIC_MODEL"] = LocalModelStore.entry.id
            workerEnv["PSIONIC_CTX"] = String(LocalModelStore.recommendedContextTokens)
        }
        ok("parent-model", pickedModel)

        // Configure page state without touching UserDefaults.
        _ = cd_shot_frame(page, 1340, 880, 1.0 / 60.0)
        var prefs = [Int32](repeating: 0, count: 32)
        let prefN = prefs.withUnsafeMutableBufferPointer { cd_flawed_prefs_get(page, $0.baseAddress, Int32($0.count)) }
        guard prefN >= 21 else { return fail("prefs", "page prefs vector < 21") }
        prefs[6] = (mode == "plan") ? 3 : 0
        prefs[7] = 0
        prefs[20] = (mode == "off") ? 0 : 1
        prefs.withUnsafeBufferPointer { cd_flawed_prefs_set(page, $0.baseAddress, prefN) }
        cd_claude_sub(page, 1)
        cd_claude_status(page, claudeLabel)
        cd_folder(page, workDir)
        guard cd_pick_model(page, pickedModel) != 0 else {
            return fail("model", "cd_pick_model(\(pickedModel)) failed")
        }

        let prompt: String
        switch mode {
        case "off":
            prompt = "Delegate this to Claude: reply with the word KIWI."
        case "two":
            prompt = "Call your Agent tool twice in the same message, both with subagent_type claude-haiku. The first prompt: Reply with exactly the word KIWI and nothing else. The second prompt: Reply with exactly the word PLUM and nothing else. Then tell me both words the sub-agents replied with."
        case "stop":
            prompt = "Use your Agent tool once with subagent_type claude-haiku and this prompt: Run this exact shell command in the foreground and wait for it to finish: for i in $(seq 1 120); do sleep 1; done; echo OA_MAC_STOP_MARKER. Then reply DONE."
        case "file":
            prompt = "Use your Agent tool once with subagent_type claude-haiku and this prompt: Use the Write tool to create a file named oa-delegate-check.txt in the current folder containing the single word GUAVA. Then reply DONE. Then tell me what the sub-agent replied."
        case "plan":
            prompt = "Use your Agent tool once with subagent_type claude-haiku and this prompt: Try to create a file named oa-plan-forbidden.txt in the current folder containing BLOCKED. If you are in plan/read-only mode or cannot write files, reply PLAN_BLOCKED. Then tell me what the sub-agent replied."
        default:
            prompt = "Use your Agent tool once with subagent_type claude-sonnet and this prompt: Use your Read tool to read README.md in the current folder and reply with the exact value after SECRET_TOKEN= and nothing else. Then tell me the exact value the sub-agent replied with."
        }

        let parent = CoderWorker()
        parent.program = "oa-worker"
        parent.folder = workDir
        parent.extraEnv = workerEnv
        var subWorkers: [Int32: CoderWorker] = [:]
        var subEngines: [Int32: Int32] = [:]
        var ranClaude = false
        var anyNonClaude = false
        var maxConcurrentClaude = 0
        var claudeToolCalls = 0
        var capturedClaudeTask = ""
        var parentGone = false

        defer {
            for (_, w) in subWorkers { w.line = nil; w.gone = nil; w.stop(forget: true) }
            parent.line = nil; parent.gone = nil; parent.stop(forget: true)
        }

        var lineOut = [CChar](repeating: 0, count: 1_048_576)
        func pumpSubagents() {
            while true {
                let j = cd_agent_next(page)
                if j < 0 { break }
                let eng = (mode == "off") ? Int32(0) : cd_agent_engine(page, j)
                let w = CoderWorker()
                w.folder = workDir
                if eng == 1 {
                    w.program = "claudeworker"
                    ranClaude = true
                } else {
                    w.program = "oa-worker"
                    w.extraEnv = workerEnv
                    anyNonClaude = true
                }
                w.line = { text in
                    if eng == 1 && (text.contains("\"kind\":\"ask\"") || text.contains("\"kind\":\"command\"")) {
                        claudeToolCalls += 1
                    }
                    cd_agent_line(page, j, text)
                    pumpSubagents()
                }
                w.gone = {
                    if subWorkers[j] === w {
                        subWorkers[j] = nil
                        subEngines[j] = nil
                        cd_agent_failed(page, j, "The sub-agent's worker stopped running.")
                        pumpSubagents()
                    }
                }
                guard w.start() else {
                    cd_agent_failed(page, j, w.failure ?? "The sub-agent's worker could not start.")
                    continue
                }
                subWorkers[j] = w
                subEngines[j] = eng
                let conc = subEngines.values.filter { $0 == 1 }.count
                if conc > maxConcurrentClaude { maxConcurrentClaude = conc }
                let tn = cd_agent_task(page, j, "mac-test-agent-\(j)-\(UUID().uuidString.prefix(8))", workDir, &lineOut, Int32(lineOut.count))
                if eng == 1 && capturedClaudeTask.isEmpty && tn > 0 {
                    capturedClaudeTask = String(decoding: lineOut.prefix(Int(tn)).map { UInt8(bitPattern: $0) }, as: UTF8.self)
                }
                w.write(lineOut, count: Int(tn))
            }
            for (j, w) in Array(subWorkers) {
                let an = cd_agent_answer_line(page, j, &lineOut, Int32(lineOut.count))
                if an > 0 { w.write(lineOut, count: Int(an)) }
                if cd_agent_running(page, j) == 0 {
                    subWorkers[j] = nil
                    subEngines[j] = nil
                    w.line = nil; w.gone = nil; w.stop(forget: true)
                }
            }
            let rn = cd_answer_line(page, &lineOut, Int32(lineOut.count))
            if rn > 0 { parent.write(lineOut, count: Int(rn)) }
        }

        parent.line = { text in
            cd_line(page, text)
            pumpSubagents()
        }
        parent.gone = { parentGone = true }

        guard parent.start() else { return fail("worker", parent.failure ?? "oa-worker failed to start") }
        guard cd_set_text(page, prompt) != 0 && cd_submit(page) != 0 else { return fail("send", "cd_submit failed") }
        let tn = cd_task(page, "mac-delegate-\(UUID().uuidString.prefix(8))", workDir, &lineOut, Int32(lineOut.count))
        guard tn > 0 else { return fail("task", "cd_task returned 0") }
        parent.write(lineOut, count: Int(tn))

        func pgrepMatches(_ pattern: String, exact: Bool = false) -> Bool {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/usr/bin/pgrep")
            p.arguments = exact ? ["-x", pattern] : ["-f", pattern]
            p.standardOutput = FileHandle.nullDevice
            p.standardError = FileHandle.nullDevice
            do { try p.run(); p.waitUntilExit(); return p.terminationStatus == 0 } catch { return false }
        }

        let deadline = Date().addingTimeInterval(180)
        var sawRunning = false
        var stopped = false
        var stopDeadline: Date? = nil
        while Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.02))
            _ = cd_shot_frame(page, 1340, 880, 1.0 / 60.0)
            if cd_permit_open(page) != 0 {
                _ = cd_permit_choose(page, 1, 0)
            }
            pumpSubagents()
            if mode == "stop" && ranClaude && !stopped {
                if stopDeadline == nil { stopDeadline = Date().addingTimeInterval(35) }
                if pgrepMatches("OA_MAC_STOP_[M]ARKER") {
                    ok("stop-armed", "Claude sub-agent child command is running; issuing Stop")
                    var sb = [CChar](repeating: 0, count: 1024)
                    let sn = cd_stop_line(&sb, Int32(sb.count))
                    parent.write(sb, count: Int(sn))
                    for (_, w) in subWorkers { w.write(sb, count: Int(sn)) }
                    let waitStop = Date().addingTimeInterval(4.0)
                    while Date() < waitStop && (pgrepMatches("OA_MAC_STOP_[M]ARKER") || !subWorkers.isEmpty) {
                        RunLoop.current.run(until: Date().addingTimeInterval(0.05))
                        pumpSubagents()
                    }
                    for (_, w) in subWorkers { w.line = nil; w.gone = nil; w.stop(forget: true) }
                    subWorkers = [:]
                    parent.line = nil; parent.gone = nil; parent.stop(forget: true)
                    Thread.sleep(forTimeInterval: 1.0)
                    stopped = true
                    break
                } else if let sd = stopDeadline, Date() > sd {
                    return fail("stop", "timed out waiting for OA_MAC_STOP_MARKER child command to start")
                }
            }
            if cd_running(page) != 0 { sawRunning = true }
            else if sawRunning || parentGone { break }
        }

        var saveBuf = [CChar](repeating: 0, count: 1_048_576)
        let savedN = cd_save(page, &saveBuf, Int32(saveBuf.count))
        let saved = savedN > 0 ? String(decoding: saveBuf.prefix(Int(savedN)).map { UInt8(bitPattern: $0) }, as: UTF8.self) : ""

        switch mode {
        case "off":
            if ranClaude { return fail("off", "claudeworker started with opt-in off") }
            if anyNonClaude { return fail("off", "another sub-agent ran in Claude's place") }
            guard saved.contains("Allow delegating") else {
                return fail("off", "reply did not name Allow delegating setting: \(saved.suffix(600))")
            }
            ok("off", "no claudeworker started; reply explained opt-in setting")
        case "stop":
            guard stopped else { return fail("stop", "never reached stop trigger") }
            if pgrepMatches("OA_MAC_STOP_[M]ARKER") { return fail("stop", "child shell command survived Stop") }
            if pgrepMatches("claudeworker", exact: true) { return fail("stop", "claudeworker survived Stop") }
            ok("stop", "Stop terminated claudeworker and child command with zero leftovers")
        case "two":
            guard ranClaude && maxConcurrentClaude >= 2 else {
                return fail("two", "expected >=2 concurrent Claude sub-agents, saw \(maxConcurrentClaude)")
            }
            guard saved.contains("KIWI") && saved.contains("PLUM") else {
                return fail("two", "chat missing KIWI/PLUM: \(saved.suffix(800))")
            }
            ok("two", "2 concurrent Claude sub-agents ran and returned KIWI + PLUM")
        case "file":
            let fileURL = URL(fileURLWithPath: workDir).appendingPathComponent("oa-delegate-check.txt")
            let content = (try? String(contentsOf: fileURL, encoding: .utf8)) ?? ""
            guard ranClaude && content.contains("GUAVA") else {
                return fail("file", "expected GUAVA in \(fileURL.path), ranClaude=\(ranClaude), content=\(content)")
            }
            ok("file", "Claude sub-agent wrote GUAVA to \(fileURL.path)")
        case "plan":
            let fileURL = URL(fileURLWithPath: workDir).appendingPathComponent("oa-plan-forbidden.txt")
            guard ranClaude else { return fail("plan", "Claude sub-agent did not start") }
            guard capturedClaudeTask.contains("\"mode\":\"plan\"") && capturedClaudeTask.contains("\"read_only\":true") else {
                return fail("plan", "Claude task line missing plan/read_only: \(capturedClaudeTask.prefix(400))")
            }
            guard !FileManager.default.fileExists(atPath: fileURL.path) else {
                return fail("plan", "forbidden file was created despite Plan mode!")
            }
            ok("plan", "Plan mode forwarded mode=plan + read_only=true and prevented file creation")
        default:
            guard ranClaude else { return fail("default", "claudeworker did not start") }
            guard claudeToolCalls >= 1 else { return fail("default", "expected Claude tool calls in sub-agent pane, saw 0") }
            guard saved.contains("CEDAR_4291") else {
                return fail("default", "parent final reply missing CEDAR_4291: \(saved.suffix(800))")
            }
            ok("default", "Claude Sonnet ran via claudeworker (\(claudeToolCalls) tool call events) and returned CEDAR_4291")
        }
        print("openagents-desktop-macos delegate-claude\(mode.isEmpty ? "" : " (\(mode))"): ok")
        return 0
    }

    static func main() {
        if let arg = CommandLine.arguments.first(where: { $0 == "--delegate-claude-test" || $0.hasPrefix("--delegate-claude-test=") }) {
            let mode = arg.hasPrefix("--delegate-claude-test=") ? String(arg.dropFirst("--delegate-claude-test=".count)) : ""
            let cwdArg: String? = CommandLine.arguments.firstIndex(of: "--cwd").flatMap { $0 + 1 < CommandLine.arguments.count ? CommandLine.arguments[$0 + 1] : nil }
            let modelArg: String? = CommandLine.arguments.firstIndex(of: "--model").flatMap { $0 + 1 < CommandLine.arguments.count ? CommandLine.arguments[$0 + 1] : nil }
            exit(runDelegateClaudeTest(mode: mode, cwdArg: cwdArg, modelArg: modelArg))
        }
        if let at = CommandLine.arguments.firstIndex(of: "--shot-providers"), at + 1 < CommandLine.arguments.count {
            exit(runProvidersShots(outDir: CommandLine.arguments[at + 1]))
        }
        if let at = CommandLine.arguments.firstIndex(of: "--shot-lean-rh"), at + 1 < CommandLine.arguments.count {
            exit(runLeanRhShots(outDir: CommandLine.arguments[at + 1]))
        }
        if let at = CommandLine.arguments.firstIndex(of: "--shot-rh-3d"), at + 1 < CommandLine.arguments.count {
            exit(runRh3dShots(outDir: CommandLine.arguments[at + 1]))
        }
        if let at = CommandLine.arguments.firstIndex(of: "--record-rh-3d"), at + 1 < CommandLine.arguments.count {
            exit(runRh3dRecord(outPath: CommandLine.arguments[at + 1]))
        }
        let app = NSApplication.shared; app.setActivationPolicy(.regular)
        let delegate = CoderDelegate(); app.delegate = delegate
        let bar = NSMenu(), appItem = NSMenuItem(), editItem = NSMenuItem()
        let appMenu = NSMenu(), edit = NSMenu(title: "Edit")
        let fileItem = NSMenuItem(), file = NSMenu(title: "File")
        file.addItem(withTitle: "New Chat", action: #selector(CoderDelegate.newChat(_:)), keyEquivalent: "n").target = delegate
        file.addItem(withTitle: "Dashboard", action: #selector(CoderDelegate.openDashboard(_:)), keyEquivalent: "d").target = delegate
        let newWinItem = file.addItem(withTitle: "New Window", action: #selector(CoderDelegate.newWindow(_:)), keyEquivalent: "N")
        newWinItem.keyEquivalentModifierMask = [.command, .shift]
        newWinItem.target = delegate
        file.addItem(withTitle: "Open Folder…", action: #selector(CoderDelegate.openFolder(_:)), keyEquivalent: "o").target = delegate
        let openSceneItem = file.addItem(withTitle: "Open 3D Scene…", action: #selector(CoderDelegate.openScene(_:)), keyEquivalent: "")
        openSceneItem.target = delegate
        delegate.gatedMenuItems.append((openSceneItem, { FeatureFlags.isOn(.scenes) }))
        file.addItem(withTitle: "Search…", action: #selector(CoderDelegate.search(_:)), keyEquivalent: "k").target = delegate
        file.addItem(withTitle: "Save", action: #selector(CoderDelegate.saveFile(_:)), keyEquivalent: "s").target = delegate
        file.addItem(.separator())
        file.addItem(withTitle: "Rename Chat…", action: #selector(CoderDelegate.renameChat(_:)), keyEquivalent: "").target = delegate
        file.addItem(withTitle: "Fork Chat", action: #selector(CoderDelegate.forkChat(_:)), keyEquivalent: "").target = delegate
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
        let chooseModelItem = view.addItem(withTitle: "Choose Model", action: #selector(CoderDelegate.chooseModel(_:)), keyEquivalent: "/")
        chooseModelItem.target = delegate
        let sim = view.addItem(withTitle: "iOS Simulator", action: #selector(CoderDelegate.toggleSimulator(_:)), keyEquivalent: "s")
        sim.target = delegate; sim.keyEquivalentModifierMask = [.command, .option]
        delegate.gatedMenuItems.append((sim, { FeatureFlags.isOn(.simulator) }))
        let term = view.addItem(withTitle: "Toggle Terminal", action: #selector(CoderDelegate.toggleTerminal(_:)), keyEquivalent: "j")
        term.target = delegate; term.keyEquivalentModifierMask = [.command]
        delegate.gatedMenuItems.append((term, { FeatureFlags.isOn(.terminal) }))
        let webPrev = view.addItem(withTitle: "Toggle Web Preview", action: #selector(CoderDelegate.toggleWebPreview(_:)), keyEquivalent: "p")
        webPrev.target = delegate; webPrev.keyEquivalentModifierMask = [.command, .option]
        delegate.gatedMenuItems.append((webPrev, { FeatureFlags.isOn(.preview) }))
        let mvItem = view.addItem(withTitle: "Toggle Metaverse Viewer", action: #selector(CoderDelegate.toggleMetaverse(_:)), keyEquivalent: "m")
        mvItem.target = delegate; mvItem.keyEquivalentModifierMask = [.command, .option]
        delegate.gatedMenuItems.append((mvItem, { FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) }))
        let mvExp = view.addItem(withTitle: "Expand / Restore Metaverse Viewer", action: #selector(CoderDelegate.toggleMetaverseExpand(_:)), keyEquivalent: "M")
        mvExp.target = delegate; mvExp.keyEquivalentModifierMask = [.command, .shift]
        delegate.gatedMenuItems.append((mvExp, { FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) }))
        let gbItem = view.addItem(withTitle: "Open GameBuilder", action: #selector(CoderDelegate.openGameBuilder(_:)), keyEquivalent: "g")
        gbItem.target = delegate; gbItem.keyEquivalentModifierMask = [.command, .option]
        delegate.gatedMenuItems.append((gbItem, { FeatureFlags.isOn(.gamebuilder) }))
        let gtaItem = view.addItem(withTitle: "GTA 6 Workbench (Mandy Key Lento)", action: #selector(CoderDelegate.openGtaWorkbench(_:)), keyEquivalent: "6")
        gtaItem.target = delegate; gtaItem.keyEquivalentModifierMask = [.command, .option]
        delegate.gatedMenuItems.append((gtaItem, { FeatureFlags.isOn(.metaverse) || FeatureFlags.isOn(.gamebuilder) || FeatureFlags.isOn(.scenes) }))
        let snapItem = view.addItem(withTitle: "SnapShot Region", action: #selector(CoderDelegate.triggerSnapShot(_:)), keyEquivalent: "6")
        snapItem.target = delegate; snapItem.keyEquivalentModifierMask = [.command, .shift]
        delegate.gatedMenuItems.append((snapItem, { FeatureFlags.isOn(.preview) }))
        let proofItem = view.addItem(withTitle: "Thread Proof & ATIF Trace…", action: #selector(CoderDelegate.openThreadProof(_:)), keyEquivalent: "P")
        proofItem.target = delegate; proofItem.keyEquivalentModifierMask = [.command, .shift]
        delegate.gatedMenuItems.append((proofItem, { FeatureFlags.isOn(.proof) }))
        let agentsItem = view.addItem(withTitle: "Agents (Coder, Tester, Game Builder)…", action: #selector(CoderDelegate.openAgentsPage(_:)), keyEquivalent: "A")
        agentsItem.target = delegate; agentsItem.keyEquivalentModifierMask = [.command, .shift]
        delegate.gatedMenuItems.append((agentsItem, { FeatureFlags.isOn(.agents) }))
        // Control-Tab is heard before the composer takes it (Window+Palette.swift); these name the keys
        let next = view.addItem(withTitle: "Next Chat", action: #selector(CoderDelegate.nextChat(_:)), keyEquivalent: "\t")
        next.target = delegate; next.keyEquivalentModifierMask = [.control]
        let previous = view.addItem(withTitle: "Previous Chat", action: #selector(CoderDelegate.previousChat(_:)), keyEquivalent: "\t")
        previous.target = delegate; previous.keyEquivalentModifierMask = [.control, .shift]
        view.addItem(.separator())
        view.addItem(withTitle: "Back to Chats", action: #selector(CoderDelegate.backToChats(_:)), keyEquivalent: "[").target = delegate
        viewItem.submenu = view
        appMenu.addItem(withTitle: "Settings…", action: #selector(CoderDelegate.openSettings(_:)), keyEquivalent: ",").target = delegate
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
        for item in delegate.accountItems { item.isHidden = !FeatureFlags.isOn(.account) }
        appMenu.addItem(.separator())
        let claudeKeyItem = appMenu.addItem(withTitle: "Claude Key…", action: #selector(CoderDelegate.claudeKey(_:)), keyEquivalent: "")
        claudeKeyItem.target = delegate
        delegate.gatedMenuItems.append((claudeKeyItem, { FeatureFlags.isOn(.modelsClaude) }))
        let orItem = appMenu.addItem(withTitle: "OpenRouter Models…", action: #selector(CoderDelegate.openRouterModels(_:)), keyEquivalent: "")
        orItem.target = delegate
        delegate.gatedMenuItems.append((orItem, { FeatureFlags.isOn(.modelsOpenrouter) }))
        appMenu.addItem(withTitle: "Check for Updates…", action: #selector(CoderDelegate.checkForUpdates(_:)), keyEquivalent: "").target = delegate
        let updAllItem = appMenu.addItem(withTitle: "Update Everything…", action: #selector(CoderDelegate.openUpdateEverything(_:)), keyEquivalent: "")
        updAllItem.target = delegate
        delegate.gatedMenuItems.append((updAllItem, { FeatureFlags.isOn(.scheduled) }))
        let schedItem = appMenu.addItem(withTitle: "Scheduled Tasks…", action: #selector(CoderDelegate.openScheduledTasks(_:)), keyEquivalent: "")
        schedItem.target = delegate
        delegate.gatedMenuItems.append((schedItem, { FeatureFlags.isOn(.scheduled) }))
        let capItem = appMenu.addItem(withTitle: "New Local Capability (Two-Arm Eval)…", action: #selector(CoderDelegate.openCapabilityStudio(_:)), keyEquivalent: "")
        capItem.target = delegate
        delegate.gatedMenuItems.append((capItem, { FeatureFlags.isOn(.capabilities) }))
        appMenu.addItem(withTitle: "Install CLI Shim…", action: #selector(CoderDelegate.toggleCliShim(_:)), keyEquivalent: "").target = delegate
        appMenu.addItem(withTitle: "Hold-to-Quit Protection (⌘Q×2)", action: #selector(CoderDelegate.toggleQuitHold(_:)), keyEquivalent: "").target = delegate
        for (item, check) in delegate.gatedMenuItems { item.isHidden = !check() }
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Hide OpenAgents", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit OpenAgents", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        let undo = edit.addItem(withTitle: "Undo", action: #selector(CoderDelegate.editUndo(_:)), keyEquivalent: "z")
        undo.target = delegate; undo.keyEquivalentModifierMask = [.command]
        let redo = edit.addItem(withTitle: "Redo", action: #selector(CoderDelegate.editRedo(_:)), keyEquivalent: "Z")
        redo.target = delegate; redo.keyEquivalentModifierMask = [.command, .shift]
        edit.addItem(.separator())
        edit.addItem(withTitle: "Cut", action: #selector(CoderDelegate.editCut(_:)), keyEquivalent: "x").target = delegate
        edit.addItem(withTitle: "Copy", action: #selector(CoderDelegate.editCopy(_:)), keyEquivalent: "c").target = delegate
        edit.addItem(withTitle: "Paste", action: #selector(CoderDelegate.editPaste(_:)), keyEquivalent: "v").target = delegate
        edit.addItem(withTitle: "Select All", action: #selector(CoderDelegate.editSelectAll(_:)), keyEquivalent: "a").target = delegate
        edit.addItem(.separator())
        edit.addItem(withTitle: "Find…", action: #selector(CoderDelegate.findInSettings(_:)), keyEquivalent: "f").target = delegate
        edit.addItem(withTitle: "Find Next", action: #selector(CoderDelegate.findNext(_:)), keyEquivalent: "g").target = delegate
        let findPrev = edit.addItem(withTitle: "Find Previous", action: #selector(CoderDelegate.findPrev(_:)), keyEquivalent: "g")
        findPrev.target = delegate; findPrev.keyEquivalentModifierMask = [.command, .shift]
        appItem.submenu = appMenu; editItem.submenu = edit; bar.addItem(appItem); bar.addItem(fileItem); bar.addItem(editItem); bar.addItem(viewItem)
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
        delegate.foremanMenuItem = foremanItem; foremanItem.isHidden = !FeatureFlags.isOn(.foreman)
        bar.addItem(foremanItem)
        app.mainMenu = bar
        app.run()
    }
}
