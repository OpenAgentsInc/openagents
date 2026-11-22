// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Settings (settings.coax): opening and closing, the search's keyboard, and what only the app knows.
import AppKit

#if OA_FULL
/// Settings > Models > the model new chats start on: Gemini 4 Neon unless set, else one of the
/// Claude models (0 Fable 5.2, 1 Opus 5.5, 2 Sonnet 5.5, 3 Haiku 5.5), which Claude Code runs.
/// Kept in UserDefaults (`orchestratorClaude`, `orchestratorClaudeModel`).
enum CoderOrchestrator {
    static let claudeKey = "orchestratorClaude", modelKey = "orchestratorClaudeModel"
    static var claude: Bool {
        get { UserDefaults.standard.bool(forKey: claudeKey) }
        set { UserDefaults.standard.set(newValue, forKey: claudeKey) }
    }
    static var model: Int {
        get { UserDefaults.standard.object(forKey: modelKey) as? Int ?? 1 }
        set { UserDefaults.standard.set(newValue, forKey: modelKey) }
    }
    /// Tells a page the pick. A page learns whether Claude Code is installed first (coderApplyClaude).
    /// Each Claude model's thinking level, in the picker's order (Fable 5.2, Opus 5.5, Sonnet 5.5,
    /// Haiku 5.5): 0 low, 1 medium, 2 high, 3 x-high, 4 max. High unless set (`claudeEfforts`).
    static let effortsKey = "claudeEfforts"
    static var efforts: [Int] {
        get {
            let kept = UserDefaults.standard.array(forKey: effortsKey) as? [Int] ?? []
            return (0..<4).map { $0 < kept.count && (0...4).contains(kept[$0]) ? kept[$0] : 2 }
        }
        set { UserDefaults.standard.set(newValue, forKey: effortsKey) }
    }
    static func apply(_ page: UnsafeMutableRawPointer?) {
        cd_orchestrator_set(page, claude ? 1 : 0, Int32(model))
        for (k, level) in efforts.enumerated() { cd_claude_effort_set(page, Int32(k), Int32(level)) }
    }
}
/// What every page is told of Claude: whether Claude Code is installed here (its models join the
/// picker, and sub-agents may run on them), and the model new chats start on.
func coderApplyClaude(_ page: UnsafeMutableRawPointer?) {
    cd_claude(page, CoderWorker.hasClaude ? 1 : 0)
    CoderOrchestrator.apply(page)
}
#endif

/// Compile-time bit positions matching coax/lib/agent/desktop/flags.coax (single source of truth).
enum DesktopFlag: Int64, CaseIterable {
    case modelsGemini = 0
    case modelsClaude = 1
    case modelsOpenrouter = 2
    case modelsAgy = 3
    case modelsExternal = 4
    case modelsExecutor = 5
    case modelsInklingNeon = 6
    case account = 7
    case foreman = 8
    case gym = 9
    case wallet = 10
    case forge = 11
    case git = 12
    case simulator = 13
    case terminal = 14
    case preview = 15
    case voice = 16
    case mcp = 17
    case scheduled = 18
    case environments = 19
    case pairing = 20
    case usage = 21
    case customize = 22
    case proof = 23
    case capabilities = 24
    case agents = 25
    case scenes = 26
    case rich = 27
    case files = 28
}

/// Resolved desktop feature flag mask backed by #authority coax/lib/agent/desktop/flags.coax.
/// All 29 flags default OFF for the Inkling-first release (single-model Inkling 2 Mini).
enum FeatureFlags {
    #if OA_FULL
    private static let flagNames: [String] = {
        let n = Int(DesktopCore.onCoreStack { oa_dc_flag_count() })
        return (0..<n).map { i in
            DesktopCore.callString(64) { oa_dc_flag_name(Int64(i), $0) }
        }
    }()

    static var mask: Int64 {
        let ud = UserDefaults.standard
        var defaultsMask: Int64 = 0
        for (i, name) in flagNames.enumerated() where !name.isEmpty {
            if ud.bool(forKey: "flag." + name) || ud.bool(forKey: "oa.flag." + name) {
                defaultsMask |= (Int64(1) << Int64(i))
            }
        }
        let exp: Int64 = ud.bool(forKey: Experimental.key) ? 1 : 0
        let envFlags = ProcessInfo.processInfo.environment["OA_FLAGS"] ?? ""
        let envExp = ProcessInfo.processInfo.environment["OA_EXPERIMENTAL"] ?? ""
        let argsNl = CommandLine.arguments.dropFirst().joined(separator: "\n")
        return DesktopCore.onCoreStack {
            DesktopCore.withCoaxString(envFlags) { ef in
                DesktopCore.withCoaxString(envExp) { ee in
                    DesktopCore.withCoaxString(argsNl) { an in
                        oa_dc_flags_resolve(exp, defaultsMask, ef, ee, an)
                    }
                }
            }
        }
    }

