// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Settings (settings.coax): opening and closing, the search's keyboard, and what only the app knows.
import AppKit

/// Settings > General > Experimental features, off unless set (UserDefaults `experimentalFeatures`):
/// log in and the account, Gym, and Foreman (its sidebar row, its menu and its control server).
/// Off, Coder is local: chats, runs and pairing a phone over Tailscale need no login.
enum Experimental {
    static let key = "experimentalFeatures"
    static var on: Bool { UserDefaults.standard.bool(forKey: key) }
}

/// The person's agent instructions (~/.openagents/instructions.md, GitHub #13808), told to Microcoder,
/// Claude and Antigravity in every chat. Every rule (where the file is, the default, seeding, reset)
/// is the desktop core's (coax/lib/agent/user-instructions.coax); this only calls it.
enum AgentInstructions {
    static var path: String { DesktopCore.callString(4096) { oa_dc_instructions_path($0) } }
    static var limit: Int { Int(DesktopCore.onCoreStack { oa_dc_instructions_max() }) }
    /// First run: the default, unless a file is there.
    @discardableResult static func seed() -> Bool { DesktopCore.onCoreStack { oa_dc_instructions_seed() } == 1 }
    static func reset() -> Bool { DesktopCore.onCoreStack { oa_dc_instructions_reset() } == 1 }
    /// The file's text (the default when there is none); nil when it is longer than the editor holds.
    static func read() -> String? {
        var buf = [UInt8](repeating: 0, count: limit + 1)
        let n = DesktopCore.withByteBuf(&buf) { oa_dc_instructions_read($0) }
        if n < 0 { return nil }
        return String(decoding: buf[0..<min(Int(n), buf.count)], as: UTF8.self)
    }
    static func write(_ text: String) -> Bool { DesktopCore.withCoaxString(text) { oa_dc_instructions_write($0) } == 1 }
    /// What General's row shows: "Default", "Edited", "Off (empty)".
    static var words: String { DesktopCore.callString(64) { oa_dc_instructions_words($0) } }
}

