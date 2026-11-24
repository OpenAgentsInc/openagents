// Firm reason: 6 (AppKit dialogs, UserDefaults persistence, and worker environment wiring over page/flawed_settings.coax).
import AppKit

/// Persisted Claude-Code-parity settings, key storage, live auth probe, and worker environment
/// builder for the built-in pure-Coax Flawed Agent (`coder/tui/flawedworker.coax`).
enum FlawedSettings {
    static let prefsCount = 20
    private static let d = UserDefaults.standard

    private static let kDefaultAgent = "flawed.defaultAgent"
    private static let kModelSlot = "flawed.modelSlot"
    private static let kBackend = "flawed.backend"
    private static let kThinkingMode = "flawed.thinkingMode"
    private static let kEffort = "flawed.effort"
    private static let kThinkingDisplay = "flawed.thinkingDisplay"
    private static let kPermissionMode = "flawed.permissionMode"
    private static let kBypassDanger = "flawed.bypassDanger"
    private static let kSandboxMode = "flawed.sandboxMode"
    private static let kGitGuard = "flawed.gitGuard"
    private static let kPromptCache = "flawed.promptCache"
    private static let kAutoCompact = "flawed.autoCompact"
    private static let kAutoMemory = "flawed.autoMemory"
    private static let kBareMode = "flawed.bareMode"
    private static let kOutputStyle = "flawed.outputStyle"
    private static let kHooksMode = "flawed.hooksMode"
    private static let kMcpDefer = "flawed.mcpDefer"
    private static let kFastMode = "flawed.fastMode"
    private static let kExtendedCtx = "flawed.extendedCtx"
    private static let kRemoteBridge = "flawed.remoteBridge"

    private static let kBaseURL = "flawed.baseURL"
    private static let kCustomModel = "flawed.customModel"
    private static let kAllowedTools = "flawed.allowedTools"
    private static let kDisallowedTools = "flawed.disallowedTools"
    private static let kAdditionalDirs = "flawed.additionalDirs"
    private static let kMaxTurns = "flawed.maxTurns"
    private static let kMaxBudgetMicros = "flawed.maxBudgetMicros"

    private static var cachedAuthStatus: String = "Checking…"
    private static var probingAuth = false