    static func isOn(_ flag: DesktopFlag) -> Bool {
        (mask & (Int64(1) << flag.rawValue)) != 0
    }

    static var anyExtraModel: Bool {
        DesktopCore.onCoreStack { oa_dc_flags_any_extra_model(mask) } != 0
    }
    #else
    // Minimal v1.0 release build: excluded features are stripped at compile time and stay permanently off.
    static var mask: Int64 { 0 }
    static func isOn(_ flag: DesktopFlag) -> Bool { false }
    static var anyExtraModel: Bool { false }
    #endif
}

/// Settings > General > Experimental features, off unless set (UserDefaults `experimentalFeatures`):
/// enables all 28 developer/experimental flags as an alias for OA_FLAGS=all.
enum Experimental {
    static let key = "experimentalFeatures"
    static var on: Bool {
        #if OA_FULL
        if UserDefaults.standard.bool(forKey: key) { return true }
        if ProcessInfo.processInfo.arguments.contains("--experimental") { return true }
        if ProcessInfo.processInfo.environment["OA_EXPERIMENTAL"] == "1" { return true }
        #endif
        return false
    }
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

    #if OA_FULL
    /// The page's Experimental features row was pressed (CP_ASK_EXPERIMENTAL): keep it, then show or
    /// hide what it covers here and in the menu bar.
    /// Settings > Models > "New chats start on" or "Claude model" was pressed (CP_ASK_ORCHESTRATOR):
    /// the pick is kept, and every chat out of view with no message yet moves to it.
    func orchestratorChanged() {
        CoderOrchestrator.claude = cd_orchestrator(page) != 0
        CoderOrchestrator.model = Int(cd_orchestrator_model(page))
        for r in runs.values where r.page != page { CoderOrchestrator.apply(r.page) }
    }
    /// Settings > Models > a Claude model's thinking level was pressed (CP_ASK_CLAUDE_EFFORT): the
    /// levels are kept, and every chat out of view learns them.
    func claudeEffortChanged() {
        CoderOrchestrator.efforts = (0..<4).map { Int(cd_claude_effort(page, Int32($0))) }
        for r in runs.values where r.page != page { CoderOrchestrator.apply(r.page) }
    }
    func experimentalChanged() {
        let on = cd_experimental(page) != 0
        UserDefaults.standard.set(on, forKey: Experimental.key)
        applyFlags()
    }
    #endif
    /// Settings > General > Notifications row was pressed (CP_ASK_NOTIFY): persist and request OS permission if needed.
    func notificationModeChanged() {
        let mode = CoderNotifyMode(clamping: cd_notify_mode(page))
        CoderNotifyMode.current = mode
        for r in runs.values where r.page != page {
            cd_notify_mode_set(r.page, mode.rawValue)
        }
        if mode.needsOS { CoderNotifications.shared.requestAuthorizationIfNeeded() }
        wake()
    }
    #if OA_FULL
    /// Propagates the current flag mask, OpenAgents account login state, and Gemini API key to the shown page and background runs.
    func syncCloudAuth() {
        let m = FeatureFlags.mask
        let authed: Int32 = (FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn) ? 1 : 0
        let gemini = FeatureFlags.isOn(.modelsGemini) ? (CoderGemini.saved.map { "Ends " + String($0.suffix(4)) } ?? "") : ""
        cd_flags_set(page, m)
        cd_cloud_authed_set(page, authed)
        cd_settings_gemini(page, gemini)
        for r in runs.values where r.page != page {
            cd_flags_set(r.page, m)
            cd_cloud_authed_set(r.page, authed)
            cd_settings_gemini(r.page, gemini)
        }
        if FeatureFlags.isOn(.modelsGemini) {
            CoderProbes.refreshGeminiAuth { [weak self] in self?.applyProbes() }
        }
        wake()
    }
    /// The Gemini sign-in and Antigravity's real model (CoderProbes) on the shown page and every run's.
    func applyProbes() {
        guard FeatureFlags.isOn(.modelsGemini) || FeatureFlags.isOn(.modelsAgy) else { return }
        CoderProbes.apply(page)
        for r in runs.values where r.page != page { CoderProbes.apply(r.page) }
        wake()
    }
    /// Applies the resolved 28-flag mask across UI pages, host background services, and menu bar items.
    func applyFlags() {
        let m = FeatureFlags.mask
        let acctAuthed: Int32 = (FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn) ? 1 : 0
        cd_flags_set(page, m)
        cd_cloud_authed_set(page, acctAuthed)
        for r in runs.values where r.page != page {
            cd_flags_set(r.page, m)
            cd_cloud_authed_set(r.page, acctAuthed)
        }
        if FeatureFlags.isOn(.foreman) {
            startForemanServer()
        } else {
            if isForemanShown { newChat() }
            foremanServer.stop()
            ForemanControlDescriptor.remove()
        }
        if FeatureFlags.isOn(.gym) {
            startGymRequests()
        } else {
            stopGymRequests()
            closeGym()
        }
        if FeatureFlags.isOn(.pairing) {
            CoderTailnet.shared.start()
        } else {
            CoderTailnet.shared.stopListening()
        }
        if FeatureFlags.isOn(.scheduled) {
            startScheduledAutomation()
        }
        if !FeatureFlags.isOn(.usage) {
            usageTimer?.invalidate(); usageTimer = nil
        }
        if !FeatureFlags.isOn(.voice) {
            voice.stop(cancel: true)
        }
        if FeatureFlags.isOn(.files) {
            detectInstalledEditors()
        }
        (NSApp.delegate as? CoderDelegate)?.applyFlags()
        syncSettingsInfo()
        wake()
    }
    /// Legacy alias kept for callers toggling Experimental features.
    func applyExperimental(_ on: Bool) {
        UserDefaults.standard.set(on, forKey: Experimental.key)
        applyFlags()
    }
    /// Starts the Foreman control server (127.0.0.1) and writes its descriptor for programs outside the app.
    func startForemanServer() {
        if foremanServer.start() {
            ForemanControlDescriptor.write(url: foremanServer.baseURL, token: foremanServer.operatorToken, auditLog: foremanAudit.fileURL.path)
        }
    }
    #endif
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
        let info = Bundle.main.infoDictionary
        var version = info?["CFBundleShortVersionString"] as? String ?? ""
        if let build = info?["CFBundleVersion"] as? String { version += " (" + build + ")" }
        #if OA_FULL
        func ends(_ key: String) -> String { "Ends " + String(key.suffix(4)) }
        var claude = ""
        if FeatureFlags.isOn(.modelsClaude) {
            if let launch = CoderKey.fromLaunch { claude = ends(launch) + " (this launch)" } else if let kept = CoderKey.saved { claude = ends(kept) }
        }
        var openrouter = ""
        if FeatureFlags.isOn(.modelsOpenrouter) {
            if let kept = CoderOpenRouter.saved { openrouter = ends(kept) } else if CoderOpenRouter.hasKey { openrouter = "From openrouter.env" }
        }
        let gemini = FeatureFlags.isOn(.modelsGemini) ? (CoderGemini.saved.map { ends($0) } ?? "") : ""
        #else
        let claude = ""
        let openrouter = ""
        let gemini = ""
        #endif
        cd_flags_set(page, FeatureFlags.mask)
        cd_settings_info(page, claude, openrouter, version)
        cd_settings_gemini(page, gemini)
        cd_settings_instructions(page, AgentInstructions.words)
        cd_notify_mode_set(page, CoderNotifyMode.current.rawValue)
        syncLocalModels()
        #if OA_FULL
        if FeatureFlags.isOn(.modelsAgy) { syncAntigravitySettings() }
        if FeatureFlags.isOn(.modelsExternal) { syncExternalProviders(on: page) }
        #endif
        #if OA_WALLET
        if FeatureFlags.isOn(.wallet) { syncWalletSettings() }
        #endif
        #if OA_FULL
        if FeatureFlags.isOn(.usage) && cd_settings_page(page) == 6 { refreshUsageDashboard() }
        if FeatureFlags.isOn(.mcp) { syncMcpSkills() }
        if FeatureFlags.isOn(.environments) || FeatureFlags.isOn(.pairing) { syncEnvironments() }
        if FeatureFlags.isOn(.scheduled) { syncScheduledSettings() }
        if FeatureFlags.isOn(.modelsGemini) || FeatureFlags.isOn(.modelsAgy) { applyProbes() }
        if FeatureFlags.isOn(.modelsGemini) {
            CoderProbes.refreshGeminiAuth { [weak self] in self?.applyProbes() }
        }
        #endif
    }
    func syncLocalState(on target: UnsafeMutableRawPointer) {
        let ui = LocalModelStore.uiState()
        let singleModel = !FeatureFlags.anyExtraModel
        var status = LocalModelStore.statusSummary()
        var statusCode = ui.statusCode
        var err = ui.err
        var on: Int32 = 0
        if LocalModelStore.isSelectable {
            switch LocalModelServer.shared.state {
            case .ready:
                on = 1
                statusCode = 3 // CP_LOCAL_READY
                err = ""
            case .failed(let msg):
                on = singleModel ? 0 : 1
                statusCode = 6 // CP_LOCAL_SERVER_ERROR
                err = msg
                status = "Server error — click Retry"
            case .idle, .starting:
                on = singleModel ? 0 : 1
                statusCode = 5 // CP_LOCAL_STARTING
                err = ""
                status = "Starting Inkling 2 Mini on Metal…"
            }
        }
        cd_settings_local(target, status)
        cd_inkling(target, on)
        cd_local_state(target, ui.osOk, ui.hwOk, ui.ramGB, statusCode, ui.pct, ui.dlMB, ui.totalMB, err)
    }
    func syncLocalModels() {
        LocalModelStore.verifyExistingIfNeeded { [weak self] in
            self?.syncLocalModels()
        }
        if LocalModelStore.isSelectable && (!FeatureFlags.anyExtraModel || cd_model_inkling(page) != 0) {
            LocalModelServer.shared.prewarmIfReady()
        }
        syncLocalState(on: page)
        for r in runs.values where r.page != page {
            syncLocalState(on: r.page)
        }
        if let shownState = localModelsDialogState, overlay.dialogOpen {
            let cur = LocalModelStore.state
            if cur == shownState {
                cd_dialog_body(page, LocalModelStore.dialogBody())
            } else {
                manageLocalModels()
            }
        }
        #if OA_FULL
        if FeatureFlags.isOn(.modelsExecutor) {
            syncExecutorModels()
        } else {
            cd_hybrid_exec(page, 0)
            for r in runs.values where r.page != page { cd_hybrid_exec(r.page, 0) }
        }
        #else
        cd_hybrid_exec(page, 0)
        for r in runs.values where r.page != page { cd_hybrid_exec(r.page, 0) }
        #endif
        wake()
    }
    #if OA_FULL
    func syncExecutorModels() {
        let snap = ExecutorModelStore.scan()
        let on: Int32 = snap.selectable ? 1 : 0
        cd_settings_executor(page, snap.status, snap.lineage, snap.badge)
        cd_hybrid_exec(page, on)
        for r in runs.values where r.page != page {
            cd_settings_executor(r.page, snap.status, snap.lineage, snap.badge)
            cd_hybrid_exec(r.page, on)
        }
        syncPylonExecutorDashboard()
    }
    func syncPylonExecutorDashboard() {
        let p = ExecutorReleaseUpdater.snapshot()
        cd_settings_pylon(page, p.pylonState, p.contributed, p.settledSats, p.releaseSummary)
        for r in runs.values where r.page != page {
            cd_settings_pylon(r.page, p.pylonState, p.contributed, p.settledSats, p.releaseSummary)
        }
    }
    func syncAntigravitySettings() {
        cd_settings_agy(page, CoderAntigravity.statusSummary(), Int32(CoderAntigravity.modeIndex), CoderAntigravity.modelSummary(), CoderAntigravity.authSummary())
        cd_permit_approval_mode_set(page, CoderAntigravity.modeIndex != 0 ? 1 : 0)
    }
    func setAntigravityMode(_ modeIndex: Int) {
        CoderAntigravity.modeIndex = modeIndex
        applyAntigravity()
    }
    func applyAntigravity() {
        let hasAgy: Int32 = CoderWorker.hasAntigravity ? 1 : 0
        let approvalOn: Int32 = CoderAntigravity.modeIndex != 0 ? 1 : 0
        cd_agy(page, hasAgy)
        cd_permit_approval_mode_set(page, approvalOn)
        configureWorkerEnv(run, isForeman: isForemanShown)
        for (id, r) in runs {
            cd_agy(r.page, hasAgy)
            cd_permit_approval_mode_set(r.page, approvalOn)
            configureWorkerEnv(r, isForeman: id == Foreman.chatID)
        }
        syncAntigravitySettings()
        // the model agy runs depends on these settings: ask it again
        applyProbes()
        CoderProbes.refreshAgyModel { [weak self] in self?.applyProbes() }
        wake()
    }
    #endif
}