extension CoderWindow {
    /// Settings > General > Agent instructions > Edit…: the file in a box; Save writes it whole, and
    /// the next message of every chat is told it. A file too long for the box opens in the editor.
    func editInstructions() {
        guard let current = AgentInstructions.read() else {
            var d = CoderDialog(size: .md, title: "Agent Instructions", body: "The file is longer than " + String(AgentInstructions.limit / 1024) + " KB, more than this box holds, and agents are told only its start. Open it in your text editor to change it:\n\n" + AgentInstructions.path)
            d.buttons = [.init(title: "Open File", aside: true)]
            d.cancel = d.buttons.count; d.buttons.append(.init(title: "OK", kind: .primary)); d.confirm = d.cancel
            present(d) { [weak self] answer, _ in
                if answer == 0 { NSWorkspace.shared.open(URL(fileURLWithPath: AgentInstructions.path)) }
                self?.refocus()
            }
            return
        }
        var d = CoderDialog(size: .lg, title: "Agent Instructions", body: "Microcoder, Claude and Antigravity are told these in every chat, in every folder; a repository's own AGENTS.md wins where they differ. Edits apply from the next message. Empty the box to turn them off. Kept in ~/.openagents/instructions.md.")
        // a box of fourteen lines: Return makes a new line, Command-Return saves
        d.fields = [.init(value: current, lines: 14)]
        d.buttons = [.init(title: "Reset to Default", kind: .danger, aside: true)]
        let reset = 0
        d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
        let save = d.buttons.count; d.buttons.append(.init(title: "Save", kind: .primary))
        d.confirm = save
        present(d) { [weak self] answer, values in
            guard let self else { return }
            switch answer {
            case save:
                var text = values.first ?? ""
                if !text.isEmpty && !text.hasSuffix("\n") { text += "\n" }
                if !AgentInstructions.write(text) {
                    self.present(.notice("The instructions could not be saved.", "Coder could not write " + AgentInstructions.path + "."))
                    return
                }
            case reset:
                self.resetInstructions()
                return
            default:
                self.refocus()
                return
            }
            self.syncSettingsInfo()
            self.refocus()
        }
    }
    /// Settings > General > Agent instructions > Reset…: OpenAgents' default in place of the file.
    func resetInstructions() {
        var d = CoderDialog(size: .sm, title: "Reset Agent Instructions?", body: "Your instructions are replaced with OpenAgents' default: in a git repository, commit finished work and push it.")
        d.buttons = [.init(title: "Cancel"), .init(title: "Reset", kind: .danger)]
        d.cancel = 0; d.confirm = 0   // a destructive dialog: Return presses Cancel
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 && !AgentInstructions.reset() {
                self.present(.notice("The instructions could not be reset.", "Coder could not write " + AgentInstructions.path + "."))
                return
            }
            self.syncSettingsInfo()
            self.refocus()
        }
    }

    /// The page's Experimental features row was pressed (CP_ASK_EXPERIMENTAL): keep it, then show or
    /// hide what it covers here and in the menu bar.
    func experimentalChanged() {
        let on = cd_experimental(page) != 0
        UserDefaults.standard.set(on, forKey: Experimental.key)
        applyExperimental(on)
    }
    /// Propagates the current experimental + OpenAgents account login state and Gemini API key to the shown page and background runs.
    func syncCloudAuth() {
        let authed: Int32 = (Experimental.on && MacAccount.shared.loggedIn) ? 1 : 0
        let exp: Int32 = Experimental.on ? 1 : 0
        let gemini = CoderGemini.saved.map { "Ends " + String($0.suffix(4)) } ?? ""
        cd_experimental_set(page, exp)
        cd_cloud_authed_set(page, authed)
        cd_settings_gemini(page, gemini)
        for r in runs.values where r.page != page {
            cd_experimental_set(r.page, exp)
            cd_cloud_authed_set(r.page, authed)
            cd_settings_gemini(r.page, gemini)
        }
        // a key saved or removed in Settings can change which sign-in Microcoder uses
        CoderProbes.refreshGeminiAuth { [weak self] in self?.applyProbes() }
        wake()
    }
    /// The Gemini sign-in and Antigravity's real model (CoderProbes) on the shown page and every run's.
    func applyProbes() {
        CoderProbes.apply(page)
        for r in runs.values where r.page != page { CoderProbes.apply(r.page) }
        wake()
    }
    /// Shows or hides the experimental features: the page's rows, the Foreman control server, the menus.
    func applyExperimental(_ on: Bool) {
        cd_experimental_set(page, on ? 1 : 0)
        cd_cloud_authed_set(page, (on && MacAccount.shared.loggedIn) ? 1 : 0)
        for r in runs.values where r.page != page {
            cd_experimental_set(r.page, on ? 1 : 0)
            cd_cloud_authed_set(r.page, (on && MacAccount.shared.loggedIn) ? 1 : 0)
        }
        if on {
            startForemanServer()
            startGymRequests()
        } else {
            stopGymRequests()
            closeGym()
            if isForemanShown { newChat() }
            foremanServer.stop()
            ForemanControlDescriptor.remove()
        }
        (NSApp.delegate as? CoderDelegate)?.applyExperimental(on)
        wake()
    }
    /// Starts the Foreman control server (127.0.0.1) and writes its descriptor for programs outside the app.
    func startForemanServer() {
        if foremanServer.start() {
            ForemanControlDescriptor.write(url: foremanServer.baseURL, token: foremanServer.operatorToken, auditLog: foremanAudit.fileURL.path)
        }
    }
    // MARK: - Settings (settings.coax)
    /// Coder > Settings… (Command-comma), and the menu items that open one of its pages: 0 General,
    /// 1 Models, 2 Foreman, 3 Account, 4 About & updates (-1 the last one shown), at a row ("" the top).
    func openSettings(_ which: Int32 = -1, row: String = "") {
        syncSettingsInfo()
        cd_settings_open(page, which, row)
        act(cd_action(page)); wake()
    }
    /// View > Back to Chats (Command-[), Esc and the gear: the chat as it was.
    func closeSettings() { cd_settings_open(page, -2, ""); act(cd_action(page)); wake() }
    var settingsOpen: Bool { cd_settings_page(page) >= 0 }
    /// Edit > Find (Command-F) in settings: the search field takes the keyboard.
    func findInSettings() { if settingsOpen { focusSearch() } }
    /// Esc in settings: a search is emptied first, then settings closes.
    func escape() {
        if overlay.escape() { return }   // a dialog or a menu over settings closes first
        if cd_search_clear(page) != 0 { focusSearch(); return }
        closeSettings()
    }
    /// Settings opened or closed: the keyboard goes to the page or back to the composer.
    func settingsMoved() {
        // the page moved the keyboard itself (settings.coax): away from the composer, or back to it
        if settingsOpen { syncSettingsInfo(); wake() } else { focusComposer() }
    }
    /// After a sheet: the composer takes the keyboard back, unless settings is open.
    func refocus() { if !settingsOpen { focusComposer() } else { window.makeFirstResponder(canvas); syncSettingsInfo(); wake() } }
    /// What settings' pages show that only the app knows: the keys' ends and the version.
    func syncSettingsInfo() {
        func ends(_ key: String) -> String { "Ends " + String(key.suffix(4)) }
        var claude = ""
        if let launch = CoderKey.fromLaunch { claude = ends(launch) + " (this launch)" } else if let kept = CoderKey.saved { claude = ends(kept) }
        var openrouter = ""
        if let kept = CoderOpenRouter.saved { openrouter = ends(kept) } else if CoderOpenRouter.hasKey { openrouter = "From openrouter.env" }
        let info = Bundle.main.infoDictionary
        var version = info?["CFBundleShortVersionString"] as? String ?? ""
        if let build = info?["CFBundleVersion"] as? String { version += " (" + build + ")" }
        let gemini = CoderGemini.saved.map { ends($0) } ?? ""
        cd_settings_info(page, claude, openrouter, version)
        cd_settings_gemini(page, gemini)
        cd_settings_instructions(page, AgentInstructions.words)
        syncAntigravitySettings()
        applyProbes()
        CoderProbes.refreshGeminiAuth { [weak self] in self?.applyProbes() }
    }
    func syncAntigravitySettings() {
        cd_settings_agy(page, CoderAntigravity.statusSummary(), Int32(CoderAntigravity.modeIndex), CoderAntigravity.modelSummary(), CoderAntigravity.authSummary())
    }
    func setAntigravityMode(_ modeIndex: Int) {
        CoderAntigravity.modeIndex = modeIndex
        applyAntigravity()
    }
    func applyAntigravity() {
        let hasAgy: Int32 = CoderWorker.hasAntigravity ? 1 : 0
        cd_agy(page, hasAgy)
        configureWorkerEnv(run, isForeman: isForemanShown)
        for (id, r) in runs {
            cd_agy(r.page, hasAgy)
            configureWorkerEnv(r, isForeman: id == Foreman.chatID)
        }
        syncAntigravitySettings()
        // the model agy runs depends on these settings: ask it again
        applyProbes()
        CoderProbes.refreshAgyModel { [weak self] in self?.applyProbes() }
        wake()
    }
}
