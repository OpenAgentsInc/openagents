// Firm reason: 6 (AppKit dialogs, UserDefaults persistence, and worker environment wiring over page/flawed_settings.coax).
import AppKit

/// Persisted settings, key storage, live auth probe, and worker environment
/// builder for the built-in pure-Coax OpenAgents Core worker (`coder/tui/oa-worker.coax`).
enum AgentSettings {
    static let prefsCount = 20
    private static let d = UserDefaults.standard

    private static let kDefaultAgent = "oa.agent.defaultAgent"
    private static let kModelSlot = "oa.agent.modelSlot"
    private static let kBackend = "oa.agent.backend"
    private static let kThinkingMode = "oa.agent.thinkingMode"
    private static let kEffort = "oa.agent.effort"
    private static let kThinkingDisplay = "oa.agent.thinkingDisplay"
    private static let kPermissionMode = "oa.agent.permissionMode"
    private static let kBypassDanger = "oa.agent.bypassDanger"
    private static let kSandboxMode = "oa.agent.sandboxMode"
    private static let kGitGuard = "oa.agent.gitGuard"
    private static let kPromptCache = "oa.agent.promptCache"
    private static let kAutoCompact = "oa.agent.autoCompact"
    private static let kAutoMemory = "oa.agent.autoMemory"
    private static let kBareMode = "oa.agent.bareMode"
    private static let kOutputStyle = "oa.agent.outputStyle"
    private static let kHooksMode = "oa.agent.hooksMode"
    private static let kMcpDefer = "oa.agent.mcpDefer"
    private static let kFastMode = "oa.agent.fastMode"
    private static let kExtendedCtx = "oa.agent.extendedCtx"
    private static let kRemoteBridge = "oa.agent.remoteBridge"

    private static let kBaseURL = "oa.agent.baseURL"
    private static let kCustomModel = "oa.agent.customModel"
    private static let kAllowedTools = "oa.agent.allowedTools"
    private static let kDisallowedTools = "oa.agent.disallowedTools"
    private static let kAdditionalDirs = "oa.agent.additionalDirs"
    private static let kMaxTurns = "oa.agent.maxTurns"
    private static let kMaxBudgetMicros = "oa.agent.maxBudgetMicros"

    private static let kSchemaVersion = "oa.agent.schemaVersion"
    private static let currentSchemaVersion = 5
    private static var didMigrate = false

    private static let legacyKeyPairs: [(String, String)] = [
        ("flawed.defaultAgent", kDefaultAgent),
        ("flawed.modelSlot", kModelSlot),
        ("flawed.backend", kBackend),
        ("flawed.thinkingMode", kThinkingMode),
        ("flawed.effort", kEffort),
        ("flawed.thinkingDisplay", kThinkingDisplay),
        ("flawed.permissionMode", kPermissionMode),
        ("flawed.bypassDanger", kBypassDanger),
        ("flawed.sandboxMode", kSandboxMode),
        ("flawed.gitGuard", kGitGuard),
        ("flawed.promptCache", kPromptCache),
        ("flawed.autoCompact", kAutoCompact),
        ("flawed.autoMemory", kAutoMemory),
        ("flawed.bareMode", kBareMode),
        ("flawed.outputStyle", kOutputStyle),
        ("flawed.hooksMode", kHooksMode),
        ("flawed.mcpDefer", kMcpDefer),
        ("flawed.fastMode", kFastMode),
        ("flawed.extendedCtx", kExtendedCtx),
        ("flawed.remoteBridge", kRemoteBridge),
        ("flawed.baseURL", kBaseURL),
        ("flawed.customModel", kCustomModel),
        ("flawed.allowedTools", kAllowedTools),
        ("flawed.disallowedTools", kDisallowedTools),
        ("flawed.additionalDirs", kAdditionalDirs),
        ("flawed.maxTurns", kMaxTurns),
        ("flawed.maxBudgetMicros", kMaxBudgetMicros),
    ]