    private static func getInt(_ key: String, default def: Int) -> Int {
        d.object(forKey: key) as? Int ?? def
    }
    private static func getBool(_ key: String, default def: Bool) -> Bool {
        d.object(forKey: key) as? Bool ?? def
    }
    private static func getString(_ key: String) -> String {
        (d.string(forKey: key) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static var defaultAgent: Int {
        get { max(0, min(1, getInt(kDefaultAgent, default: 0))) }
        set { d.set(max(0, min(1, newValue)), forKey: kDefaultAgent) }
    }
    static var modelSlot: Int {
        get { max(0, min(3, getInt(kModelSlot, default: 0))) }
        set { d.set(max(0, min(3, newValue)), forKey: kModelSlot) }
    }
    static var backend: Int {
        get { max(0, min(4, getInt(kBackend, default: 0))) }
        set { d.set(max(0, min(4, newValue)), forKey: kBackend) }
    }
    static var thinkingMode: Int {
        get { max(0, min(2, getInt(kThinkingMode, default: 0))) }
        set { d.set(max(0, min(2, newValue)), forKey: kThinkingMode) }
    }
    static var effort: Int {
        get { max(0, min(4, getInt(kEffort, default: 2))) }
        set { d.set(max(0, min(4, newValue)), forKey: kEffort) }
    }
    static var thinkingDisplay: Int {
        get { max(0, min(1, getInt(kThinkingDisplay, default: 0))) }
        set { d.set(max(0, min(1, newValue)), forKey: kThinkingDisplay) }
    }
    static var permissionMode: Int {
        get { max(0, min(4, getInt(kPermissionMode, default: 0))) }
        set { d.set(max(0, min(4, newValue)), forKey: kPermissionMode) }
    }
    static var bypassDanger: Bool {
        get { getBool(kBypassDanger, default: false) }
        set { d.set(newValue, forKey: kBypassDanger) }
    }
    static var sandboxMode: Int {
        get { max(0, min(2, getInt(kSandboxMode, default: 0))) }
        set { d.set(max(0, min(2, newValue)), forKey: kSandboxMode) }
    }
    static var gitGuard: Bool {
        get { getBool(kGitGuard, default: true) }
        set { d.set(newValue, forKey: kGitGuard) }
    }
    static var promptCache: Int {
        get { max(0, min(2, getInt(kPromptCache, default: 0))) }
        set { d.set(max(0, min(2, newValue)), forKey: kPromptCache) }
    }
    static var autoCompact: Bool {
        get { getBool(kAutoCompact, default: true) }
        set { d.set(newValue, forKey: kAutoCompact) }
    }
    static var autoMemory: Bool {
        get { getBool(kAutoMemory, default: true) }
        set { d.set(newValue, forKey: kAutoMemory) }
    }
    static var bareMode: Bool {
        get { getBool(kBareMode, default: false) }
        set { d.set(newValue, forKey: kBareMode) }
    }
    static var outputStyle: Int {
        get { max(0, min(3, getInt(kOutputStyle, default: 0))) }
        set { d.set(max(0, min(3, newValue)), forKey: kOutputStyle) }
    }
    static var hooksMode: Int {
        get { max(0, min(2, getInt(kHooksMode, default: 0))) }
        set { d.set(max(0, min(2, newValue)), forKey: kHooksMode) }
    }
    static var mcpDefer: Bool {
        get { getBool(kMcpDefer, default: true) }
        set { d.set(newValue, forKey: kMcpDefer) }
    }
    static var fastMode: Bool {
        get { getBool(kFastMode, default: false) }
        set { d.set(newValue, forKey: kFastMode) }
    }
    static var extendedCtx: Bool {
        get { getBool(kExtendedCtx, default: false) }
        set { d.set(newValue, forKey: kExtendedCtx) }
    }
    static var remoteBridge: Bool {
        get { getBool(kRemoteBridge, default: false) }
        set { d.set(newValue, forKey: kRemoteBridge) }
    }

    static var baseURL: String {
        get { getString(kBaseURL) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kBaseURL) }
    }
    static var customModel: String {
        get { getString(kCustomModel) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kCustomModel) }
    }
    static var allowedTools: String {
        get { getString(kAllowedTools) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kAllowedTools) }
    }
    static var disallowedTools: String {
        get { getString(kDisallowedTools) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kDisallowedTools) }
    }
    static var additionalDirs: String {
        get { getString(kAdditionalDirs) }
        set { d.set(newValue.trimmingCharacters(in: .whitespacesAndNewlines), forKey: kAdditionalDirs) }
    }
    static var maxTurns: Int {
        get { max(0, getInt(kMaxTurns, default: 0)) }
        set { d.set(max(0, newValue), forKey: kMaxTurns) }
    }
    static var maxBudgetMicros: Int64 {
        get { max(0, Int64(d.object(forKey: kMaxBudgetMicros) as? Int64 ?? Int64(getInt(kMaxBudgetMicros, default: 0)))) }
        set { d.set(max(0, newValue), forKey: kMaxBudgetMicros) }
    }

    // ---- Key storage (0600 ~/Library/Application Support/Coder/flawed-key; never logged) ----

    static var savedKey: String? {
        let s = DesktopCore.withCoaxString("flawed-key") { cn in
            DesktopCore.callString(4096) { oa_dc_key_read(cn, $0) }
        }
        return s.isEmpty ? nil : s
    }

    @discardableResult
    static func saveKey(_ key: String) -> Bool {
        DesktopCore.withCoaxString("flawed-key") { cn in
            DesktopCore.withCoaxString(key.trimmingCharacters(in: .whitespacesAndNewlines)) { ck in
                oa_dc_key_save(cn, ck) == 1
            }
        }
    }

    static var keyLabel: String {
        if let key = savedKey {
            let tail = DesktopCore.withCoaxString(key) { ck in
                DesktopCore.callString(64) { oa_dc_key_tail(ck, $0) }
            }
            return tail.isEmpty ? "Saved" : "Ends \(tail)"
        }
        return "Not set (uses Anthropic / OAuth / OpenRouter)"
    }

    static var toolsSummary: String {
        var parts: [String] = []
        if !allowedTools.isEmpty { parts.append("Allow: \(allowedTools)") }
        if !disallowedTools.isEmpty { parts.append("Deny: \(disallowedTools)") }
        if !additionalDirs.isEmpty {
            let n = additionalDirs.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }.count
            if n > 0 { parts.append("+\(n) extra dir\(n == 1 ? "" : "s")") }
        }
        return parts.isEmpty ? "All built-in tools" : parts.joined(separator: " · ")
    }

    static var limitsSummary: String {
        let turnsPart = maxTurns > 0 ? "Max \(maxTurns) turn\(maxTurns == 1 ? "" : "s")" : "Unlimited turns"
        let budgetPart: String
        if maxBudgetMicros > 0 {
            budgetPart = String(format: "$%.2f cap", Double(maxBudgetMicros) / 1_000_000.0)
        } else {
            budgetPart = "No extra cap"
        }
        return "\(turnsPart) · \(budgetPart)"
    }

    static var baseURLLabel: String {
        baseURL.isEmpty ? "Default endpoint" : baseURL
    }

    // ---- Live credential probe via bundled flawedworker --auth ----

    static var authStatus: String { cachedAuthStatus }

    static func probeAuth(_ done: @escaping () -> Void) {
        guard !probingAuth else { return }
        probingAuth = true
        var env: [String: String] = [
            "FLAWED_BACKEND": backendWireName(backend),
        ]
        if !baseURL.isEmpty { env["FLAWED_BASE_URL"] = baseURL }
        if backend == 4 || backend == 0 {
            env["PSIONIC_BASE_URL"] = LocalModelServer.baseURL
        }
        DispatchQueue.global(qos: .userInitiated).async {
            let line = CoderProbes.ask("flawedworker", ["--auth"], env: env, timeout: 8)
            var status = "Unavailable (flawedworker missing)"
            if !line.isEmpty, let data = line.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                let ready = (obj["ready"] as? Bool) ?? false
                let label = (obj["label"] as? String) ?? ""
                if ready {
                    status = label.isEmpty ? "Ready" : label
                } else {
                    status = label.isEmpty ? "No credentials configured" : label
                }
            }
            DispatchQueue.main.async {
                probingAuth = false
                cachedAuthStatus = status
                done()
            }
        }
    }

    // ---- Wire environment for flawedworker ----

    private static func backendWireName(_ v: Int) -> String {
        switch v {
        case 1: return "anthropic"
        case 2: return "claude_oauth"
        case 3: return "openrouter"
        case 4: return "local"
        default: return "auto"
        }
    }

    private static func modelWireName(_ slot: Int) -> String {
        switch slot {
        case 1: return "claude-sonnet-4-6"
        case 2: return "claude-haiku-4-5"
        case 3: return "claude-opus-4-6"
        default: return "claude-opus-4-7"
        }
    }

    private static func effortWireName(_ e: Int) -> String {
        switch e {
        case 0: return "low"
        case 1: return "medium"
        case 3: return "xhigh"
        case 4: return "max"
        default: return "high"
        }
    }

    private static func thinkingModeWireName(_ v: Int) -> String {
        switch v {
        case 1: return "extended"
        case 2: return "off"
        default: return "adaptive"
        }
    }

    private static func permissionModeWireName(_ v: Int) -> String {
        switch v {
        case 1: return "accept_edits"
        case 2: return "plan"
        case 3: return "auto"
        case 4: return "dont_ask"
        default: return "default"
        }
    }

    private static func sandboxModeWireName(_ v: Int) -> String {
        switch v {
        case 1: return "auto"
        case 2: return "enforced"
        default: return "off"
        }
    }

    private static func promptCacheWireName(_ v: Int) -> String {
        switch v {
        case 1: return "org"
        case 2: return "off"
        default: return "global"
        }
    }

    private static func outputStyleWireName(_ v: Int) -> String {
        switch v {
        case 1: return "concise"
        case 2: return "explanatory"
        case 3: return "learning"
        default: return "default"
        }
    }

    private static func hooksModeWireName(_ v: Int) -> String {
        switch v {
        case 1: return "managed"
        case 2: return "off"
        default: return "all"
        }
    }

    static func workerEnv() -> [String: String] {
        var env: [String: String] = [
            "FLAWED_BACKEND": backendWireName(backend),
            "FLAWED_MODEL": modelWireName(modelSlot),
            "FLAWED_EFFORT": effortWireName(effort),
            "FLAWED_THINKING_MODE": thinkingModeWireName(thinkingMode),
            "FLAWED_THINKING_DISPLAY": thinkingDisplay == 1 ? "omitted" : "summarized",
            "FLAWED_PERMISSION_MODE": permissionModeWireName(permissionMode),
            "FLAWED_BYPASS_PERMISSIONS": bypassDanger ? "1" : "0",
            "FLAWED_SANDBOX_MODE": sandboxModeWireName(sandboxMode),
            "FLAWED_GIT_GUARD": gitGuard ? "1" : "0",
            "FLAWED_PROMPT_CACHE": promptCacheWireName(promptCache),
            "FLAWED_AUTO_COMPACT": autoCompact ? "1" : "0",
            "FLAWED_AUTO_MEMORY": autoMemory ? "1" : "0",
            "FLAWED_BARE_MODE": bareMode ? "1" : "0",
            "FLAWED_OUTPUT_STYLE": outputStyleWireName(outputStyle),
            "FLAWED_HOOKS_MODE": hooksModeWireName(hooksMode),
            "FLAWED_MCP_DEFER": mcpDefer ? "1" : "0",
            "FLAWED_FAST_MODE": fastMode ? "1" : "0",
            "FLAWED_EXTENDED_CTX": extendedCtx ? "1" : "0",
            "FLAWED_REMOTE_BRIDGE": remoteBridge ? "1" : "0",
        ]
        if !customModel.isEmpty { env["FLAWED_CUSTOM_MODEL"] = customModel }
        if !baseURL.isEmpty { env["FLAWED_BASE_URL"] = baseURL }
        if !allowedTools.isEmpty { env["FLAWED_ALLOWED_TOOLS"] = allowedTools }
        if !disallowedTools.isEmpty { env["FLAWED_DISALLOWED_TOOLS"] = disallowedTools }
        if !additionalDirs.isEmpty { env["FLAWED_ADDITIONAL_DIRS"] = additionalDirs }
        if maxTurns > 0 { env["FLAWED_MAX_TURNS"] = String(maxTurns) }
        if maxBudgetMicros > 0 { env["FLAWED_MAX_BUDGET_MICROS"] = String(maxBudgetMicros) }
        return env
    }

    // ---- Page synchronization ----

    static func prefsVector() -> [Int32] {
        [
            Int32(defaultAgent),
            Int32(modelSlot),
            Int32(backend),
            Int32(thinkingMode),
            Int32(effort),
            Int32(thinkingDisplay),
            Int32(permissionMode),
            bypassDanger ? 1 : 0,
            Int32(sandboxMode),
            gitGuard ? 1 : 0,
            Int32(promptCache),
            autoCompact ? 1 : 0,
            autoMemory ? 1 : 0,
            bareMode ? 1 : 0,
            Int32(outputStyle),
            Int32(hooksMode),
            mcpDefer ? 1 : 0,
            fastMode ? 1 : 0,
            extendedCtx ? 1 : 0,
            remoteBridge ? 1 : 0,
        ]
    }

    static func apply(to page: UnsafeMutableRawPointer?) {
        guard let page else { return }
        let vec = prefsVector()
        vec.withUnsafeBufferPointer { buf in
            cd_flawed_prefs_set(page, buf.baseAddress, Int32(buf.count))
        }
        cd_flawed_info(page, authStatus, keyLabel, baseURLLabel, customModel, toolsSummary, limitsSummary)
    }

    static func readBack(from page: UnsafeMutableRawPointer?) {
        guard let page else { return }
        var vec = [Int32](repeating: 0, count: prefsCount)
        let n = vec.withUnsafeMutableBufferPointer { buf in
            cd_flawed_prefs_get(page, buf.baseAddress, Int32(buf.count))
        }
        guard n >= prefsCount else { return }
        defaultAgent = Int(vec[0])
        modelSlot = Int(vec[1])
        backend = Int(vec[2])
        thinkingMode = Int(vec[3])
        effort = Int(vec[4])
        thinkingDisplay = Int(vec[5])
        permissionMode = Int(vec[6])
        bypassDanger = vec[7] != 0
        sandboxMode = Int(vec[8])
        gitGuard = vec[9] != 0
        promptCache = Int(vec[10])
        autoCompact = vec[11] != 0
        autoMemory = vec[12] != 0
        bareMode = vec[13] != 0
        outputStyle = Int(vec[14])
        hooksMode = Int(vec[15])
        mcpDefer = vec[16] != 0
        fastMode = vec[17] != 0
        extendedCtx = vec[18] != 0
        remoteBridge = vec[19] != 0
    }

    static func resetAll() {
        defaultAgent = 0
        modelSlot = 0
        backend = 0
        thinkingMode = 0
        effort = 2
        thinkingDisplay = 0
        permissionMode = 0
        bypassDanger = false
        sandboxMode = 0
        gitGuard = true
        promptCache = 0
        autoCompact = true
        autoMemory = true
        bareMode = false
        outputStyle = 0
        hooksMode = 0
        mcpDefer = true
        fastMode = false
        extendedCtx = false
        remoteBridge = false
        baseURL = ""
        customModel = ""
        allowedTools = ""
        disallowedTools = ""
        additionalDirs = ""
        maxTurns = 0
        maxBudgetMicros = 0
    }
}

