// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Settings (settings.coax): opening and closing, the search's keyboard, and what only the app knows.
import AppKit

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
    case metaverse = 28
    case gamebuilder = 29
    case browser = 30
}

/// Resolved desktop feature flag mask backed by #authority coax/lib/agent/desktop/flags.coax.
/// All 31 flags except account default OFF for the Inkling-first release (single-model Inkling 2 Mini).
enum FeatureFlags {
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
}

/// Settings > General > Sidebar view (UserDefaults `sidebarCondensed`):
/// false = Expanded (2-line chat card), true = Condensed (top line only).
enum SidebarViewMode {
    static let key = "sidebarCondensed"
    static var condensed: Bool {
        get { UserDefaults.standard.bool(forKey: key) }
        set { UserDefaults.standard.set(newValue, forKey: key) }
    }
}

/// Settings > General > Experimental features, off unless set (UserDefaults `experimentalFeatures`):
/// enables all 28 developer/experimental flags as an alias for OA_FLAGS=all.
enum Experimental {
    static let key = "experimentalFeatures"
    static var on: Bool {
        if UserDefaults.standard.bool(forKey: key) { return true }
        if ProcessInfo.processInfo.arguments.contains("--experimental") { return true }
        if ProcessInfo.processInfo.environment["OA_EXPERIMENTAL"] == "1" { return true }
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

/// The person's custom system prompt (~/.openagents/system-prompt.md), appended to every single
/// system prompt across all agents (Flawed, Inky, Microcoder, Claude, and Antigravity).
enum CustomSystemPrompt {
    static var path: String { DesktopCore.callString(4096) { oa_dc_custom_prompt_path($0) } }
    static var limit: Int { Int(DesktopCore.onCoreStack { oa_dc_custom_prompt_max() }) }
    /// The file's text ("" when absent); nil when it is longer than the editor holds.
    static func read() -> String? {
        var buf = [UInt8](repeating: 0, count: limit + 1)
        let n = DesktopCore.withByteBuf(&buf) { oa_dc_custom_prompt_read($0) }
        if n < 0 { return nil }
        return String(decoding: buf[0..<min(Int(n), buf.count)], as: UTF8.self)
    }
    static func write(_ text: String) -> Bool { DesktopCore.withCoaxString(text) { oa_dc_custom_prompt_write($0) } == 1 }
    static func clear() -> Bool { DesktopCore.onCoreStack { oa_dc_custom_prompt_clear() } == 1 }
    /// What Settings shows: "Off", "Active", or "Active (over 8 KB)".
    static var words: String { DesktopCore.callString(64) { oa_dc_custom_prompt_words($0) } }
    /// Short single-line preview for the Settings row description, or "" when off.
    static var preview: String {
        guard let raw = read() else {
            return "Active in ~/.openagents/system-prompt.md (over \(limit / 1024) KB; first \(limit / 1024) KB appended)."
        }
        let first = raw
            .split(whereSeparator: { $0.isNewline })
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .first(where: { !$0.isEmpty }) ?? ""
        if first.isEmpty { return "" }
        if first.count <= 110 { return "Appended to every system prompt: \"\(first)\"" }
        return "Appended to every system prompt: \"\(first.prefix(107))…\""
    }
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
        var d = CoderDialog(size: .lg, title: "Agent Instructions", body: "OpenAgents and connected external harnesses are told these in every chat, in every folder; a repository's own AGENTS.md wins where they differ. Edits apply from the next message. Empty the box to turn them off. Kept in ~/.openagents/instructions.md.")
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

    /// Settings > General > Custom system prompt > Edit…: edits ~/.openagents/system-prompt.md,
    /// which is appended to every single system prompt across all agents starting on the next turn.
    func editCustomSystemPrompt() {
        guard let current = CustomSystemPrompt.read() else {
            var d = CoderDialog(size: .md, title: "Custom System Prompt", body: "The file is longer than " + String(CustomSystemPrompt.limit / 1024) + " KB, more than this box holds, and agents are told only its first " + String(CustomSystemPrompt.limit / 1024) + " KB. Open it in your text editor to change it:\n\n" + CustomSystemPrompt.path)
            d.buttons = [.init(title: "Open File", aside: true)]
            d.cancel = d.buttons.count; d.buttons.append(.init(title: "OK", kind: .primary)); d.confirm = d.cancel
            present(d) { [weak self] answer, _ in
                if answer == 0 { NSWorkspace.shared.open(URL(fileURLWithPath: CustomSystemPrompt.path)) }
                self?.refocus()
            }
            return
        }
        var d = CoderDialog(size: .lg, title: "Custom System Prompt", body: "Added to every system prompt across OpenAgents and connected external harnesses in every chat and folder. Edits apply from the next message. Leave empty to turn off. Kept in ~/.openagents/system-prompt.md.")
        d.fields = [.init(value: current, hint: "e.g., Always be concise, show exact commands, and explain architectural trade-offs.", lines: 14)]
        d.buttons = [.init(title: "Clear", kind: .danger, aside: true)]
        let clearBtn = 0
        d.cancel = d.buttons.count; d.buttons.append(.init(title: "Cancel"))
        let save = d.buttons.count; d.buttons.append(.init(title: "Save", kind: .primary))
        d.confirm = save
        present(d) { [weak self] answer, values in
            guard let self else { return }
            switch answer {
            case save:
                var text = values.first ?? ""
                if !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !text.hasSuffix("\n") { text += "\n" }
                if !CustomSystemPrompt.write(text) {
                    self.present(.notice("The custom system prompt could not be saved.", "Coder could not write " + CustomSystemPrompt.path + "."))
                    return
                }
            case clearBtn:
                self.clearCustomSystemPrompt()
                return
            default:
                self.refocus()
                return
            }
            self.syncSettingsInfo()
            self.refocus()
        }
    }

    /// Settings > General > Custom system prompt > Clear…: removes custom system prompt text.
    func clearCustomSystemPrompt() {
        var d = CoderDialog(size: .sm, title: "Clear Custom System Prompt?", body: "Your custom system prompt instructions will be cleared so nothing extra is appended to system prompts.")
        d.buttons = [.init(title: "Cancel"), .init(title: "Clear", kind: .danger)]
        d.cancel = 0; d.confirm = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 && !CustomSystemPrompt.clear() {
                self.present(.notice("The custom system prompt could not be cleared.", "Coder could not write " + CustomSystemPrompt.path + "."))
                return
            }
            self.syncSettingsInfo()
            self.refocus()
        }
    }

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
    /// Settings > General > Sidebar view row was pressed (CP_ASK_SIDEBAR_VIEW): persist and sync pages.
    func sidebarViewChanged() {
        let condensed = cd_sb_condensed(page) != 0
        SidebarViewMode.condensed = condensed
        for r in runs.values where r.page != page {
            cd_sb_condensed_set(r.page, condensed ? 1 : 0)
        }
        wake()
    }
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
    /// The Gemini sign-in and Antigravity's real model (CoderProbes) on the shown page and every run's.
    func applyProbes() {
        CoderProbes.apply(page)
        for r in runs.values where r.page != page { CoderProbes.apply(r.page) }
        wake()
    }
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
        configureWorkerEnv(run, isForeman: false)
        checkFlawedAuth(showToast: false)
        syncAccountSettings()
        if FeatureFlags.isOn(.modelsGemini) {
            CoderProbes.refreshGeminiAuth { [weak self] in self?.applyProbes() }
        }
        wake()
    }
    /// Applies the resolved flag mask across UI pages and menu bar items.
    func applyFlags() {
        let m = FeatureFlags.mask
        let acctAuthed: Int32 = (FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn) ? 1 : 0
        cd_flags_set(page, m)
        cd_cloud_authed_set(page, acctAuthed)
        for r in runs.values where r.page != page {
            cd_flags_set(r.page, m)
            cd_cloud_authed_set(r.page, acctAuthed)
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
    // MARK: - v1 OpenAgents Cloud Account, Enrollment, Execution Target & Projects
    static let executionTargetKey = "oa.executionTarget"
    static let enrolledComputerKey = "oa.cloudEnrolledComputer"

    var executionTarget: String {
        get { UserDefaults.standard.string(forKey: Self.executionTargetKey) ?? "Auto" }
        set { UserDefaults.standard.set(newValue, forKey: Self.executionTargetKey) }
    }

    var enrolledComputerStatus: String {
        if let name = UserDefaults.standard.string(forKey: Self.enrolledComputerKey), !name.isEmpty {
            return "Enrolled (\(name))"
        }
        return "Not enrolled"
    }

    func syncAccountSettings() {
        let acct = MacAccount.shared
        let email = acct.loggedIn ? (acct.email.isEmpty ? "Signed in" : acct.email) : "Not signed in"
        cd_settings_account(
            page,
            email,
            acct.orgName,
            acct.paidCreditLabel,
            acct.promoCreditLabel,
            acct.promoExpiryLabel,
            acct.githubStatus,
            enrolledComputerStatus,
            executionTarget,
            acct.projectsSummary
        )
        wake()
    }

    /// Tier 2 outbound OpenAgents Cloud computer enrollment (zero Tailscale, zero inbound ports).
    func manageCloudEnrollment() {
        guard MacAccount.shared.loggedIn else {
            Task { @MainActor in await MacAccount.shared.showLogin() }
            return
        }
        let hostName = Host.current().localizedName ?? ProcessInfo.processInfo.hostName
        let currentlyEnrolled = !(UserDefaults.standard.string(forKey: Self.enrolledComputerKey) ?? "").isEmpty
        var d = CoderDialog(
            size: .md,
            title: "OpenAgents Cloud Computer Enrollment",
            body: currentlyEnrolled
                ? "This Mac (\(hostName)) is enrolled in OpenAgents Cloud over outbound HTTPS/SSE (zero Tailscale, zero inbound ports). Web and iOS can dispatch turns to your local repos."
                : "Enroll this Mac (\(hostName)) into OpenAgents Cloud so Web and iOS can run turns on your local repositories over outbound HTTPS/SSE — no Tailscale and no inbound ports."
        )
        d.buttons = [
            .init(title: "Cancel"),
            .init(title: currentlyEnrolled ? "Unenroll" : "Enroll This Mac", kind: currentlyEnrolled ? .danger : .primary)
        ]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, _ in
            guard let self, answer == 1 else { self?.refocus(); return }
            if currentlyEnrolled {
                UserDefaults.standard.removeObject(forKey: Self.enrolledComputerKey)
                self.syncAccountSettings()
                cd_toast(self.page, "Computer unenrolled from OpenAgents Cloud", 0)
            } else {
                UserDefaults.standard.set(hostName, forKey: Self.enrolledComputerKey)
                self.syncAccountSettings()
                cd_toast(self.page, "Enrolled \(hostName) in OpenAgents Cloud", 0)
            }
            self.refocus()
        }
    }

    /// Execution Target Selector: Auto | Local | Cloud | Cloud Sandbox (oa-boat) | Enrolled Computer.
    func chooseExecutionTarget() {
        let hostName = UserDefaults.standard.string(forKey: Self.enrolledComputerKey)
        var choices = ["Auto", "Local", "Cloud", "Cloud Sandbox"]
        if let hostName, !hostName.isEmpty { choices.append("Enrolled: \(hostName)") }
        let current = executionTarget
        let body = "Current target: \(current)\nChoose where new turns execute by default:\n• Auto — smart placement across Local, Cloud, and Sandboxes\n• Local — run locally on this Mac\n• Cloud — run on OpenAgents Cloud (Tier 1 tools)\n• Cloud Sandbox — ephemeral Debian 12 GCE VM (oa-boat)"
        var d = CoderDialog(size: .md, title: "Execution Target", body: body)
        d.buttons = choices.map { $0 == current ? .init(title: $0, kind: .primary) : .init(title: $0) }
        d.buttons.append(.init(title: "Cancel"))
        d.cancel = d.buttons.count - 1
        d.confirm = max(0, choices.firstIndex(of: current) ?? 0)
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer >= 0 && answer < choices.count {
                self.executionTarget = choices[answer]
                self.syncAccountSettings()
                cd_toast(self.page, "Execution target: \(choices[answer])", 0)
            }
            self.refocus()
        }
    }