    private static func migrateIfNeeded() {
        guard !didMigrate else { return }
        didMigrate = true
        // Seamless one-time migration from legacy flawed.* UserDefaults to oa.agent.*.
        let v = max(d.integer(forKey: kSchemaVersion), d.integer(forKey: "flawed.schemaVersion"))
        if d.object(forKey: kSchemaVersion) == nil {
            for (oldKey, newKey) in legacyKeyPairs {
                if d.object(forKey: newKey) == nil, let val = d.object(forKey: oldKey) {
                    d.set(val, forKey: newKey)
                }
            }
        }
        if v < 2 {
            d.set(0, forKey: kModelSlot)
            d.set(0, forKey: kBackend)
        }
        if v < 3 {
            d.set(0, forKey: kPermissionMode)
            d.set(false, forKey: kBypassDanger)
            d.set(false, forKey: kGitGuard)
        }
        if v < 4 {
            // Always start new chats on OpenAgents · Gemini 4 Neon (slot 0) with Auto backend.
            d.set(0, forKey: kDefaultAgent)
            d.set(0, forKey: kModelSlot)
            d.set(0, forKey: kBackend)
        }
        if v < currentSchemaVersion || d.object(forKey: kSchemaVersion) == nil {
            d.set(currentSchemaVersion, forKey: kSchemaVersion)
        }
    }

    private static var cachedAuthStatus: String = "Checking…"
    private static var probingAuth = false