extension CoderWindow {
    /// Mirrors Flawed settings onto the foreground page and every active background run.
    func syncFlawedSettingsAll() {
        FlawedSettings.apply(to: page)
        for r in runs.values where r.page != page {
            FlawedSettings.apply(to: r.page)
        }
        configureWorkerEnv(run, isForeman: isForemanShown)
        wake()
    }

    /// Dispatches Settings -> Flawed actions (CP_ASK_FLAWED_* = 190..197).
    func flawedAsked(_ what: Int32) {
        switch what {
        case 190: flawedPrefsChanged()
        case 191: editFlawedKey()
        case 192: editFlawedBaseURL()
        case 193: editFlawedCustomModel()
        case 194: editFlawedTools()
        case 195: editFlawedLimits()
        case 196: checkFlawedAuth(showToast: true)
        case 197: resetFlawedSettings()
        default: break
        }
    }

    func flawedPrefsChanged() {
        let prevBackend = FlawedSettings.backend
        FlawedSettings.readBack(from: page)
        syncFlawedSettingsAll()
        if FlawedSettings.backend != prevBackend {
            checkFlawedAuth(showToast: false)
        }
    }

    func checkFlawedAuth(showToast: Bool) {
        FlawedSettings.probeAuth { [weak self] in
            guard let self else { return }
            self.syncFlawedSettingsAll()
            if showToast {
                cd_toast(self.page, "Flawed auth: \(FlawedSettings.authStatus)", 0)
                self.wake()
            }
        }
    }

