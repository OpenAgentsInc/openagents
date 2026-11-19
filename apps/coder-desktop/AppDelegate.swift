// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The app delegate, the status item, the main menu and main.
import AppKit

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
        // the daemon keeps them running; the next Coder attaches to them (GitHub #13694)
        if main?.runsOutliveQuit == true { return .terminateNow }
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
        if item.action == #selector(renameChat(_:)) { return main?.canRename == true }
        if item.action == #selector(deleteChat(_:)) { return main?.canDelete == true }
        if item.action == #selector(openFolder(_:)) { return main?.canFolder == true }
        if item.action == #selector(followUpOther(_:)) {
            guard let w = main else { return false }
            item.title = cd_followup_queues(w.page) != 0 ? "Steer the Reply" : "Queue a Follow-up"
            return cd_running(w.page) != 0
        }
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
        // while a reply runs, Command-Return does what Return does not: queue, or steer (#13717)
        file.addItem(.separator())
        let other = file.addItem(withTitle: "Queue a Follow-up", action: #selector(CoderDelegate.followUpOther(_:)), keyEquivalent: "\r")
        other.target = delegate; other.keyEquivalentModifierMask = [.command]
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
        view.addItem(.separator())
        view.addItem(withTitle: "Back to Chats", action: #selector(CoderDelegate.backToChats(_:)), keyEquivalent: "[").target = delegate
        viewItem.submenu = view
        appMenu.addItem(withTitle: "Settings…", action: #selector(CoderDelegate.openSettings(_:)), keyEquivalent: ",").target = delegate
        appMenu.addItem(.separator())
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
        edit.addItem(.separator())
        edit.addItem(withTitle: "Find in Settings", action: #selector(CoderDelegate.findInSettings(_:)), keyEquivalent: "f").target = delegate
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