    private static func getInt(_ key: String, default def: Int) -> Int {
        migrateIfNeeded()
        return d.object(forKey: key) as? Int ?? def
    }
    private static func getBool(_ key: String, default def: Bool) -> Bool {
        migrateIfNeeded()
        return d.object(forKey: key) as? Bool ?? def
    }
    private static func getString(_ key: String) -> String {
        migrateIfNeeded()
        return (d.string(forKey: key) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static var defaultAgent: Int {
        get { max(0, min(1, getInt(kDefaultAgent, default: 0))) }
        set { d.set(max(0, min(1, newValue)), forKey: kDefaultAgent) }
    }
    static var modelSlot: Int {
        get { max(0, min(4, getInt(kModelSlot, default: 0))) }
        set { d.set(max(0, min(4, newValue)), forKey: kModelSlot) }
    }
    static var backend: Int {
        get { max(0, min(4, getInt(kBackend, default: 0))) }
        set { d.set(max(0, min(4, newValue)), forKey: kBackend) }
    }

    /// True when the active page is on Flawed and routed to the local Psionic Metal server (Inkling 2 Mini).
    static func pageNeedsLocal(_ page: UnsafeMutableRawPointer?) -> Bool {
        guard let page, cd_model_flawed(page) != 0 else { return false }
        if backend == 1 { return true }
        return backend == 0 && cd_model_flawed_local(page) != 0
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
        get { getBool(kGitGuard, default: false) }
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

    // ---- Key storage (0600 ~/Library/Application Support/Coder/agent-key; never logged) ----

    static var savedKey: String? {
        let primary = DesktopCore.withCoaxString("agent-key") { cn in
            DesktopCore.callString(4096) { oa_dc_key_read(cn, $0) }
        }
        if !primary.isEmpty { return primary }
        // One-time seamless migration from legacy flawed-key.
        let legacy = DesktopCore.withCoaxString("flawed-key") { cn in
            DesktopCore.callString(4096) { oa_dc_key_read(cn, $0) }
        }
        if !legacy.isEmpty {
            _ = saveKey(legacy)
            return legacy
        }
        return nil
    }

    @discardableResult
    static func saveKey(_ key: String) -> Bool {
        let trimmed = key.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            _ = DesktopCore.withCoaxString("flawed-key") { cn in
                DesktopCore.withCoaxString("") { ck in oa_dc_key_save(cn, ck) }
            }
        }
        return DesktopCore.withCoaxString("agent-key") { cn in
            DesktopCore.withCoaxString(trimmed) { ck in
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
        return "Not set (uses Local Psionic / Vertex AI)"
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

    // ---- Live credential probe via bundled oa-worker --auth ----

    static var authStatus: String { cachedAuthStatus }

    static func probeAuth(_ done: @escaping () -> Void) {
        guard !probingAuth else { return }
        probingAuth = true
        var env: [String: String] = [
            "OA_AGENT_BACKEND": backendWireName(backend),
        ]
        if !baseURL.isEmpty { env["OA_AGENT_BASE_URL"] = baseURL }
        if backend == 1 || backend == 0 {
            env["PSIONIC_BASE_URL"] = LocalModelServer.baseURL
            env["PSIONIC_MODEL"] = LocalModelStore.entry.id
            env["PSIONIC_CTX"] = String(LocalModelStore.recommendedContextTokens)
        }
        DispatchQueue.global(qos: .userInitiated).async {
            let line = CoderProbes.ask("oa-worker", ["--auth"], env: env, timeout: 8)
            var status = "Unavailable (oa-worker missing)"
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
        case 1: return "local"
        case 2: return "vertex"
        case 3: return "gemini"
        case 4: return "openrouter"
        default: return "auto"
        }
    }

    private static func modelWireName(_ slot: Int) -> String {
        switch slot {
        case 0: return "gemini-4-neon"
        case 1: return "gemini-4-carbon"
        case 2: return "gemini-4-hydrogen"
        case 3: return "gemini-4-argon"
        case 4: return "inkling-2-mini"
        default: return "gemini-4-neon"
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
        case 1: return "ask"
        case 2: return "accept_edits"
        case 3: return "plan"
        case 4: return "dont_ask"
        default: return "allow_all"
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
            "OA_AGENT_BACKEND": backendWireName(backend),
            "OA_AGENT_MODEL": modelWireName(modelSlot),
            "OA_AGENT_EFFORT": effortWireName(effort),
            "OA_AGENT_THINKING_MODE": thinkingModeWireName(thinkingMode),
            "OA_AGENT_THINKING_DISPLAY": thinkingDisplay == 1 ? "omitted" : "summarized",
            "OA_AGENT_PERMISSION_MODE": permissionModeWireName(permissionMode),
            "OA_AGENT_BYPASS_PERMISSIONS": bypassDanger ? "1" : "0",
            "OA_AGENT_SANDBOX_MODE": sandboxModeWireName(sandboxMode),
            "OA_AGENT_GIT_GUARD": gitGuard ? "1" : "0",
            "OA_AGENT_PROMPT_CACHE": promptCacheWireName(promptCache),
            "OA_AGENT_AUTO_COMPACT": autoCompact ? "1" : "0",
            "OA_AGENT_AUTO_MEMORY": autoMemory ? "1" : "0",
            "OA_AGENT_BARE_MODE": bareMode ? "1" : "0",
            "OA_AGENT_OUTPUT_STYLE": outputStyleWireName(outputStyle),
            "OA_AGENT_HOOKS_MODE": hooksModeWireName(hooksMode),
            "OA_AGENT_MCP_DEFER": mcpDefer ? "1" : "0",
            "OA_AGENT_FAST_MODE": fastMode ? "1" : "0",
            "OA_AGENT_EXTENDED_CTX": extendedCtx ? "1" : "0",
            "OA_AGENT_REMOTE_BRIDGE": remoteBridge ? "1" : "0",
        ]
        if !customModel.isEmpty { env["OA_AGENT_CUSTOM_MODEL"] = customModel }
        if !baseURL.isEmpty { env["OA_AGENT_BASE_URL"] = baseURL }
        if !allowedTools.isEmpty { env["OA_AGENT_ALLOWED_TOOLS"] = allowedTools }
        if !disallowedTools.isEmpty { env["OA_AGENT_DISALLOWED_TOOLS"] = disallowedTools }
        if !additionalDirs.isEmpty { env["OA_AGENT_ADDITIONAL_DIRS"] = additionalDirs }
        if maxTurns > 0 { env["OA_AGENT_MAX_TURNS"] = String(maxTurns) }
        if maxBudgetMicros > 0 { env["OA_AGENT_MAX_BUDGET_MICROS"] = String(maxBudgetMicros) }
        env["OA_FULL_AUTO"] = isFullAuto ? "1" : "0"
        env["OA_OPERATOR_VAULT_PATH"] = OperatorVault.vaultURL.path
        return env
    }

    /// True when global Full Auto mode is active (default ON: Allow All or Bypass Danger).
    static var isFullAuto: Bool {
        permissionMode == 0 || bypassDanger
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
        cd_permit_full_auto_set(page, isFullAuto ? 1 : 0)
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
        gitGuard = false
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

/// Autonomous Operator Vault & Autofill Profile (#14063).
/// Persists local-only (0600) tokens, API keys, identity/form fields, and billing/bank autofill
/// entries at `~/.openagents/vault/operator-profile.json` so Full Auto resolves secret/form
/// requests without interrupting the operator. Values are never logged to chat transcripts.
final class OperatorVault {
    static let shared = OperatorVault()

    static var vaultURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/vault/operator-profile.json")
    }

    static func normalizeKey(_ raw: String) -> String {
        var out = ""
        var prevSep = true
        for scalar in raw.unicodeScalars {
            let v = scalar.value
            if (v >= 65 && v <= 90) || (v >= 97 && v <= 122) || (v >= 48 && v <= 57) {
                out.append(Character(scalar).uppercased())
                prevSep = false
            } else if v == 95 || v == 45 || v == 46 || v == 32 {
                if !prevSep { out.append("_") }
                prevSep = true
            }
        }
        while out.hasSuffix("_") { out.removeLast() }
        return out
    }

    func loadAll() -> [String: String] {
        guard let data = try? Data(contentsOf: Self.vaultURL),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return [:]
        }
        var flat: [String: String] = [:]
        for (k, v) in obj {
            if let s = v as? String, !s.isEmpty {
                flat[Self.normalizeKey(k)] = s
            } else if let dict = v as? [String: Any] {
                for (sk, sv) in dict {
                    if let s = sv as? String, !s.isEmpty {
                        flat[Self.normalizeKey(sk)] = s
                    }
                }
            }
        }
        return flat
    }

    /// Resolves a requested secret/token/form key from:
    /// 1. `~/.openagents/vault/operator-profile.json`
    /// 2. `~/Library/Application Support/Coder/secrets/<slug>`
    /// 3. Current process environment variables
    func resolve(key rawKey: String) -> String? {
        let norm = Self.normalizeKey(rawKey)
        guard !norm.isEmpty else { return nil }
        let vault = loadAll()
        if let v = vault[norm], !v.isEmpty { return v }
        let fm = FileManager.default
        let slug = rawKey.lowercased().map { ch -> Character in
            (ch.isASCII && (ch.isLetter || ch.isNumber || ch == "-" || ch == "_")) ? ch : "-"
        }
        let cleanSlug = String(slug).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        if !cleanSlug.isEmpty,
           let base = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first {
            let secURL = base.appendingPathComponent("Coder/secrets/\(cleanSlug)")
            if let text = try? String(contentsOf: secURL, encoding: .utf8) {
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty { return trimmed }
            }
        }
        if let envVal = ProcessInfo.processInfo.environment[norm], !envVal.isEmpty {
            return envVal
        }
        if let rawEnv = ProcessInfo.processInfo.environment[rawKey], !rawEnv.isEmpty {
            return rawEnv
        }
        return nil
    }

    @discardableResult
    func save(key rawKey: String, value: String) -> Bool {
        let norm = Self.normalizeKey(rawKey)
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !norm.isEmpty, !trimmed.isEmpty else { return false }
        var map = loadAll()
        map[norm] = trimmed
        let fm = FileManager.default
        let dir = Self.vaultURL.deletingLastPathComponent()
        do {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            let data = try JSONSerialization.data(withJSONObject: map, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: Self.vaultURL, options: .atomic)
            try fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: Self.vaultURL.path)
            return true
        } catch {
            return false
        }
    }
}

extension CoderWindow {
    /// Mirrors Flawed settings onto the foreground page and every active background run.
    func syncFlawedSettingsAll() {
        AgentSettings.apply(to: page)
        for r in runs.values where r.page != page {
            AgentSettings.apply(to: r.page)
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
        let prevBackend = AgentSettings.backend
        AgentSettings.readBack(from: page)
        syncFlawedSettingsAll()
        if AgentSettings.backend != prevBackend {
            checkFlawedAuth(showToast: false)
        }
    }

    func checkFlawedAuth(showToast: Bool) {
        AgentSettings.probeAuth { [weak self] in
            guard let self else { return }
            self.syncFlawedSettingsAll()
            if showToast {
                cd_toast(self.page, "Agent auth: \(AgentSettings.authStatus)", 0)
                self.wake()
            }
        }
    }

    func editFlawedKey() {
        let hasSaved = AgentSettings.savedKey != nil
        var d = CoderDialog(
            size: .md,
            title: "Gemini / OpenRouter API key",
            body: "Optional key stored in ~/Library/Application Support/Coder/agent-key (0600, never logged). Not needed when using Local Inkling 2 Mini or Google Cloud Vertex AI service account credentials."
        )
        d.fields = [
            .init(label: "API key", value: "", hint: hasSaved ? "Currently \(AgentSettings.keyLabel) — paste a new key to replace" : "AIza… (Gemini) or sk-or-v1-… (OpenRouter)", secure: true),
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
                _ = AgentSettings.saveKey("")
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
                cd_toast(self.page, "Removed saved Agent API key.", 0)
            } else if answer == d.confirm {
                let raw = (values.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
                if !raw.isEmpty {
                    _ = AgentSettings.saveKey(raw)
                    self.syncFlawedSettingsAll()
                    self.checkFlawedAuth(showToast: false)
                    cd_toast(self.page, "Saved Agent API key.", 0)
                }
            }
            self.refocus()
        }
    }

    func editFlawedBaseURL() {
        let hasURL = !AgentSettings.baseURL.isEmpty
        var d = CoderDialog(
            size: .md,
            title: "Custom API base URL",
            body: "Optional endpoint override for Vertex/Gemini proxies, OpenRouter mirrors, or remote Psionic Metal servers. Leave blank for the default endpoint."
        )
        d.fields = [
            .init(label: "Base URL", value: AgentSettings.baseURL, hint: "https://generativelanguage.googleapis.com"),
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
                AgentSettings.baseURL = ""
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
            } else if answer == d.confirm {
                AgentSettings.baseURL = values.first ?? ""
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
            }
            self.refocus()
        }
    }

    func editFlawedCustomModel() {
        let hasCustom = !AgentSettings.customModel.isEmpty
        var d = CoderDialog(
            size: .md,
            title: "Custom model override",
            body: "Override the model slug sent on the wire (e.g. gemini-4-neon, gemini-4-carbon, inkling-2-mini, google/gemini-4-hydrogen). Leave blank to use the model selected in the composer picker."
        )
        d.fields = [
            .init(label: "Model slug", value: AgentSettings.customModel, hint: "Leave empty to use picker selection"),
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
                AgentSettings.customModel = ""
                self.syncFlawedSettingsAll()
            } else if answer == d.confirm {
                AgentSettings.customModel = values.first ?? ""
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
            .init(label: "Allowed tools (empty = all)", value: AgentSettings.allowedTools, hint: "e.g. Read,Glob,Grep,Edit"),
            .init(label: "Disallowed tools", value: AgentSettings.disallowedTools, hint: "e.g. Bash,WebFetch"),
            .init(label: "Additional directories (CSV)", value: AgentSettings.additionalDirs, hint: "e.g. ~/shared-lib,/opt/specs"),
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
                AgentSettings.allowedTools = ""
                AgentSettings.disallowedTools = ""
                AgentSettings.additionalDirs = ""
                self.syncFlawedSettingsAll()
            } else if answer == 2 {
                AgentSettings.allowedTools = values.count > 0 ? values[0] : ""
                AgentSettings.disallowedTools = values.count > 1 ? values[1] : ""
                AgentSettings.additionalDirs = values.count > 2 ? values[2] : ""
                self.syncFlawedSettingsAll()
            }
            self.refocus()
        }
    }

    func editFlawedLimits() {
        let curTurns = AgentSettings.maxTurns > 0 ? String(AgentSettings.maxTurns) : ""
        let curBudget = AgentSettings.maxBudgetMicros > 0
            ? String(format: "%.2f", Double(AgentSettings.maxBudgetMicros) / 1_000_000.0)
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
                AgentSettings.maxTurns = 0
                AgentSettings.maxBudgetMicros = 0
                self.syncFlawedSettingsAll()
            } else if answer == 2 {
                let tRaw = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                let bRaw = (values.count > 1 ? values[1] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                    .replacingOccurrences(of: "$", with: "")
                AgentSettings.maxTurns = max(0, Int(tRaw) ?? 0)
                if let usd = Double(bRaw), usd > 0 {
                    AgentSettings.maxBudgetMicros = Int64((usd * 1_000_000.0).rounded())
                } else {
                    AgentSettings.maxBudgetMicros = 0
                }
                self.syncFlawedSettingsAll()
            }
            self.refocus()
        }
    }

    func resetFlawedSettings() {
        var d = CoderDialog(
            size: .sm,
            title: "Reset Agent settings?",
            body: "Restores Agent defaults (Gemini 4 Neon, Allow All permissions, sandbox off, thinking adaptive, compaction & memory on). Your saved API key is kept."
        )
        d.buttons = [
            .init(title: "Cancel"),
            .init(title: "Reset", kind: .danger),
        ]
        d.cancel = 0; d.confirm = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 {
                AgentSettings.resetAll()
                self.syncFlawedSettingsAll()
                self.checkFlawedAuth(showToast: false)
                cd_toast(self.page, "Agent settings reset to defaults.", 0)
            }
            self.refocus()
        }
    }
}