    func editFlawedKey() {
        let hasSaved = FlawedSettings.savedKey != nil
        var d = CoderDialog(
            size: .md,
            title: "Anthropic / Flawed API key",
            body: "Stored in ~/Library/Application Support/Coder/flawed-key (0600, never logged). Leave empty or click Remove to fall back to ANTHROPIC_API_KEY, Claude Code OAuth, OpenRouter, or Local Psionic."
        )
        d.fields = [
            .init(label: "API key", value: "", hint: hasSaved ? "Currently \(FlawedSettings.keyLabel) — paste a new key to replace" : "sk-ant-api03-… or sk-or-v1-…", secure: true),
        ]
        if hasSaved {
            d.buttons = [
                .init(title: "Remove", kind: .danger, aside: true),
                .init(title: "Cancel"),
                .init(title: "Save", kind: .primary),
            ]
            d.cancel = 1; d.confirm = 2
        } else {
            d.buttons = [
                .init(title: "Cancel"),
                .init(title: "Save", kind: .primary),
            ]
            d.cancel = 0; d.confirm = 1
        }
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if hasSaved && answer == 0 {
                _ = FlawedSettings.saveKey("")
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
                cd_toast(self.page, "Removed saved Flawed API key.", 0)
            } else if answer == d.confirm {
                let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !raw.isEmpty {
                    _ = FlawedSettings.saveKey(raw)
                    self.syncFlawedSettingsAll()
                    self.checkFlawedAuth(showToast: false)
                    cd_toast(self.page, "Saved Flawed API key.", 0)
                }
            }
            self.refocus()
        }
    }

    func editFlawedBaseURL() {
        let hasURL = !FlawedSettings.baseURL.isEmpty
        var d = CoderDialog(
            size: .md,
            title: "Custom API base URL",
            body: "Optional endpoint override for Anthropic-compatible proxies, enterprise gateways, OpenRouter mirrors, or custom OpenAI-compatible servers. Leave blank for the default endpoint."
        )
        d.fields = [
            .init(label: "Base URL", value: FlawedSettings.baseURL, hint: "https://api.anthropic.com"),
        ]
        if hasURL {
            d.buttons = [
                .init(title: "Reset", kind: .danger, aside: true),
                .init(title: "Cancel"),
                .init(title: "Save", kind: .primary),
            ]
            d.cancel = 1; d.confirm = 2
        } else {
            d.buttons = [
                .init(title: "Cancel"),
                .init(title: "Save", kind: .primary),
            ]
            d.cancel = 0; d.confirm = 1
        }
        d.selectAll = true
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if hasURL && answer == 0 {
                FlawedSettings.baseURL = ""
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
            } else if answer == d.confirm {
                FlawedSettings.baseURL = values.first ?? ""
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
            }
            self.refocus()
        }
    }

    func editFlawedCustomModel() {
        let hasCustom = !FlawedSettings.customModel.isEmpty
        var d = CoderDialog(
            size: .md,
            title: "Custom model override",
            body: "Override the model slug sent on the wire (e.g. claude-opus-4-7, claude-sonnet-4-6[1m], anthropic/claude-opus-4-7). Leave blank to use the model selected in the composer picker."
        )
        d.fields = [
            .init(label: "Model slug", value: FlawedSettings.customModel, hint: "Leave empty to use picker selection"),
        ]
        if hasCustom {
            d.buttons = [
                .init(title: "Clear", kind: .danger, aside: true),
                .init(title: "Cancel"),
                .init(title: "Save", kind: .primary),
            ]
            d.cancel = 1; d.confirm = 2
        } else {
            d.buttons = [
                .init(title: "Cancel"),
                .init(title: "Save", kind: .primary),
            ]
            d.cancel = 0; d.confirm = 1
        }
        d.selectAll = true
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if hasCustom && answer == 0 {
                FlawedSettings.customModel = ""
                self.syncFlawedSettingsAll()
            } else if answer == d.confirm {
                FlawedSettings.customModel = values.first ?? ""
                self.syncFlawedSettingsAll()
            }
            self.refocus()
        }
    }

    func editFlawedTools() {
        var d = CoderDialog(
            size: .md,
            title: "Tool rules & extra directories",
            body: "Comma-separated built-in tool names: Read, Edit, Write, Bash, Glob, Grep, TodoWrite, WebFetch. Disallowed tools always win over allowed tools."
        )
        d.fields = [
            .init(label: "Allowed tools (empty = all)", value: FlawedSettings.allowedTools, hint: "e.g. Read,Glob,Grep,Edit"),
            .init(label: "Disallowed tools", value: FlawedSettings.disallowedTools, hint: "e.g. Bash,WebFetch"),
            .init(label: "Additional directories (CSV)", value: FlawedSettings.additionalDirs, hint: "e.g. ~/shared-lib,/opt/specs"),
        ]
        d.buttons = [
            .init(title: "Clear all", kind: .danger, aside: true),
            .init(title: "Cancel"),
            .init(title: "Save", kind: .primary),
        ]
        d.cancel = 1; d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 0 {
                FlawedSettings.allowedTools = ""
                FlawedSettings.disallowedTools = ""
                FlawedSettings.additionalDirs = ""
                self.syncFlawedSettingsAll()
            } else if answer == 2 {
                FlawedSettings.allowedTools = values.count > 0 ? values[0] : ""
                FlawedSettings.disallowedTools = values.count > 1 ? values[1] : ""
                FlawedSettings.additionalDirs = values.count > 2 ? values[2] : ""
                self.syncFlawedSettingsAll()
            }
            self.refocus()
        }
    }

    func editFlawedLimits() {
        let curTurns = FlawedSettings.maxTurns > 0 ? String(FlawedSettings.maxTurns) : ""
        let curBudget = FlawedSettings.maxBudgetMicros > 0
            ? String(format: "%.2f", Double(FlawedSettings.maxBudgetMicros) / 1_000_000.0)
            : ""
        var d = CoderDialog(
            size: .md,
            title: "Turn & budget limits",
            body: "Set optional per-request guardrails matching Claude Code's --max-turns and --max-budget-usd flags. Leave empty or 0 for unlimited."
        )
        d.fields = [
            .init(label: "Max agentic turns (0 = unlimited)", value: curTurns, hint: "e.g. 40"),
            .init(label: "Max spend per turn in USD (0 = no extra cap)", value: curBudget, hint: "e.g. 5.00"),
        ]
        d.buttons = [
            .init(title: "Clear", kind: .danger, aside: true),
            .init(title: "Cancel"),
            .init(title: "Save", kind: .primary),
        ]
        d.cancel = 1; d.confirm = 2
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 0 {
                FlawedSettings.maxTurns = 0
                FlawedSettings.maxBudgetMicros = 0
                self.syncFlawedSettingsAll()
            } else if answer == 2 {
                let tRaw = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                let bRaw = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                    .replacingOccurrences(of: "$", with: "")
                FlawedSettings.maxTurns = max(0, Int(tRaw) ?? 0)
                if let usd = Double(bRaw), usd > 0 {
                    FlawedSettings.maxBudgetMicros = Int64((usd * 1_000_000.0).rounded())
                } else {
                    FlawedSettings.maxBudgetMicros = 0
                }
                self.syncFlawedSettingsAll()
            }
            self.refocus()
        }
    }

    func resetFlawedSettings() {
        var d = CoderDialog(
            size: .sm,
            title: "Reset Flawed settings?",
            body: "Restores Flawed's default agent, model, thinking, permissions, sandbox, caching, compaction, output style, hooks, tool rules, and limits to Claude Code defaults. Your saved API key is kept."
        )
        d.buttons = [
            .init(title: "Cancel"),
            .init(title: "Reset", kind: .danger),
        ]
        d.cancel = 0; d.confirm = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 {
                FlawedSettings.resetAll()
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
                cd_toast(self.page, "Flawed settings reset to defaults.", 0)
            }
            self.refocus()
        }
    }
}