    /// Lists synced Cloud Projects (/api/projects/v1) matched to the user's account.
    func manageCloudProjects() {
        guard MacAccount.shared.loggedIn else {
            Task { @MainActor in await MacAccount.shared.showLogin() }
            return
        }
        Task { @MainActor in
            let list = await MacAccount.shared.fetchCloudProjects()
            self.syncAccountSettings()
            let lines = list.prefix(12).compactMap { p -> String? in
                let name = (p["name"] as? String) ?? (p["slug"] as? String) ?? "Project"
                let repo = (p["repo"] as? String) ?? ""
                return repo.isEmpty ? "• \(name)" : "• \(name) (\(repo))"
            }
            let body = lines.isEmpty
                ? "No Cloud Projects synced yet. Create a project on openagents.com/projects or connect GitHub via WorkOS Pipes."
                : lines.joined(separator: "\n")
            var d = CoderDialog(size: .md, title: "OpenAgents Cloud Projects", body: body)
            d.buttons = [.init(title: "Open openagents.com/projects"), .init(title: "Done", kind: .primary)]
            d.cancel = 1; d.confirm = 1
            self.present(d) { [weak self] answer, _ in
                if answer == 0, let u = URL(string: MacAccount.origin + "/projects") {
                    NSWorkspace.shared.open(u)
                }
                self?.refocus()
            }
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
        if cd_settings_page(page) == 14 && cd_key(page, -1, 0) != 0 { wake(); return }
        if cd_search_clear(page) != 0 { focusSearch(); return }
        closeSettings()
    }
    /// Settings opened or closed: the keyboard goes to the page or back to the composer.
    func settingsMoved() {
        // the page moved the keyboard itself (settings.coax): away from the composer, or back to it
        if settingsOpen {
            syncSettingsInfo()
            wake()
        } else {
            focusComposer()
        }
    }
    /// After a sheet: the composer takes the keyboard back, unless settings is open.
    func refocus() { if !settingsOpen { focusComposer() } else { window.makeFirstResponder(canvas); syncSettingsInfo(); wake() } }
    /// What settings' pages show that only the app knows: the keys' ends and the version.
    func syncSettingsInfo() {
        let info = Bundle.main.infoDictionary
        var version = BundleMetadata.currentUpdateIdentifier
        if let profile = info?["OAProfile"] as? String, !profile.isEmpty { version += " (" + profile + ")" }
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
        cd_flags_set(page, FeatureFlags.mask)
        cd_cloud_authed_set(page, (FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn) ? 1 : 0)
        AgentSettings.apply(to: page)
        cd_settings_info(page, claude, openrouter, version)
        cd_settings_gemini(page, gemini)
        syncAccountSettings()
        cd_settings_instructions(page, AgentInstructions.words)
        cd_settings_custom_prompt(page, CustomSystemPrompt.words, CustomSystemPrompt.preview)
        cd_notify_mode_set(page, CoderNotifyMode.current.rawValue)
        cd_sb_condensed_set(page, SidebarViewMode.condensed ? 1 : 0)
        syncLocalModels()
        applyProbes()
        if FeatureFlags.isOn(.mcp) { syncMcpSkills() }
        if FeatureFlags.isOn(.modelsGemini) && (ComposerModelVisibility.showOtherModels || cd_settings_page(page) == 1) {
            CoderProbes.refreshGeminiAuth { [weak self] in self?.applyProbes() }
        }
    }
    func syncLocalState(on target: UnsafeMutableRawPointer) {
        let ui = LocalModelStore.uiState()
        let showOthers = ComposerModelVisibility.showOtherModels
        var status = LocalModelStore.statusSummary()
        var statusCode = ui.statusCode
        var err = ui.err
        var on: Int32 = 0
        var serverStarting = false
        var serverErr: String? = nil
        if LocalModelStore.isSelectable {
            switch LocalModelServer.shared.state {
            case .ready:
                on = 1
                statusCode = 3 // CP_LOCAL_READY
                err = ""
            case .idle:
                // Weights are verified on disk; composer stays usable and loads on demand when sent.
                on = 1
                statusCode = 3 // CP_LOCAL_READY
                err = ""
            case .starting:
                // Allow immediate compose/send while loading; an active turn shows "Loading model…"
                // until the server is warm and excludes model load from TTFT.
                on = 1
                serverStarting = true
                statusCode = 5 // CP_LOCAL_STARTING
                err = ""
                let tag = LocalModelStore.entry.id == LocalModelCatalogEntry.inkling2Mini.id ? "" : "\(LocalModelStore.entry.shortLabel) · "
                status = "\(tag)Loading model…"
            case .failed(let msg):
                on = 0
                serverErr = msg
                statusCode = 6 // CP_LOCAL_SERVER_ERROR
                err = msg
                status = "Server error — click Retry"
            }
        }
        let desc = LocalModelStore.pickerModelDesc(serverStarting: serverStarting, serverError: serverErr)
        cd_show_other_models_set(target, showOthers ? 1 : 0)
        cd_inkling_info(target, LocalModelStore.pickerModelId, LocalModelStore.pickerModelLabel, desc)
        cd_settings_local(target, status)
        cd_inkling(target, on)
        cd_local_state(target, ui.osOk, ui.hwOk, ui.ramGB, statusCode, ui.pct, ui.dlMB, ui.totalMB, err)
    }
    func syncLocalModels(prewarm: Bool = true) {
        LocalModelStore.verifyExistingIfNeeded { [weak self] in
            self?.syncLocalModels()
        }
        syncLocalState(on: page)
        for r in runs.values where r.page != page {
            syncLocalState(on: r.page)
        }
        let wantsLocal = cd_model_inkling(page) != 0 || AgentSettings.pageNeedsLocal(page)
        if prewarm && LocalModelStore.isSelectable && wantsLocal {
            LocalModelServer.shared.prewarmIfReady()
        }
        if let shownState = localModelsDialogState, overlay.dialogOpen {
            let cur = LocalModelStore.state
            if cur == shownState {
                cd_dialog_body(page, LocalModelStore.dialogBody())
            } else {
                manageLocalModels()
            }
        }
        if FeatureFlags.isOn(.modelsExecutor) {
            cd_hybrid_exec(page, 0)
            for r in runs.values where r.page != page { cd_hybrid_exec(r.page, 0) }
        }
        wake()
    }
}
